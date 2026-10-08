use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PaletteMode {
	#[default]
	Command,
	File,
	Shell,
	Recents,
	Yanked,
	Marked,
	Archive,
}
