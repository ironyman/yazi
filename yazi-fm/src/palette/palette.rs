use ratatui_core::{buffer::Buffer, layout::{Alignment, Constraint, Layout, Rect}, symbols::merge::MergeStrategy, text::Line, widgets::Widget};
use ratatui_widgets::{block::{Block, Padding}, borders::BorderType};
use yazi_config::{KEYMAP, THEME};
use yazi_core::{Core, palette::PaletteMode};
use yazi_shared::{Layer, event::Action};
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
			.title(palette.title())
			.title_alignment(Alignment::Center)
			.title_bottom(Line::styled(Self::hints(palette.view().0), THEME.palette.chord.get()))
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

impl Palette<'_> {
	fn hints(mode: PaletteMode) -> String {
		let submit = Self::key(|a| a.name == "close" && a.bool("submit") && !a.bool("pager"));
		let pager = Self::key(|a| a.name == "close" && a.bool("pager"));
		let complete = Self::key(|a| a.name == "complete");
		let prev = Self::key(|a| a.name == "arrow" && a.first::<&str>().is_ok_and(|s| s == "prev"));
		let next = Self::key(|a| a.name == "arrow" && a.first::<&str>().is_ok_and(|s| s == "next"));
		let close = Self::key(|a| a.name == "escape");

		let (submit_desc, complete_desc, prefixes) = match mode {
			PaletteMode::Command => ("run", "complete/cycle", "! shell"),
			PaletteMode::File => ("open", "complete", "> commands  ! shell"),
			PaletteMode::Shell => ("run", "complete", ""),
		};

		let hints = [
			submit.map(|k| format!("{k} {submit_desc}")),
			pager.filter(|_| mode == PaletteMode::Shell).map(|k| format!("{k} show output")),
			complete.map(|k| format!("{k} {complete_desc}")),
			prev.zip(next).map(|(p, n)| format!("{p}/{n} select")),
			close.map(|k| format!("{k} close")),
			Some(prefixes.to_owned()).filter(|s| !s.is_empty()),
		];
		format!(" {} ", hints.into_iter().flatten().collect::<Vec<_>>().join("  "))
	}

	/// The key bound to the action, preferring special keys like `<Up>` over characters typed into the filter.
	fn key(f: impl Fn(&Action) -> bool) -> Option<String> {
		let chords = KEYMAP.chords(Layer::Palette);
		let mut it = chords.iter().filter(|c| matches!(&c.run[..], [a] if f(a))).map(|c| c.on());

		let first = it.next()?;
		let key =
			if first.starts_with('<') { first } else { it.find(|k| k.starts_with('<')).unwrap_or(first) };

		Some(match key.as_str() {
			"<Enter>" => "↵".to_owned(),
			"<C-Enter>" => "^↵".to_owned(),
			"<Tab>" => "⇥".to_owned(),
			"<Up>" => "↑".to_owned(),
			"<Down>" => "↓".to_owned(),
			"<Esc>" => "esc".to_owned(),
			_ => key,
		})
	}
}
