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

impl Widget for Entries<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let entries = self.core.palette.window();
		if entries.is_empty() {
			return;
		}

		// Label
		let col1: Vec<_> = entries.iter().map(|e| ListItem::new(e.label())).collect();

		// Setting value, or chord
		let pref = &self.core.active().pref;
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
