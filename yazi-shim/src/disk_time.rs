/// The "% Disk Time" performance counter of the system drive, as shown by e.g. the
/// Resource Monitor, which `sysinfo` doesn't provide; clamped to 100%, since it goes
/// beyond that with several outstanding requests.
#[cfg(windows)]
pub struct DiskTime {
	query:   windows_sys::Win32::System::Performance::PDH_HQUERY,
	counter: windows_sys::Win32::System::Performance::PDH_HCOUNTER,
}

// PDH handles can be used from any thread
#[cfg(windows)]
unsafe impl Send for DiskTime {}

#[cfg(windows)]
impl DiskTime {
	pub fn new() -> Option<Self> {
		use windows_sys::Win32::System::Performance::{PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhOpenQueryW};

		use crate::ToWide;

		let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
		let path = format!("\\LogicalDisk({drive})\\% Disk Time").to_wide();

		let (mut query, mut counter) = (std::ptr::null_mut(), std::ptr::null_mut());
		unsafe {
			if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
				return None;
			}
			if PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) != 0 {
				PdhCloseQuery(query);
				return None;
			}
			// The counter is a rate between two collections, this is the first
			PdhCollectQueryData(query);
		}
		Some(Self { query, counter })
	}

	pub fn sample(&mut self) -> Option<f32> {
		use windows_sys::Win32::System::Performance::{PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PdhCollectQueryData, PdhGetFormattedCounterValue};

		// Not in windows-sys: "do not cap the value at 100", see `PdhGetFormattedCounterValue`
		const PDH_FMT_NOCAP100: u32 = 0x0000_8000;

		unsafe {
			if PdhCollectQueryData(self.query) != 0 {
				return None;
			}

			let mut value: PDH_FMT_COUNTERVALUE = std::mem::zeroed();
			let format = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
			if PdhGetFormattedCounterValue(self.counter, format, std::ptr::null_mut(), &mut value) != 0 {
				return None;
			}
			Some(value.Anonymous.doubleValue.clamp(0.0, 100.0) as f32)
		}
	}
}

#[cfg(windows)]
impl Drop for DiskTime {
	fn drop(&mut self) {
		unsafe { windows_sys::Win32::System::Performance::PdhCloseQuery(self.query) };
	}
}

#[cfg(not(windows))]
pub struct DiskTime;

#[cfg(not(windows))]
impl DiskTime {
	pub fn new() -> Option<Self> { None }

	pub fn sample(&mut self) -> Option<f32> { None }
}
