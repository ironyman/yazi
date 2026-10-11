use std::time::{Duration, Instant};

use sysinfo::{DiskRefreshKind, Disks, Networks, System};
use yazi_config::mgr::Meter;
use yazi_shim::DiskTime;

use super::{MeterStats, NetworkPeak};

/// Rates measured over less than this are too noisy to be taken as the network's peak.
const MINIMUM_PEAK_INTERVAL: Duration = Duration::from_millis(500);

pub(super) struct Sampler {
	system:    System,
	networks:  Networks,
	disks:     Disks,
	disk_time: Option<DiskTime>,
	net_peak:  NetworkPeak,
	last:      Instant,
}

impl Sampler {
	pub(super) fn new() -> Self {
		let mut system = System::new();
		// CPU usage is computed between two refreshes
		system.refresh_cpu_usage();

		Self {
			system,
			networks: Networks::new_with_refreshed_list(),
			disks: Disks::new_with_refreshed_list_specifics(Self::disk_refresh_kind()),
			disk_time: DiskTime::new(),
			net_peak: NetworkPeak::load(),
			last: Instant::now(),
		}
	}

	/// Samples only the `meters` shown.
	pub(super) fn sample(&mut self, meters: &[Meter]) -> MeterStats {
		let interval = self.last.elapsed();
		let per_second = |bytes: u64| (bytes as f64 / interval.as_secs_f64().max(0.001)) as u64;
		self.last = Instant::now();

		let mut stats = MeterStats::default();
		for meter in meters {
			match meter {
				Meter::Cpu => {
					self.system.refresh_cpu_usage();
					stats.cpu = self.system.global_cpu_usage();
				}
				Meter::Ram => {
					self.system.refresh_memory();
					stats.memory_used = self.system.used_memory();
					stats.memory_total = self.system.total_memory();
				}
				Meter::Disk if self.disk_time.is_some() => {
					stats.disk_busy = self.disk_time.as_mut().and_then(DiskTime::sample);
				}
				Meter::Disk => {
					self.disks.refresh_specifics(true, Self::disk_refresh_kind());
					let (read, written) = self
						.disks
						.iter()
						.map(|disk| disk.usage())
						.fold((0, 0), |(r, w), u| (r + u.read_bytes, w + u.written_bytes));

					stats.disk_read = per_second(read);
					stats.disk_written = per_second(written);
				}
				Meter::Net => {
					self.networks.refresh(true);
					// The busiest interface rather than the sum of all of them, since traffic
					// is often counted on several, e.g. a virtual switch and the adapter behind it
					if let Some(data) = self
						.networks
						.iter()
						.filter(|&(name, _)| !Self::is_loopback(name))
						.map(|(_, data)| data)
						.max_by_key(|data| data.total_received() + data.total_transmitted())
					{
						stats.net_received = per_second(data.received());
						stats.net_transmitted = per_second(data.transmitted());
					}
					if interval >= MINIMUM_PEAK_INTERVAL {
						self.net_peak.observe(stats.net());
					}
					stats.net_peak = self.net_peak.bytes;
				}
			}
		}
		stats
	}

	fn disk_refresh_kind() -> DiskRefreshKind { DiskRefreshKind::nothing().with_io_usage() }

	fn is_loopback(name: &str) -> bool {
		name == "lo" || name.to_ascii_lowercase().contains("loopback")
	}
}
