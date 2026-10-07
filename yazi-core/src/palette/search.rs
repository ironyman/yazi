use std::{cmp::Reverse, collections::VecDeque, path::{MAIN_SEPARATOR, Path, PathBuf}, sync::mpsc::{self, Receiver, TryRecvError}, time::{Duration, Instant}};

use yazi_macro::{emit, relay};
use yazi_shared::id::Id;

use super::{Entry, PaletteFiles, fuzzy};

/// Scans and matches files on a blocking thread, so neither ever stalls typing.
/// Each query sent through the channel is answered with the best matches;
/// dropping the sender stops the search.
pub(super) struct Search {
	root:    PathBuf,
	anchor:  String,
	ticket:  Id,
	files:   Vec<Entry>,
	keyword: String,
}

impl Search {
	const INTERVAL: Duration = Duration::from_millis(50);
	const LIMIT: usize = 50_000;
	const SHOWN: usize = 1_000;

	pub(super) fn spawn(root: PathBuf, anchor: String, ticket: Id) -> mpsc::Sender<String> {
		let (tx, rx) = mpsc::channel();
		let me = Self { root, anchor, ticket, files: vec![], keyword: String::new() };

		tokio::task::spawn_blocking(move || me.run(rx));
		tx
	}

	fn run(mut self, rx: Receiver<String>) {
		let mut dirs = VecDeque::from([self.root.clone()]);
		let mut published = Instant::now();

		while let Some(dir) = dirs.pop_front() {
			match Self::latest(&rx) {
				Err(()) => return,
				Ok(Some(kw)) => {
					self.keyword = kw;
					published = self.publish(false);
				}
				Ok(None) => {}
			}

			if !self.read(&dir, &mut dirs) {
				break;
			} else if published.elapsed() >= Self::INTERVAL {
				published = self.publish(false);
			}
		}

		self.publish(true);
		while let Ok(kw) = rx.recv() {
			self.keyword = Self::latest(&rx).ok().flatten().unwrap_or(kw);
			self.publish(true);
		}
	}

	/// Reads `dir` breadth-first, skipping hidden entries; returns `false` once the limit is reached.
	fn read(&mut self, dir: &Path, dirs: &mut VecDeque<PathBuf>) -> bool {
		let Ok(it) = std::fs::read_dir(dir) else { return true };
		for entry in it.flatten() {
			if entry.file_name().as_encoded_bytes().starts_with(b".") {
				continue;
			}

			let path = entry.path();
			let Ok(rel) = path.strip_prefix(&self.root).map(|p| p.to_string_lossy()) else {
				continue;
			};

			// Follow the separator the user typed, e.g. `C:/Users/` on Windows
			let label = match self.anchor.chars().last() {
				Some(sep) if sep != MAIN_SEPARATOR => {
					format!("{}{}", self.anchor, rel.replace(MAIN_SEPARATOR, sep.encode_utf8(&mut [0; 4])))
				}
				_ => format!("{}{rel}", self.anchor),
			};

			let dir = entry.file_type().is_ok_and(|t| t.is_dir());
			if dir {
				dirs.push_back(path.clone());
			}

			self.files.push(Entry::File { url: path.into(), label, dir });
			if self.files.len() >= Self::LIMIT {
				return false;
			}
		}
		true
	}

	fn publish(&self, done: bool) -> Instant {
		let mut scored: Vec<_> = self
			.files
			.iter()
			.enumerate()
			.filter_map(|(i, e)| Some((fuzzy(&e.label(), &self.keyword)?, i)))
			.collect();

		scored.sort_unstable_by_key(|&(score, i)| (Reverse(score), i));
		let entries =
			scored.into_iter().take(Self::SHOWN).map(|(_, i)| self.files[i].clone()).collect();

		let files = PaletteFiles { ticket: self.ticket, entries, done };
		emit!(Call(relay!(palette:update_files).with_any("files", files)));
		Instant::now()
	}

	/// Drains pending queries and returns the latest one; `Err` once the palette is gone.
	fn latest(rx: &Receiver<String>) -> Result<Option<String>, ()> {
		let mut latest = None;
		loop {
			match rx.try_recv() {
				Ok(kw) => latest = Some(kw),
				Err(TryRecvError::Empty) => return Ok(latest),
				Err(TryRecvError::Disconnected) => return Err(()),
			}
		}
	}
}
