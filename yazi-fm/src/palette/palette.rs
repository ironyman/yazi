use ratatui_core::{buffer::Buffer, layout::{Alignment, Constraint, Layout, Rect}, symbols::merge::MergeStrategy, widgets::Widget};
use ratatui_widgets::{block::{Block, Padding}, borders::BorderType};
use yazi_config::THEME;
use yazi_core::Core;
use yazi_shim::ratatui::Padable;

use super::Entries;

pub(crate) struct Palette<'a> {
	core: &'a Core,
}

impl<'a> Palette<'a> {
	pub fn new(core: &'a Core) -> Self { Self { core } }
}

impl Widget for Palette<'_> {
	fn render(self, _: Rect, buf: &mut Buffer) {
		let palette = &self.core.palette;
		let area = self.core.mgr.area(palette.position);
		let padding = palette.padding();

		yazi_widgets::clear::Clear::default().render(area, buf);

		Block::bordered()
			.title("Command Palette")
			.title_alignment(Alignment::Center)
			.border_type(BorderType::Rounded)
			.border_style(THEME.palette.border.get())
			.render(area, buf);

		let chunks =
			Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)])
				.split(area.padding(Padding { left: 0, right: 0, ..padding }));

		// Input
		palette.input.render(chunks[0].padding(Padding { top: 0, bottom: 0, ..padding }), buf);

		// Divider
		Block::bordered()
			.border_type(BorderType::Rounded)
			.border_style(THEME.palette.border.get())
			.merge_borders(MergeStrategy::Fuzzy)
			.render(chunks[1], buf);

		// Entries
		Entries::new(self.core)
			.render(chunks[2].padding(Padding { top: 0, bottom: 0, ..padding }), buf);
	}
}
