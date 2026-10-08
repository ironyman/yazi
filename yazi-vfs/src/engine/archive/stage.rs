use std::{io, path::PathBuf, time::SystemTime};

use hashbrown::{HashMap, HashSet};

use super::{Index, Node, NodeKind, split};

/// A staged change to one key of an archive.
#[derive(Clone, Debug)]
pub(super) enum Slot {
	Gone,
	Dir(Option<SystemTime>),
	File { source: Source, mode: Option<u32>, mtime: Option<SystemTime> },
}

#[derive(Clone, Debug)]
pub(super) enum Source {
	/// An entry of the archive, under its original key.
	Base(String),
	/// A local file holding new content.
	Blob(PathBuf),
}

/// What a key resolves to in the archive with its staged changes applied.
pub(super) enum Entry<'a> {
	Base(&'a Node),
	Slot(&'a Slot),
}

impl Entry<'_> {
	pub(super) fn is_dir(&self) -> bool {
		match self {
			Self::Base(node) => node.kind == NodeKind::Dir,
			Self::Slot(slot) => matches!(slot, Slot::Dir(_)),
		}
	}
}

#[derive(Default)]
pub(super) struct Stage {
	pub(super) slots: HashMap<String, Slot>,
	seq:              u64,
}

impl Stage {
	#[inline]
	pub(super) fn is_empty(&self) -> bool { self.slots.is_empty() }

	pub(super) fn get<'a>(&'a self, index: &'a Index, key: &str) -> Option<Entry<'a>> {
		match self.slots.get(key) {
			Some(Slot::Gone) => None,
			Some(slot) => Some(Entry::Slot(slot)),
			None => index.nodes.get(key).map(Entry::Base),
		}
	}

	pub(super) fn children(&self, index: &Index, dir: &str) -> Vec<String> {
		let base = index.children.get(dir).into_iter().flatten().map(String::as_str);
		let staged =
			self.slots.keys().filter(|k| !k.is_empty() && split(k).0 == dir).map(|k| split(k).1);

		let mut seen = HashSet::new();
		base
			.chain(staged)
			.filter(|&name| seen.insert(name) && self.get(index, &join(dir, name)).is_some())
			.map(ToOwned::to_owned)
			.collect()
	}

	pub(super) fn blob(&mut self, dir: &std::path::Path) -> PathBuf {
		self.seq += 1;
		dir.join(self.seq.to_string())
	}

	/// Makes `key` absent, recording a removal only if the archive has it.
	pub(super) fn remove(&mut self, index: &Index, key: &str) {
		if index.nodes.contains_key(key) {
			self.slots.insert(key.to_owned(), Slot::Gone);
		} else {
			self.slots.remove(key);
		}
	}

	pub(super) fn rename(&mut self, index: &Index, from: &str, to: &str) -> io::Result<()> {
		if from == to {
			return Ok(());
		} else if to.starts_with(from) && to[from.len()..].starts_with('/') {
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"Cannot move a directory into itself",
			));
		}

		let src = self.get(index, from).ok_or(io::ErrorKind::NotFound)?;
		match self.get(index, to) {
			Some(dst) if dst.is_dir() && !src.is_dir() => Err(io::ErrorKind::IsADirectory)?,
			Some(dst) if !dst.is_dir() && src.is_dir() => Err(io::ErrorKind::NotADirectory)?,
			Some(dst) if dst.is_dir() && !self.children(index, to).is_empty() => {
				Err(io::ErrorKind::DirectoryNotEmpty)?
			}
			_ => {}
		}

		let keys = self.subtree(index, from);
		let moved: Vec<_> = keys
			.iter()
			.map(|k| (format!("{to}{}", &k[from.len()..]), self.materialize(index, k)))
			.collect();
		for key in &keys {
			self.remove(index, key);
		}
		for (key, slot) in moved {
			self.put(index, key, slot);
		}
		Ok(())
	}

	/// Stages `slot` at `key`, dropping it if it equals what the archive has.
	pub(super) fn put(&mut self, index: &Index, key: String, slot: Slot) {
		let unchanged = match (&slot, index.nodes.get(&key)) {
			(Slot::Dir(_), Some(node)) => node.kind == NodeKind::Dir,
			(Slot::File { source: Source::Base(k), mode: None, mtime: None }, Some(_)) => *k == key,
			_ => false,
		};

		if unchanged {
			self.slots.remove(&key);
		} else {
			self.slots.insert(key, slot);
		}
	}

	/// `key` and everything under it, parents before children.
	pub(super) fn subtree(&self, index: &Index, key: &str) -> Vec<String> {
		let mut keys = vec![key.to_owned()];
		let mut i = 0;
		while let Some(key) = keys.get(i) {
			let children: Vec<_> = self.children(index, key).iter().map(|n| join(key, n)).collect();
			keys.extend(children);
			i += 1;
		}
		keys
	}

	/// The slot that recreates what `key` currently resolves to.
	pub(super) fn materialize(&self, index: &Index, key: &str) -> Slot {
		match self.slots.get(key) {
			Some(slot) => slot.clone(),
			None => match index.nodes.get(key) {
				Some(node) if node.kind == NodeKind::Dir => Slot::Dir(node.mtime),
				_ => Slot::File { source: Source::Base(key.to_owned()), mode: None, mtime: None },
			},
		}
	}

	pub(super) fn status(&self, index: &Index, key: &str) -> Option<&'static str> {
		let based = index.nodes.contains_key(key);
		Some(match self.slots.get(key)? {
			Slot::Gone => return None,
			Slot::Dir(_) if based => return None,
			Slot::Dir(_) => "added",
			Slot::File { source: Source::Blob(_), .. } if based => "modified",
			Slot::File { source: Source::Blob(_), .. } => "added",
			Slot::File { source: Source::Base(k), .. } if k == key => "modified",
			Slot::File { source: Source::Base(_), .. } => "moved",
		})
	}
}

pub(super) fn join(dir: &str, name: &str) -> String {
	if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") }
}
