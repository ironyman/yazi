use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr, VariantNames};

/// How the system meters are drawn, see `mgr.system_meters_style`.
#[derive(
	Clone,
	Copy,
	Debug,
	Default,
	Deserialize,
	EnumString,
	Eq,
	IntoStaticStr,
	PartialEq,
	Serialize,
	VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum MeterStyle {
	/// A one column bar per meter (e.g. `c ▅`), 8 levels.
	#[default]
	Bars,
	/// A braille cell per meter with the last two samples side by side (e.g. `c ⣰`), 4 levels.
	Braille,
	/// Percentages and rates as text (e.g. `c  42%`).
	Text,
}
