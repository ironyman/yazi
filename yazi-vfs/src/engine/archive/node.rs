use std::time::SystemTime;

use yazi_fs::stat::{Stat, StatKind, StatMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NodeKind {
	File,
	Dir,
	Link,
}

#[derive(Clone, Debug)]
pub(super) struct Node {
	pub(super) kind:    NodeKind,
	pub(super) len:     u64,
	pub(super) mtime:   Option<SystemTime>,
	pub(super) mode:    Option<u32>,
	/// Position of the entry in the archive, `None` for directories implied by
	/// their children.
	pub(super) ordinal: Option<usize>,
	pub(super) link:    Option<String>,
}

impl Node {
	pub(super) fn dir(mtime: Option<SystemTime>) -> Self {
		Self { kind: NodeKind::Dir, len: 0, mtime, mode: None, ordinal: None, link: None }
	}

	pub(super) fn stat(&self, name: &str) -> Stat {
		let (r#type, perm) = match self.kind {
			NodeKind::File => (StatMode::T_FILE, 0o644),
			NodeKind::Dir => (StatMode::T_DIR, 0o755),
			NodeKind::Link => (StatMode::T_LINK, 0o777),
		};

		let perm = StatMode::from_bits_truncate(self.mode.unwrap_or(perm) as u16) - StatMode::T_MASK;
		let kind =
			if cfg!(unix) && name.starts_with('.') { StatKind::HIDDEN } else { StatKind::empty() };

		Stat { kind, mode: r#type | perm, len: self.len, mtime: self.mtime, ..Default::default() }
	}
}
