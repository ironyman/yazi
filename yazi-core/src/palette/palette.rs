use std::{cmp::Reverse, path::{Path, PathBuf, is_separator}, sync::mpsc};

use anyhow::Result;
use ratatui_widgets::block::Padding;
use yazi_binding::position::Position;
use yazi_fs::{engine::local::try_absolute, path::expand_url};
use yazi_macro::render;
use yazi_shared::{id::Id, url::{UrlBuf, UrlLike}};
use yazi_term::event::KeyEvent;
use yazi_tty::sequence::SetCursorStyle;
use yazi_widgets::{Scrollable, input::Input};

use super::{Entry, PaletteFiles, PaletteMode, Search, fuzzy};
use crate::mgr::Recent;

#[derive(Default)]
pub struct Palette {
	pub visible:  bool,
	pub mode:     PaletteMode,
	pub position: Position,
	pub entries:  Vec<Entry>,

	// Sources
	pub matched:  Vec<Entry>,
	pub history:  Vec<String>,
	pub recents:  Vec<Recent>,
	pub cwd:      Option<PathBuf>,
	pub scope:    Option<(PathBuf, String)>,
	pub scanning: bool,
	pub ticket:   Id,
	pub query:    Option<mpsc::Sender<String>>,

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
		let changed = self.keyword != kw;
		if changed {
			self.keyword = kw.to_owned();
			(self.offset, self.cursor) = (0, 0);
		}

		if let (PaletteMode::File, q) = self.view() {
			let q = q.trim().to_owned();
			let scope = Self::scope(&q).or_else(|| Some((self.cwd.clone()?, String::new())));

			let respawned = scope != self.scope;
			if respawned {
				self.search(scope);
			}
			if let Some(tx) = self.query.as_ref().filter(|_| changed || respawned) {
				tx.send(q).ok();
			}
		}

		self.entries = self.matches();
		self.scroll(0);
		render!();
	}

	fn search(&mut self, scope: Option<(PathBuf, String)>) {
		self.ticket = Id::unique();
		self.matched.clear();
		self.query = scope.clone().map(|(root, anchor)| Search::spawn(root, anchor, self.ticket));
		self.scanning = self.query.is_some();
		self.scope = scope;
	}

	/// The directory to search, if the query starts with a home-relative or absolute one,
	/// along with that directory as typed, which prefixes every result.
	fn scope(q: &str) -> Option<(PathBuf, String)> {
		let anchor = &q[..=q.rfind(is_separator)?];
		if !anchor.starts_with('~') && !Path::new(anchor).is_absolute() {
			return None;
		}

		let url = try_absolute(expand_url(UrlBuf::from(Path::new(anchor))))?;
		Some((url.as_local()?.to_owned(), anchor.to_owned()))
	}

	pub fn update_files(&mut self, files: PaletteFiles) {
		if files.ticket != self.ticket || self.query.is_none() {
			return;
		}

		self.matched = files.entries;
		self.scanning = !files.done;
		if self.view().0 == PaletteMode::File {
			self.filter_apply();
		}
	}

	pub fn close(&mut self) {
		self.visible = false;
		self.scanning = false;
		self.scope = None;
		self.query = None;
		self.matched = vec![];
	}

	/// The effective mode, after applying any `!` or `>` prefix, and the query without it.
	pub fn view(&self) -> (PaletteMode, &str) {
		let kw = self.keyword.as_str();
		if let Some(q) = kw.strip_prefix('!') {
			(PaletteMode::Shell, q)
		} else if let Some(q) = kw.strip_prefix('>') {
			(PaletteMode::Command, q)
		} else {
			(self.mode, kw)
		}
	}

	pub fn prefix(&self) -> &str {
		let (_, q) = self.view();
		&self.keyword[..self.keyword.len() - q.len()]
	}

	fn matches(&self) -> Vec<Entry> {
		match self.view() {
			(PaletteMode::Command, q) => Entry::commands(q.trim()),
			(PaletteMode::File, _) => self.matched.clone(),
			(PaletteMode::Shell, q) => Self::shell(q.trim(), &self.history),
			(PaletteMode::Recents, q) => Self::recents(q.trim(), &self.recents),
		}
	}

	fn shell(cmd: &str, history: &[String]) -> Vec<Entry> {
		let typed = Some(cmd).filter(|c| !c.is_empty());
		let recent = history.iter().rev().filter(|&h| h != cmd && h.contains(cmd));

		typed.into_iter().map(Into::into).chain(recent.cloned()).map(Entry::Shell).collect()
	}

	/// Recent places fuzzy-matching `q`, best first, keeping the most recent first among equals.
	fn recents(q: &str, recents: &[Recent]) -> Vec<Entry> {
		let mut scored: Vec<_> =
			recents.iter().filter_map(|r| Some((fuzzy(&r.url.to_string(), q)?, r))).collect();

		scored.sort_by_key(|&(score, _)| Reverse(score));
		scored.into_iter().map(|(_, r)| Entry::Recent(r.clone())).collect()
	}
}

impl Palette {
	pub fn padding(&self) -> Padding { Padding::new(1, 1, 1, 1) }

	pub fn title(&self) -> &'static str {
		match self.view().0 {
			PaletteMode::Command => "Command Palette",
			PaletteMode::File if self.scanning => "Go to File (scanning...)",
			PaletteMode::File => "Go to File",
			PaletteMode::Shell => "Shell",
			PaletteMode::Recents => "Recent Places",
		}
	}

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
