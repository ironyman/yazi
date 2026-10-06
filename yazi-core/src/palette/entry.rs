use std::borrow::Cow;

use hashbrown::HashSet;
use yazi_config::{KEYMAP, keymap::ChordArc};
use yazi_shared::Layer;

use super::Setting;

#[derive(Clone)]
pub enum Entry {
	Setting(Setting),
	Chord(ChordArc),
}

impl Entry {
	pub fn all() -> Vec<Self> {
		let mut seen = HashSet::new();
		let chords = KEYMAP
			.chords(Layer::Mgr)
			.iter()
			.filter(|&c| seen.insert(c.desc_or_run().into_owned()))
			.cloned()
			.map(Self::Chord)
			.collect::<Vec<_>>();

		Setting::ALL.into_iter().map(Self::Setting).chain(chords).collect()
	}

	pub fn label(&self) -> Cow<'_, str> {
		match self {
			Self::Setting(s) => s.name().into(),
			Self::Chord(c) => c.desc_or_run(),
		}
	}
}
