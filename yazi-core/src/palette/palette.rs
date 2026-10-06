use anyhow::Result;
use ratatui_widgets::block::Padding;
use yazi_binding::position::Position;
use yazi_macro::render;
use yazi_term::event::KeyEvent;
use yazi_tty::sequence::SetCursorStyle;
use yazi_widgets::{Scrollable, input::Input};

use super::Entry;

#[derive(Default)]
pub struct Palette {
	pub visible:  bool,
	pub position: Position,
	pub entries:  Vec<Entry>,

	// Filter
	pub input:   Input,
	pub keyword: String,

	pub offset: usize,
	pub cursor: usize,
	pub height: u16,
}

impl Palette {
	pub fn r#type(&mut self, key: &KeyEvent) -> Result<bool> {
		if !self.input.r#type(key)? {
			return Ok(false);
		}

		self.filter_apply();
		Ok(true)
	}

	pub fn filter_apply(&mut self) {
		let kw = self.input.value();

		if kw.is_empty() {
			self.keyword.clear();
			self.entries = Entry::all();
		} else if self.keyword != kw {
			self.keyword = kw.to_owned();
			self.entries = Self::filter_entries(Entry::all(), kw);
		}

		render!(self.scroll(0));
	}

	fn filter_entries(entries: Vec<Entry>, kw: &str) -> Vec<Entry> {
		let kw = kw.to_lowercase();
		entries.into_iter().filter(|e| e.label().to_lowercase().contains(&kw)).collect()
	}
}

impl Palette {
	pub fn padding(&self) -> Padding { Padding::new(1, 1, 1, 1) }

	// --- Entries
	pub fn window(&self) -> &[Entry] {
		let end = (self.offset + self.limit()).min(self.entries.len());
		&self.entries[self.offset..end]
	}

	pub fn hovered(&self) -> Option<&Entry> { self.entries.get(self.cursor) }

	// --- Cursor
	pub(crate) fn cursor(&self) -> Option<u16> { self.visible.then_some(self.input.cursor()) }

	pub fn rel_cursor(&self) -> usize { self.cursor - self.offset }

	pub(crate) fn cursor_shape(&self) -> Option<SetCursorStyle> {
		self.visible.then_some(self.input.cursor_shape())
	}
}

impl Scrollable for Palette {
	fn total(&self) -> usize { self.entries.len() }

	fn limit(&self) -> usize {
		let p = self.padding();
		self.height.saturating_sub(p.top + /* input */ 1 + /* divider */ 1 + p.bottom) as usize
	}

	fn cursor_mut(&mut self) -> &mut usize { &mut self.cursor }

	fn offset_mut(&mut self) -> &mut usize { &mut self.offset }
}
