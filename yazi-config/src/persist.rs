use anyhow::{Context, Result};
use yazi_fs::{Xdg, ok_or_not_found};

/// Sets `key` under `[table]` in the user's config file `name`, preserving its formatting
/// and comments, once `validate` accepts the edited content.
pub(crate) async fn persist(
	name: &str,
	table: &str,
	key: &str,
	value: impl Into<toml_edit::Value>,
	validate: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
	let p = Xdg::config_dir().join(name);
	let mut doc: toml_edit::DocumentMut = ok_or_not_found(tokio::fs::read_to_string(&p).await)
		.with_context(|| format!("Failed to read config {p:?}"))?
		.parse()?;

	doc.entry(table).or_insert(toml_edit::table())[key] = toml_edit::value(value);
	let s = doc.to_string();
	validate(&s)?;

	tokio::fs::create_dir_all(Xdg::config_dir()).await?;
	Ok(tokio::fs::write(&p, s).await?)
}
