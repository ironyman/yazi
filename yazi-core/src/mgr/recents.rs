use std::{path::PathBuf, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

use serde::{Deserialize, Serialize};
use yazi_fs::Xdg;
use yazi_shared::url::{UrlBuf, UrlLike};

use crate::tab::Tab;

/// How long a stay must last before the place counts as recent on its own.
const LINGER: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recent {
	pub url:   UrlBuf,
	pub count: u32,
	/// Seconds since the Unix epoch.
	pub at:    u64,
}

/// Places the user has lingered or worked in, most recent first.
#[derive(Default)]
pub struct Recents {
	inner: Vec<Recent>,
}

impl Recents {
	pub fn load() -> Self {
		let inner = std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok());
		Self { inner: inner.unwrap_or_default() }
	}

	pub fn iter(&self) -> impl Iterator<Item = &Recent> { self.inner.iter() }

	/// Counts the tab's cwd as recent since the user worked in it, at most once per stay.
	pub fn mark(&mut self, tab: &mut Tab) {
		if tab.arrived.take().is_some() && self.visit(tab.cwd()) {
			tokio::spawn(self.save());
		}
	}

	/// Ends the tab's stay in its cwd, counting it if the user lingered, and begins the next one.
	pub fn depart(&mut self, tab: &mut Tab) {
		if Self::lingered(tab.arrived.replace(Instant::now())) && self.visit(tab.cwd()) {
			tokio::spawn(self.save());
		}
	}

	/// Ends the stays of all tabs, and returns the final save to await before exiting.
	pub fn conclude(&mut self, tabs: &mut [Tab]) -> impl Future<Output = ()> + use<> {
		for tab in tabs {
			if Self::lingered(tab.arrived.take()) {
				self.visit(tab.cwd());
			}
		}
		self.save()
	}

	fn visit(&mut self, url: &UrlBuf) -> bool {
		if !url.is_absolute() || url.is_view() {
			return false;
		}

		let count = match self.inner.iter().position(|r| r.url == *url) {
			Some(i) => self.inner.remove(i).count + 1,
			None => 1,
		};

		let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
		self.inner.insert(0, Recent { url: url.clone(), count, at });
		self.inner.truncate(100);
		true
	}

	fn lingered(arrived: Option<Instant>) -> bool { arrived.is_some_and(|t| t.elapsed() >= LINGER) }

	fn save(&self) -> impl Future<Output = ()> + use<> {
		let json = serde_json::to_vec(&self.inner);
		async move {
			let Ok(json) = json else { return };
			let path = Self::path();
			if let Some(parent) = path.parent() {
				tokio::fs::create_dir_all(parent).await.ok();
			}
			tokio::fs::write(path, json).await.ok();
		}
	}

	fn path() -> PathBuf { Xdg::state_dir().join("recents.json") }
}
