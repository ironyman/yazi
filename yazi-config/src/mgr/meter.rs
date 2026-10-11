use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr, VariantNames};

/// A system resource shown in the header, see `mgr.system_meters`.
#[derive(
	Clone,
	Copy,
	Debug,
	Deserialize,
	EnumString,
	Eq,
	IntoStaticStr,
	Ord,
	PartialEq,
	PartialOrd,
	Serialize,
	VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum Meter {
	Cpu,
	Ram,
	Disk,
	Net,
}

impl Meter {
	pub const ALL: [Self; 4] = [Self::Cpu, Self::Ram, Self::Disk, Self::Net];
}
