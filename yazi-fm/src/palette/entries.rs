use std::time::{SystemTime, UNIX_EPOCH};

use ratatui_core::{buffer::Buffer, layout::{self, Alignment, Constraint, Rect}, text::Line, widgets::Widget};
use ratatui_widgets::list::{List, ListItem};
use yazi_config::THEME;
use yazi_core::{Core, palette::{Entry, SettingKind}};

pub(super) struct Entries<'a> {
	core: &'a Core,
}

impl<'a> Entries<'a> {
	pub(super) fn new(core: &'a Core) -> Self { Self { core } }
}

impl Entries<'_> {
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
			})
			.collect();

		let chunks =
			layout::Layout::horizontal([Constraint::Fill(1), Constraint::Length(24)]).split(area);

		let cursor = self.core.palette.rel_cursor() as u16;
		buf.set_style(
			Rect { x: area.x, y: area.y + cursor, width: area.width, height: 1 },
			THEME.palette.hovered.get(),
		);

		List::new(col1).render(chunks[0], buf);
		List::new(col2).render(chunks[1], buf);
	}
}
