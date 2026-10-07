use ansi_to_tui::IntoText;
use ratatui_core::{buffer::Buffer, layout::{Alignment, Rect}, text::{Line, Text}, widgets::Widget};
use ratatui_widgets::{block::Block, borders::BorderType, paragraph::Paragraph};
use yazi_config::THEME;
use yazi_core::Core;

pub(crate) struct Pager<'a> {
	core: &'a Core,
}

impl<'a> Pager<'a> {
	pub fn new(core: &'a Core) -> Self { Self { core } }
}

impl Widget for Pager<'_> {
	fn render(self, _: Rect, buf: &mut Buffer) {
		let pager = &self.core.pager;
		let area = self.core.mgr.area(pager.position);

		yazi_widgets::clear::Clear::default().render(area, buf);

		let lines = &pager.lines;
		let end = (pager.offset + pager.limit()).min(lines.len());
		let text: Text = lines[pager.offset.min(end)..end]
			.iter()
			.flat_map(|l| l.as_bytes().into_text().map_or_else(|_| vec![Line::raw(l)], |t| t.lines))
			.collect();

		let position = format!(" {}/{} ", end, lines.len());
		Paragraph::new(text)
			.block(
				Block::bordered()
					.title(Line::styled(format!(" {} ", pager.title), THEME.pager.title.get()))
					.title_bottom(Line::raw(position).alignment(Alignment::Right))
					.border_type(BorderType::Rounded)
					.border_style(THEME.pager.border.get()),
			)
			.render(area, buf);
	}
}
