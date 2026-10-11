use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr, VariantNames};

use super::Meter;

/// What the system meters are labelled with, see `mgr.system_meters_labels`.
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
pub enum MeterLabels {
	/// `c m d n`
	#[default]
	Letters,
	/// `CPU RAM DSK NET`
	Ascii,
	/// `▣ ▤ ▮ ⇅`
	Emoji,
	None,
}

impl MeterLabels {
	pub fn label(self, meter: Meter) -> &'static str {
		match (self, meter) {
			(Self::None, _) => "",
			(Self::Letters, Meter::Cpu) => "c",
			(Self::Letters, Meter::Ram) => "m",
			(Self::Letters, Meter::Disk) => "d",
			(Self::Letters, Meter::Net) => "n",
			(Self::Ascii, Meter::Cpu) => "CPU",
			(Self::Ascii, Meter::Ram) => "RAM",
			(Self::Ascii, Meter::Disk) => "DSK",
			(Self::Ascii, Meter::Net) => "NET",
			(Self::Emoji, Meter::Cpu) => "▣",
			(Self::Emoji, Meter::Ram) => "▤",
			(Self::Emoji, Meter::Disk) => "▮",
			(Self::Emoji, Meter::Net) => "⇅",
		}
	}
}
