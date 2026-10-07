use std::{ffi::OsString, os::windows::ffi::OsStringExt, ptr::null_mut, thread, time::Duration};

use windows_sys::Win32::{Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW}, System::WindowsProgramming::{DRIVE_CDROM, DRIVE_REMOTE, DRIVE_REMOVABLE}};

use super::{Locked, Partition, Partitions};

impl Partitions {
	pub fn monitor<F>(me: &'static Locked, cb: F)
	where
		F: Fn() + Send + 'static,
	{
		// Drive letters come and go rarely, so polling the cheap bitmask beats a message-only window
		thread::spawn(move || {
			let mut last = 0;
			loop {
				let mask = unsafe { GetLogicalDrives() };
				if mask != last {
					last = mask;
					me.write().inner = (0..26).filter(|i| mask & 1 << i != 0).map(Self::drive).collect();
					cb();
				}
				thread::sleep(Duration::from_secs(2));
			}
		});
	}

	fn drive(index: u8) -> Partition {
		let root = format!("{}:\\", (b'A' + index) as char);
		let wide: Vec<u16> = root.encode_utf16().chain([0]).collect();

		let (mut label, mut fstype) = ([0; 261], [0; 261]);
		let named = unsafe {
			GetVolumeInformationW(
				wide.as_ptr(),
				label.as_mut_ptr(),
				label.len() as _,
				null_mut(),
				null_mut(),
				null_mut(),
				fstype.as_mut_ptr(),
				fstype.len() as _,
			)
		} != 0;

		let mut capacity = 0;
		unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), null_mut(), &mut capacity, null_mut()) };

		let kind = unsafe { GetDriveTypeW(wide.as_ptr()) };
		Partition {
			src: root.clone().into(),
			dist: Some(root.into()),
			label: named.then(|| Self::wide_str(&label)).filter(|s| !s.is_empty()),
			fstype: named.then(|| Self::wide_str(&fstype)),
			capacity,
			// Network drives live outside this machine
			external: Some(kind == DRIVE_REMOTE),
			removable: Some(matches!(kind, DRIVE_REMOVABLE | DRIVE_CDROM)),
		}
	}

	fn wide_str(buf: &[u16]) -> OsString {
		let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
		OsString::from_wide(&buf[..len])
	}
}
