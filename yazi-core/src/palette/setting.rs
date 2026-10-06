use std::borrow::Cow;

use strum::VariantNames;
use yazi_config::YAZI;
use yazi_fs::{SortBy, SortFallback};
use yazi_macro::{emit, relay, render};
use yazi_shim::strum::IntoStr;

use crate::tab::Preference;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Setting {
	// Display
	ShowHidden,
	Linemode,
	TabName,

	// Input
	VimMode,

	// Sorting
	SortBy,
	SortReverse,
	SortDirFirst,
	SortSensitive,
	SortTranslit,
	SortFallback,
}

#[derive(Clone, Copy, Debug)]
pub enum SettingKind {
	Toggle,
	Select(&'static [&'static str]),
	Text,
}

impl Setting {
	pub const ALL: [Self; 10] = [
		Self::ShowHidden,
		Self::Linemode,
		Self::TabName,
		Self::VimMode,
		Self::SortBy,
		Self::SortReverse,
		Self::SortDirFirst,
		Self::SortSensitive,
		Self::SortTranslit,
		Self::SortFallback,
	];

	pub fn name(self) -> &'static str {
		match self {
			Self::ShowHidden => "Settings: Show hidden files",
			Self::Linemode => "Settings: Linemode",
			Self::TabName => "Settings: Tab name",
			Self::VimMode => "Settings: Vim-style input",
			Self::SortBy => "Settings: Sort by",
			Self::SortReverse => "Settings: Sort in reverse",
			Self::SortDirFirst => "Settings: Sort directories first",
			Self::SortSensitive => "Settings: Sort case-sensitively",
			Self::SortTranslit => "Settings: Sort with transliteration",
			Self::SortFallback => "Settings: Sort fallback",
		}
	}

	pub fn kind(self) -> SettingKind {
		match self {
			Self::ShowHidden
			| Self::VimMode
			| Self::SortReverse
			| Self::SortDirFirst
			| Self::SortSensitive
			| Self::SortTranslit => SettingKind::Toggle,
			Self::Linemode => {
				SettingKind::Select(&["none", "size", "btime", "mtime", "permissions", "owner"])
			}
			Self::TabName => SettingKind::Text,
			Self::SortBy => SettingKind::Select(SortBy::VARIANTS),
			Self::SortFallback => SettingKind::Select(SortFallback::VARIANTS),
		}
	}

	pub fn value(self, pref: &Preference) -> Cow<'_, str> {
		match self {
			Self::ShowHidden => SettingKind::toggle(pref.show_hidden).into(),
			Self::Linemode => pref.linemode.as_str().into(),
			Self::TabName => pref.name.as_str().into(),
			Self::VimMode => SettingKind::toggle(YAZI.input.vim_mode.get()).into(),
			Self::SortBy => pref.sort_by.into_str().into(),
			Self::SortReverse => SettingKind::toggle(pref.sort_reverse).into(),
			Self::SortDirFirst => SettingKind::toggle(pref.sort_dir_first).into(),
			Self::SortSensitive => SettingKind::toggle(pref.sort_sensitive).into(),
			Self::SortTranslit => SettingKind::toggle(pref.sort_translit).into(),
			Self::SortFallback => pref.sort_fallback.into_str().into(),
		}
	}

	pub fn apply(self, value: &str) {
		let on = value == SettingKind::toggle(true);
		let action = match self {
			Self::VimMode => {
				YAZI.input.vim_mode.set(on);
				return render!();
			}
			Self::ShowHidden => relay!(mgr:hidden, [if on { "show" } else { "hide" }]),
			Self::Linemode => relay!(mgr:linemode, [value]),
			Self::TabName => relay!(mgr:tab_rename, [value]),
			Self::SortBy => relay!(mgr:sort, [value]),
			Self::SortReverse => relay!(mgr:sort).with("reverse", on),
			Self::SortDirFirst => relay!(mgr:sort).with("dir_first", on),
			Self::SortSensitive => relay!(mgr:sort).with("sensitive", on),
			Self::SortTranslit => relay!(mgr:sort).with("translit", on),
			Self::SortFallback => relay!(mgr:sort).with("fallback", value),
		};
		emit!(Call(action));
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

	fn toggle(state: bool) -> &'static str { Self::Toggle.options()[state as usize] }
}
