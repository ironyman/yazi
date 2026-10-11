use yazi_config::mgr::Meter;
use yazi_macro::impl_data_any;

/// Below this, disk and network throughput isn't shown relative to a peak,
/// so that idle noise doesn't look like full load.
const MINIMUM_PEAK: f64 = 1024.0 * 1024.0;

/// A sample of the system's resource usage, with rates in bytes per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterStats {
	pub cpu:             f32,
	pub memory_used:     u64,
	pub memory_total:    u64,
	pub disk_read:       u64,
	pub disk_written:    u64,
	/// How busy the system drive is, where that's available; otherwise the disk
	/// meter shows the read and write rates.
	pub disk_busy:       Option<f32>,
	pub net_received:    u64,
	pub net_transmitted: u64,
	/// The highest network throughput seen on this machine, which the network meter is relative to.
	pub net_peak:        u64,
}

impl_data_any!(MeterStats);

impl MeterStats {
	/// How loaded `meter` is, in percent. Disk throughput, where how busy the disk is
	/// isn't known, is relative to `disk_peak`, a recent peak in bytes per second.
	pub fn percent(&self, meter: Meter, disk_peak: f64) -> f64 {
		let relative = |value: u64, peak: f64| value as f64 / peak.max(MINIMUM_PEAK) * 100.0;
		let percent = match meter {
			Meter::Cpu => self.cpu as f64,
			Meter::Ram if self.memory_total == 0 => 0.0,
			Meter::Ram => self.memory_used as f64 / self.memory_total as f64 * 100.0,
			Meter::Disk => match self.disk_busy {
				Some(busy) => busy as f64,
				None => relative(self.disk(), disk_peak),
			},
			Meter::Net => relative(self.net(), self.net_peak as f64),
		};
		percent.clamp(0.0, 100.0)
	}

	pub fn disk(&self) -> u64 { self.disk_read + self.disk_written }

	pub fn net(&self) -> u64 { self.net_received + self.net_transmitted }
}
