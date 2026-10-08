use std::time::{SystemTime, UNIX_EPOCH};

use ratatui_core::{buffer::Buffer, layout::{self, Alignment, Constraint, Rect}, text::Line, widgets::Widget};
use ratatui_widgets::list::{List, ListItem};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use yazi_config::THEME;
use yazi_core::{Core, palette::{Entry, PaletteMode, SettingKind}};
use yazi_shared::url::UrlLike;

pub(super) struct Entries<'a> {
	core: &'a Core,
}

impl<'a> Entries<'a> {
	pub(super) fn new(core: &'a Core) -> Self { Self { core } }
}

impl Entries<'_> {
	/// Shortens `s` to at most `max` columns by cutting its start, so the end
	/// of a path stays visible.
	fn truncate_left(s: &str, max: usize) -> String {
		if s.width() <= max {
			return s.to_owned();
		}

		let mut width = 1;
		let mut start = s.len();
		for (i, c) in s.char_indices().rev() {
			width += c.width().unwrap_or(0);
			if width > max {
				break;
			}
			start = i;
		}
		format!("…{}", &s[start..])
	}

	/// How long ago `at`, in seconds since the Unix epoch, was, e.g. `5m` or `3d`.
	fn ago(at: u64) -> String {
		let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
		match now.saturating_sub(at) {
			s @ ..60 => format!("{s}s"),
			s @ ..3600 => format!("{}m", s / 60),
			s @ ..86400 => format!("{}h", s / 3600),
			s => format!("{}d", s / 86400),
		}
	}
}

impl Widget for Entries<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let entries = self.core.palette.window();
		if entries.is_empty() {
			return;
		}

		// Label
		let col1: Vec<_> = entries.iter().map(|e| ListItem::new(e.label())).collect();

		// Yanked files show their directories in a wider second column
		let wide = matches!(
			self.core.palette.view().0,
			PaletteMode::Yanked | PaletteMode::Marked | PaletteMode::Archive
		);
		let chunks = if wide {
			let name = entries.iter().map(|e| e.label().width()).max().unwrap_or(0) as u16 + 2;
			layout::Layout::horizontal([
				Constraint::Length(name.min(area.width / 2)),
				Constraint::Fill(1),
			])
			.split(area)
		} else {
			layout::Layout::horizontal([Constraint::Fill(1), Constraint::Length(24)]).split(area)
		};

		// Setting value, chord, or kind
		let (pref, history) = (&self.core.active().pref, &self.core.palette.history);
		let col2: Vec<_> = entries
			.iter()
			.map(|e| match e {
				Entry::Setting(s) => {
					let value = s.value(pref);
					let text = match s.kind() {
						SettingKind::Toggle => format!("[{value}]"),
						SettingKind::Select(_) => format!("‹ {value} ›"),
						SettingKind::Text => format!("\"{value}\""),
					};
					ListItem::new(Line::from(text).alignment(Alignment::Right))
						.style(THEME.palette.setting.get())
				}
				Entry::Chord(c) => ListItem::new(Line::from(c.on()).alignment(Alignment::Right))
					.style(THEME.palette.chord.get()),
				Entry::Action(_) => ListItem::new(Line::from("action").alignment(Alignment::Right))
					.style(THEME.palette.chord.get()),
				Entry::Run(_) => ListItem::new(Line::from("run").alignment(Alignment::Right))
					.style(THEME.palette.chord.get()),
				Entry::File { dir, .. } => {
					ListItem::new(Line::from(if *dir { "dir" } else { "" }).alignment(Alignment::Right))
						.style(THEME.palette.chord.get())
				}
				Entry::Shell(cmd) => {
					let kind = if history.contains(cmd) { "history" } else { "run" };
					ListItem::new(Line::from(kind).alignment(Alignment::Right))
						.style(THEME.palette.chord.get())
				}
				Entry::Recent(r) => {
					let text = format!("{}× · {}", r.count, Self::ago(r.at));
					ListItem::new(Line::from(text).alignment(Alignment::Right))
						.style(THEME.palette.chord.get())
				}
				Entry::Field(f) => {
					let text =
						Self::truncate_left(&self.core.palette.draft.display(*f), chunks[1].width as usize);
					let style =
						if f.is_choice() { THEME.palette.setting.get() } else { THEME.palette.chord.get() };
					ListItem::new(Line::from(text).alignment(Alignment::Right)).style(style)
				}
				Entry::Listed(u) => {
					let dir = u.parent().map(|p| p.to_string()).unwrap_or_default();
					let text = Self::truncate_left(&dir, chunks[1].width as usize);
					ListItem::new(Line::from(text).alignment(Alignment::Right))
						.style(THEME.palette.chord.get())
				}
			})
			.collect();

		let cursor = self.core.palette.rel_cursor() as u16;
		buf.set_style(
			Rect { x: area.x, y: area.y + cursor, width: area.width, height: 1 },
			THEME.palette.hovered.get(),
		);

		List::new(col1).render(chunks[0], buf);
		List::new(col2).render(chunks[1], buf);
	}
}
