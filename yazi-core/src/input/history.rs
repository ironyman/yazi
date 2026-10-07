use std::path::PathBuf;

use hashbrown::HashMap;
use yazi_fs::Xdg;

#[derive(Default)]
pub struct InputHistories {
	inner: HashMap<String, Vec<String>>,
}

impl InputHistories {
	pub fn load() -> Self {
		let inner = std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok());
		Self { inner: inner.unwrap_or_default() }
	}

	pub fn get(&self, group: &str) -> &[String] {
		self.inner.get(group).map(Vec::as_slice).unwrap_or(&[])
	}

	pub fn remember(&mut self, group: &str, value: &str) -> bool {
		if group.is_empty() || value.is_empty() {
			return false;
		}

		let entries = self.inner.entry_ref(group).or_default();
		if entries.last().is_some_and(|last| last == value) {
			return false;
		}

		entries.retain(|entry| entry != value);
		entries.push(value.to_owned());
		if entries.len() > 20 {
			entries.drain(..entries.len() - 20);
		}

		self.save();
		true
	}

	fn save(&self) {
		let Ok(json) = serde_json::to_vec(&self.inner) else { return };
		tokio::spawn(async move {
			let path = Self::path();
			if let Some(parent) = path.parent() {
				tokio::fs::create_dir_all(parent).await.ok();
			}
			tokio::fs::write(path, json).await.ok();
		});
	}

	fn path() -> PathBuf { Xdg::state_dir().join("input-history.json") }
}
