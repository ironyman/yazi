use std::{borrow::Cow, num::NonZeroU8, str::FromStr, sync::{Arc, LazyLock}, time::Duration};

use anyhow::{Result, bail};
use hashbrown::HashMap;
use parking_lot::Mutex;
use strum::VariantNames;
use toml_edit::{Array, InlineTable, Value};
use yazi_config::{THEME, YAZI, Yazi, mgr::{MgrRatio, MouseEvents}, theme::Theme};
use yazi_fs::{SortBy, SortFallback};
use yazi_macro::{emit, relay};
use yazi_shim::strum::IntoStr;

use crate::{notify::{MessageLevel, MessageOpt}, tab::Preference};

/// Values saved for settings that only apply after a restart, keyed by name.
static PENDING: LazyLock<Mutex<HashMap<&'static str, String>>> = LazyLock::new(Default::default);

#[derive(Clone, Copy, Debug)]
pub enum Setting {
	// Tab-specific, also saved as the default for new tabs
	ShowHidden,
	Linemode,
	TabName,
	SortBy,
	SortReverse,
	SortDirFirst,
	SortSensitive,
	SortTranslit,
	SortFallback,

	// Theme
	Selection,

	Global(&'static Global),
}

#[derive(Clone, Copy, Debug)]
pub enum SettingKind {
	Toggle,
	Select(&'static [&'static str]),
	Text,
}

/// A setting in `yazi.toml` that isn't tab-specific.
#[derive(Debug)]
pub struct Global {
	table: &'static str,
	key:   &'static str,
	name:  &'static str,
	kind:  SettingKind,
	live:  bool,
	get:   fn() -> String,
	/// Validates the input, applies it if `live`, and returns the value to save.
	set:   fn(&str) -> Result<Value>,
}

impl Setting {
	pub fn all() -> impl Iterator<Item = Self> {
		[
			Self::ShowHidden,
			Self::Linemode,
			Self::TabName,
			Self::SortBy,
			Self::SortReverse,
			Self::SortDirFirst,
			Self::SortSensitive,
			Self::SortTranslit,
			Self::SortFallback,
			Self::Selection,
		]
		.into_iter()
		.chain(GLOBALS.iter().map(Self::Global))
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::ShowHidden => "Settings: Show hidden files",
			Self::Linemode => "Settings: Linemode",
			Self::TabName => "Settings: Tab name",
			Self::SortBy => "Settings: Sort by",
			Self::SortReverse => "Settings: Sort in reverse",
			Self::SortDirFirst => "Settings: Sort directories first",
			Self::SortSensitive => "Settings: Sort case-sensitively",
			Self::SortTranslit => "Settings: Sort with transliteration",
			Self::SortFallback => "Settings: Sort fallback",
			Self::Selection => "Settings: Selection style",
			Self::Global(g) => g.name,
		}
	}

	pub fn kind(self) -> SettingKind {
		match self {
			Self::ShowHidden
			| Self::SortReverse
			| Self::SortDirFirst
			| Self::SortSensitive
			| Self::SortTranslit => SettingKind::Toggle,
			Self::Linemode => SettingKind::Select(LINEMODES),
			Self::TabName => SettingKind::Text,
			Self::SortBy => SettingKind::Select(SortBy::VARIANTS),
			Self::SortFallback => SettingKind::Select(SortFallback::VARIANTS),
			Self::Selection => SettingKind::Select(&["rounded", "rectangular"]),
			Self::Global(g) => g.kind,
		}
	}

	pub fn value(self, pref: &Preference) -> Cow<'_, str> {
		match self {
			Self::ShowHidden => toggle(pref.show_hidden).into(),
			Self::Linemode => pref.linemode.as_str().into(),
			Self::TabName => pref.name.as_str().into(),
			Self::SortBy => pref.sort_by.into_str().into(),
			Self::SortReverse => toggle(pref.sort_reverse).into(),
			Self::SortDirFirst => toggle(pref.sort_dir_first).into(),
			Self::SortSensitive => toggle(pref.sort_sensitive).into(),
			Self::SortTranslit => toggle(pref.sort_translit).into(),
			Self::SortFallback => pref.sort_fallback.into_str().into(),
			Self::Selection => {
				let padding = THEME.indicator.padding.load();
				if padding.open == ROUNDED.0 && padding.close == ROUNDED.1 {
					"rounded"
				} else if padding.open.trim().is_empty() && padding.close.trim().is_empty() {
					"rectangular"
				} else {
					"custom"
				}
				.into()
			}
			Self::Global(g) => PENDING.lock().get(g.name).cloned().unwrap_or_else(g.get).into(),
		}
	}

	pub fn apply(self, value: &str) {
		let on = value == toggle(true);
		let (action, key, saved): (_, _, Value) = match self {
			Self::Global(g) => return g.apply(value),
			Self::TabName => return emit!(Call(relay!(mgr:tab_rename, [value]))),
			Self::Selection => return Self::select(value == "rectangular"),
			Self::ShowHidden => {
				YAZI.mgr.show_hidden.set(on);
				(relay!(mgr:hidden, [if on { "show" } else { "hide" }]), "show_hidden", on.into())
			}
			Self::Linemode => {
				YAZI.mgr.linemode.store(Arc::new(value.to_owned()));
				(relay!(mgr:linemode, [value]), "linemode", value.into())
			}
			Self::SortBy => {
				YAZI.mgr.sort_by.set(value.parse().unwrap_or_default());
				(relay!(mgr:sort, [value]), "sort_by", value.into())
			}
			Self::SortReverse => {
				YAZI.mgr.sort_reverse.set(on);
				(relay!(mgr:sort).with("reverse", on), "sort_reverse", on.into())
			}
			Self::SortDirFirst => {
				YAZI.mgr.sort_dir_first.set(on);
				(relay!(mgr:sort).with("dir_first", on), "sort_dir_first", on.into())
			}
			Self::SortSensitive => {
				YAZI.mgr.sort_sensitive.set(on);
				(relay!(mgr:sort).with("sensitive", on), "sort_sensitive", on.into())
			}
			Self::SortTranslit => {
				YAZI.mgr.sort_translit.set(on);
				(relay!(mgr:sort).with("translit", on), "sort_translit", on.into())
			}
			Self::SortFallback => {
				YAZI.mgr.sort_fallback.set(value.parse().unwrap_or_default());
				(relay!(mgr:sort).with("fallback", value), "sort_fallback", value.into())
			}
		};

		emit!(Call(action));
		emit!(Call(relay!(app:resize)));
		tokio::spawn(async move {
			if let Err(e) = Yazi::persist("mgr", key, saved).await {
				notify(MessageLevel::Error, format!("Failed to save `mgr.{key}`: {e:#}"));
			}
		});
	}
}

impl Setting {
	/// Saves the hovered file's padding to `theme.toml`, then reloads the theme to apply it.
	/// Saves the edge style to `theme.toml`, then reloads the theme to apply it.
	fn select(rectangular: bool) {
		let (open, close) = if rectangular { (" ", " ") } else { ROUNDED };
		let edge =
			move || Value::InlineTable(InlineTable::from_iter([("open", open), ("close", close)]));

		tokio::spawn(async move {
			for (table, key) in EDGES {
				if let Err(e) = Theme::persist(table, key, edge()).await {
					return notify(MessageLevel::Error, format!("Failed to save `{table}.{key}`: {e:#}"));
				}
			}
			emit!(Call(relay!(app:theme)));
		});
	}
}

impl Global {
	fn apply(&'static self, input: &str) {
		let input = input.trim().to_owned();
		let value = match (self.set)(&input) {
			Ok(value) => value,
			Err(e) => {
				return notify(MessageLevel::Error, format!("Invalid `{}.{}`: {e}", self.table, self.key));
			}
		};

		if self.live {
			// Reflow, so the layout and Lua see the new value
			emit!(Call(relay!(app:resize)));
		}

		tokio::spawn(async move {
			if let Err(e) = Yazi::persist(self.table, self.key, value).await {
				notify(MessageLevel::Error, format!("Failed to save `{}.{}`: {e:#}", self.table, self.key));
			} else if !self.live {
				PENDING.lock().insert(self.name, input);
				notify(MessageLevel::Warn, format!("Restart Yazi to apply `{}.{}`", self.table, self.key));
			}
		});
	}
}

impl SettingKind {
	pub fn options(self) -> &'static [&'static str] {
		match self {
			Self::Toggle => &["off", "on"],
			Self::Select(options) => options,
			Self::Text => &[],
		}
	}
}

/// Theme keys whose `open`/`close` glyphs round the edges of the hovered file, tabs, and status bar.
const EDGES: [(&str, &str); 5] = [
	("indicator", "padding"),
	("tabs", "sep_inner"),
	("tabs", "sep_outer"),
	("status", "sep_left"),
	("status", "sep_right"),
];

/// The half-circle glyphs of the rounded style.
const ROUNDED: (&str, &str) = ("\u{e0b6}", "\u{e0b4}");

const LINEMODES: &[&str] = &["none", "size", "btime", "mtime", "permissions", "owner"];

fn toggle(state: bool) -> &'static str { SettingKind::Toggle.options()[state as usize] }

fn parse_toggle(s: &str) -> Result<bool> {
	match s {
		"on" | "true" | "yes" => Ok(true),
		"off" | "false" | "no" => Ok(false),
		_ => bail!("expected on or off, got {s:?}"),
	}
}

/// Parses a list like `1, 4, 3` or `1 4 3`.
fn parse_list<T>(s: &str, len: usize) -> Result<Vec<T>>
where
	T: FromStr,
	T::Err: std::error::Error + Send + Sync + 'static,
{
	let items: Vec<T> =
		s.split([',', ' ']).filter(|s| !s.is_empty()).map(str::parse).collect::<Result<_, _>>()?;

	if items.len() != len {
		bail!("expected {len} values, got {}", items.len());
	}
	Ok(items)
}

fn join<T: ToString>(items: impl IntoIterator<Item = T>) -> String {
	items.into_iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ")
}

fn notify(level: MessageLevel, content: String) {
	let opt =
		MessageOpt { title: "Settings".to_owned(), content, level, timeout: Duration::from_secs(5) };
	emit!(Call(relay!(notify:push).with_any("opt", opt)));
}

fn mouse(flag: MouseEvents, input: &str) -> Result<Value> {
	let mut events = YAZI.mgr.mouse_events.get();
	events.set(flag, parse_toggle(input)?);
	YAZI.mgr.mouse_events.set(events);
	Ok(Value::Array(Array::from_iter(Vec::<String>::from(events))))
}

macro_rules! restart_int {
	($table:ident. $key:ident : $ty:ty, $name:literal) => {
		Global {
			table: stringify!($table),
			key:   stringify!($key),
			name:  concat!("Settings: ", $name, " (restart)"),
			kind:  SettingKind::Text,
			live:  false,
			get:   || YAZI.$table.$key.to_string(),
			set:   |s| Ok(Value::from(s.parse::<$ty>()?.to_string().parse::<i64>()?)),
		}
	};
}

macro_rules! restart_bool {
	($table:ident. $key:ident, $name:literal) => {
		Global {
			table: stringify!($table),
			key:   stringify!($key),
			name:  concat!("Settings: ", $name, " (restart)"),
			kind:  SettingKind::Toggle,
			live:  false,
			get:   || toggle(YAZI.$table.$key).to_owned(),
			set:   |s| Ok(Value::from(parse_toggle(s)?)),
		}
	};
}

macro_rules! live_bool {
	($table:ident. $key:ident, $name:literal) => {
		Global {
			table: stringify!($table),
			key:   stringify!($key),
			name:  concat!("Settings: ", $name),
			kind:  SettingKind::Toggle,
			live:  true,
			get:   || toggle(YAZI.$table.$key.get()).to_owned(),
			set:   |s| {
				YAZI.$table.$key.set(parse_toggle(s)?);
				Ok(Value::from(YAZI.$table.$key.get()))
			},
		}
	};
}

macro_rules! mouse_event {
	($flag:ident, $name:literal) => {
		Global {
			table: "mgr",
			key:   "mouse_events",
			name:  concat!("Settings: Mouse events: ", $name),
			kind:  SettingKind::Toggle,
			live:  true,
			get:   || toggle(YAZI.mgr.mouse_events.get().contains(MouseEvents::$flag)).to_owned(),
			set:   |s| mouse(MouseEvents::$flag, s),
		}
	};
}

static GLOBALS: &[Global] = &[
	// Input
	live_bool!(input.vim_mode, "Vim-style input"),
	live_bool!(input.cursor_blink, "Input cursor blink"),
	// Manager
	Global {
		table: "mgr",
		key:   "ratio",
		name:  "Settings: Layout ratio",
		kind:  SettingKind::Text,
		live:  true,
		get:   || join(<[u16; 3]>::from(YAZI.mgr.ratio.get())),
		set:   |s| {
			let list = parse_list::<u16>(s, 3)?;
			YAZI.mgr.ratio.set(MgrRatio::try_from([list[0], list[1], list[2]])?);
			Ok(Value::Array(list.into_iter().map(i64::from).collect()))
		},
	},
	Global {
		table: "mgr",
		key:   "scrolloff",
		name:  "Settings: Scroll offset",
		kind:  SettingKind::Text,
		live:  true,
		get:   || YAZI.mgr.scrolloff.get().to_string(),
		set:   |s| {
			YAZI.mgr.scrolloff.set(s.parse()?);
			Ok(Value::from(i64::from(YAZI.mgr.scrolloff.get())))
		},
	},
	live_bool!(mgr.show_symlink, "Show symlink targets"),
	live_bool!(mgr.show_icons, "Show file icons"),
	live_bool!(mgr.show_borders, "Show column borders"),
	live_bool!(mgr.archive_7z_native, "7z random access and in-place editing"),
	mouse_event!(CLICK, "click"),
	mouse_event!(SCROLL, "scroll"),
	mouse_event!(TOUCH, "touch"),
	mouse_event!(MOVE, "move"),
	mouse_event!(DRAG, "drag"),
	// Preview
	Global {
		table: "preview",
		key:   "wrap",
		name:  "Settings: Preview wrap (restart)",
		kind:  SettingKind::Select(&["no", "yes"]),
		live:  false,
		get:   || {
			let wrap: Option<ratatui_widgets::paragraph::Wrap> = YAZI.preview.wrap.into();
			if wrap.is_some() { "yes" } else { "no" }.to_owned()
		},
		set:   |s| Ok(Value::from(s)),
	},
	restart_int!(preview.tab_size: u8, "Preview tab size"),
	restart_int!(preview.max_width: u16, "Preview max width"),
	restart_int!(preview.max_height: u16, "Preview max height"),
	Global {
		table: "preview",
		key:   "cache_dir",
		name:  "Settings: Preview cache directory (restart)",
		kind:  SettingKind::Text,
		live:  false,
		get:   || YAZI.preview.cache_dir.to_string_lossy().into_owned(),
		set:   |s| Ok(Value::from(s)),
	},
	restart_int!(preview.image_delay: u8, "Preview image delay (ms)"),
	Global {
		table: "preview",
		key:   "image_filter",
		name:  "Settings: Preview image filter (restart)",
		kind:  SettingKind::Select(&["nearest", "triangle", "catmull-rom", "gaussian", "lanczos3"]),
		live:  false,
		get:   || YAZI.preview.image_filter.clone(),
		set:   |s| Ok(Value::from(s)),
	},
	restart_int!(preview.image_quality: u8, "Preview image quality"),
	Global {
		table: "preview",
		key:   "ueberzug_scale",
		name:  "Settings: Überzug++ scale (restart)",
		kind:  SettingKind::Text,
		live:  false,
		get:   || YAZI.preview.ueberzug_scale.to_string(),
		set:   |s| Ok(Value::from(f64::from(s.parse::<f32>()?))),
	},
	Global {
		table: "preview",
		key:   "ueberzug_offset",
		name:  "Settings: Überzug++ offset (restart)",
		kind:  SettingKind::Text,
		live:  false,
		get:   || {
			let (x, y, w, h) = YAZI.preview.ueberzug_offset;
			join([x, y, w, h])
		},
		set:   |s| Ok(Value::Array(parse_list::<f32>(s, 4)?.into_iter().map(f64::from).collect())),
	},
	// Tasks
	restart_int!(tasks.file_workers: NonZeroU8, "File workers"),
	restart_int!(tasks.plugin_workers: NonZeroU8, "Plugin workers"),
	restart_int!(tasks.fetch_workers: NonZeroU8, "Fetch workers"),
	restart_int!(tasks.preload_workers: NonZeroU8, "Preload workers"),
	restart_int!(tasks.process_workers: NonZeroU8, "Process workers"),
	restart_int!(tasks.bizarre_retry: NonZeroU8, "Bizarre retry count"),
	restart_int!(tasks.image_alloc: u32, "Image memory limit"),
	Global {
		table: "tasks",
		key:   "image_bound",
		name:  "Settings: Image size bound (restart)",
		kind:  SettingKind::Text,
		live:  false,
		get:   || join(YAZI.tasks.image_bound),
		set:   |s| Ok(Value::Array(parse_list::<u16>(s, 2)?.into_iter().map(i64::from).collect())),
	},
	restart_bool!(tasks.suppress_preload, "Suppress preload"),
	// Which
	Global {
		table: "which",
		key:   "sort_by",
		name:  "Settings: Which-key sort by (restart)",
		kind:  SettingKind::Select(&["none", "key", "desc"]),
		live:  false,
		get:   || {
			use yazi_config::which::SortBy as S;
			match YAZI.which.sort_by {
				S::None => "none",
				S::Key => "key",
				S::Desc => "desc",
			}
			.to_owned()
		},
		set:   |s| Ok(Value::from(s)),
	},
	restart_bool!(which.sort_sensitive, "Which-key sort case-sensitive"),
	restart_bool!(which.sort_reverse, "Which-key sort in reverse"),
	restart_bool!(which.sort_translit, "Which-key sort translit"),
];
