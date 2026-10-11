use std::path::PathBuf;

use yazi_fs::Xdg;

/// The peak is saved again when it grows by more than this.
const SAVE_GROWTH: f64 = 1.05;

/// The highest network throughput seen on this machine, remembered across
/// sessions since it takes a while to see the link at full speed.
pub(super) struct NetworkPeak {
	pub(super) bytes: u64,
	saved:            u64,
}

impl NetworkPeak {
	pub(super) fn load() -> Self {
		let saved =
			std::fs::read_to_string(Self::path()).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
		Self { bytes: saved, saved }
	}

	pub(super) fn observe(&mut self, bytes: u64) {
		if bytes <= self.bytes {
			return;
		}

		self.bytes = bytes;
		if bytes as f64 > self.saved as f64 * SAVE_GROWTH
			&& std::fs::write(Self::path(), bytes.to_string()).is_ok()
		{
			self.saved = bytes;
		}
	}

	fn path() -> PathBuf { Xdg::state_dir().join("meters-network-peak") }
}
