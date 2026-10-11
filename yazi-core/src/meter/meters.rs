use std::{sync::OnceLock, thread::{self, Thread}, time::Duration};

use yazi_config::{YAZI, mgr::{Meter, MeterLabels, MeterStyle}};

use super::{MeterPart, MeterStats, Sampler};
use crate::AppProxy;

const INTERVAL: Duration = Duration::from_secs(1);

/// Disk throughput, where how busy the disk is isn't known, is shown relative to a
/// recent peak, which decays so that a single burst doesn't flatten the meter.
const DISK_PEAK_DECAY: f64 = 0.95;

const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

static SAMPLER: OnceLock<Thread> = OnceLock::new();

/// The system resource meters shown in the header, see `mgr.system_meters`.
#[derive(Default)]
pub struct Meters {
	stats:     MeterStats,
	/// How loaded each meter was in the sample before, which the braille style shows too.
	previous:  [f64; Meter::ALL.len()],
	disk_peak: f64,
}

impl Meters {
	pub(crate) fn serve() -> Self {
		if let Ok(handle) = thread::Builder::new().name("meters".to_owned()).spawn(Self::sample_loop) {
			SAMPLER.set(handle.thread().clone()).ok();
		}
		Self::default()
	}

	/// Samples right away, so that a change to the meters shows without waiting for the next one.
	pub fn wake() {
		if let Some(thread) = SAMPLER.get() {
			thread.unpark();
		}
	}

	fn sample_loop() {
		let mut sampler = None;
		loop {
			let meters = YAZI.mgr.system_meters.load();
			if meters.is_empty() {
				sampler = None;
				thread::park();
			} else {
				AppProxy::update_meters(sampler.get_or_insert_with(Sampler::new).sample(&meters));
				thread::park_timeout(INTERVAL);
			}
		}
	}

	pub fn update(&mut self, stats: MeterStats) {
		self.previous = Meter::ALL.map(|m| self.stats.percent(m, self.disk_peak));
		self.disk_peak = (self.disk_peak * DISK_PEAK_DECAY).max(stats.disk() as f64);
		self.stats = stats;
	}

	/// The meters in their configured style and labels, every value taking the same
	/// width whatever it is, so that the header doesn't shift as they change.
	pub fn parts(&self) -> Vec<MeterPart> {
		let meters = YAZI.mgr.system_meters.load();
		let style = YAZI.mgr.system_meters_style.get();
		let labels = YAZI.mgr.system_meters_labels.get();

		// Single glyphs need no space between them when there are no labels to tell them apart
		let separate = labels != MeterLabels::None || style == MeterStyle::Text;

		let mut parts = vec![];
		for (i, &meter) in meters.iter().enumerate() {
			if i > 0 && separate {
				parts.push(MeterPart::label(" "));
			}

			let label = labels.label(meter);
			if !label.is_empty() {
				parts.push(MeterPart::label(label));
				parts.push(MeterPart::label(" "));
			}

			let percent = self.stats.percent(meter, self.disk_peak);
			let text = match style {
				MeterStyle::Bars => Self::bar(percent).to_string(),
				MeterStyle::Braille => Self::braille(self.previous[meter as usize], percent).to_string(),
				MeterStyle::Text => match meter {
					Meter::Net => Self::rate(self.stats.net()),
					Meter::Disk if self.stats.disk_busy.is_none() => Self::rate(self.stats.disk()),
					_ => format!("{percent:>3.0}%"),
				},
			};
			parts.push(MeterPart::value(text, percent));
		}
		parts
	}

	/// How many of `levels` steps `percent` fills, rounding up so that any activity shows.
	fn level(percent: f64, levels: usize) -> usize {
		((percent.clamp(0.0, 100.0) / 100.0 * levels as f64).ceil() as usize).min(levels)
	}

	/// A one column bar with 8 levels; anything above zero shows at least the lowest one.
	fn bar(percent: f64) -> char { BARS[Self::level(percent, BARS.len()).saturating_sub(1)] }

	/// Two bars of 4 levels side by side in one braille cell, filled from the bottom.
	fn braille(left: f64, right: f64) -> char {
		// Dots from the bottom row up: 7, 3, 2, 1 on the left and 8, 6, 5, 4 on the right
		const LEFT: [u32; 4] = [0x40, 0x04, 0x02, 0x01];
		const RIGHT: [u32; 4] = [0x80, 0x20, 0x10, 0x08];

		let cell = LEFT.iter().take(Self::level(left, 4)).sum::<u32>()
			+ RIGHT.iter().take(Self::level(right, 4)).sum::<u32>();
		char::from_u32(0x2800 + cell).unwrap_or(' ')
	}

	/// A byte rate with binary prefixes, as precisely as fits in 4 columns: a decimal
	/// below 10, whole numbers above, e.g. `  0B`, `512B`, `1.5K`, ` 30K`, `9.8M`.
	fn rate(bytes: u64) -> String {
		const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];

		let (mut value, mut unit) = (bytes as f64, 0);
		// At most 3 columns of number, so anything that would round to 1000 or more moves up a unit
		while value.round() >= 1000.0 && unit < UNITS.len() - 1 {
			value /= 1024.0;
			unit += 1;
		}

		let number = if unit > 0 && (value * 10.0).round() < 100.0 {
			format!("{value:.1}")
		} else {
			format!("{value:.0}")
		};
		format!("{:>4}", format!("{number}{}", UNITS[unit]))
	}
}
