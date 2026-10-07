use anyhow::{Context, Result};
use serde::Deserialize;
use yazi_codegen::{DeserializeOver, DeserializeOver1};
use yazi_fs::{Xdg, ok_or_not_found};
use yazi_shim::toml::DeserializeOver;

use crate::{Preset, mgr, open, opener, plugin, popup, preview, tasks, which};

#[derive(Deserialize, DeserializeOver, DeserializeOver1)]
pub struct Yazi {
	pub mgr:            mgr::Mgr,
	pub preview:        preview::Preview,
	pub opener:         opener::Opener,
	pub open:           open::Open,
	pub tasks:          tasks::Tasks,
	pub plugin:         plugin::Plugin,
	pub input:          popup::Input,
	pub(crate) confirm: popup::Confirm,
	pub pick:           popup::Pick,
	pub which:          which::Which,
}

impl Yazi {
	pub(super) fn read() -> Result<String> {
		let p = Xdg::config_dir().join("yazi.toml");
		ok_or_not_found(std::fs::read_to_string(&p))
			.with_context(|| format!("Failed to read config {p:?}"))
	}

	/// Sets `key` under `[table]` in the user's `yazi.toml`, validated with the parser used at
	/// startup, so a bad value can't break the next launch.
	pub async fn persist(table: &str, key: &str, value: impl Into<toml_edit::Value>) -> Result<()> {
		crate::persist("yazi.toml", table, key, value, |s| Ok(_ = Preset::yazi()?.deserialize_over(s)?))
			.await
	}
}
