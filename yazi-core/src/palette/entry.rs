use std::borrow::Cow;

use hashbrown::HashSet;
use yazi_config::{KEYMAP, keymap::ChordArc};
use yazi_shared::{Layer, url::UrlBuf};

use super::Setting;
use crate::mgr::Recent;

#[derive(Clone, Debug)]
pub enum Entry {
	Setting(Setting),
	Chord(ChordArc),
	Action(&'static str),
	Run(String),
	File { url: UrlBuf, label: String, dir: bool },
	Shell(String),
	Recent(Recent),
}

/// Every user-facing `mgr` action, so those without a key binding can be run too.
pub const ACTIONS: &[&str] = &[
	"arrow",
	"back",
	"bulk_create",
	"bulk_rename",
	"cd",
	"close",
	"copy",
	"create",
	"download",
	"enter",
	"escape",
	"filter",
	"find",
	"find_arrow",
	"follow",
	"forward",
	"hardlink",
	"help",
	"hidden",
	"leave",
	"link",
	"linemode",
	"open",
	"palette",
	"paste",
	"peek",
	"plugin",
	"quit",
	"refresh",
	"remove",
	"rename",
	"reveal",
	"seek",
	"shell",
	"sort",
	"spot",
	"suspend",
	"tab_close",
	"tab_create",
	"tab_rename",
	"tab_swap",
	"tab_switch",
	"toggle",
	"toggle_all",
	"unyank",
	"upload",
	"visual_mode",
	"yank",
];

impl Entry {
	/// Commands matching `kw`, led by `kw` itself when it reads as a command line, e.g. `sort mtime --reverse`.
	pub fn commands(kw: &str) -> Vec<Self> {
		let name = kw.split_whitespace().next().unwrap_or_default();
		let run = (name.contains(':') || ACTIONS.contains(&name)).then(|| Self::Run(kw.to_owned()));

		let mut seen = HashSet::new();
		let chords: Vec<_> = KEYMAP
			.chords(Layer::Mgr)
			.iter()
			.filter(|&c| seen.insert(c.desc_or_run().into_owned()))
			.cloned()
			.map(Self::Chord)
			.collect();

		let lowercased = kw.to_lowercase();
		let matched = Setting::all()
			.map(Self::Setting)
			.chain(chords)
			.chain(ACTIONS.iter().map(|&a| Self::Action(a)))
			.filter(|e| e.label().to_lowercase().contains(&lowercased));

		run.into_iter().chain(matched).collect()
	}

	pub fn label(&self) -> Cow<'_, str> {
		match self {
			Self::Setting(s) => s.name().into(),
			Self::Chord(c) => c.desc_or_run(),
			Self::Action(a) => (*a).into(),
			Self::Run(cmd) | Self::Shell(cmd) => cmd.into(),
			Self::File { label, .. } => label.into(),
			Self::Recent(r) => r.url.to_string().into(),
		}
	}
}
