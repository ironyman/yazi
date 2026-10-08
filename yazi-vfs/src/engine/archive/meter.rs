use std::io::{self, Read};

/// Reports the number of bytes read through it, failing the read if the report does.
pub(super) struct Meter<R, F> {
	inner: R,
	f:     F,
}

impl<R, F> Meter<R, F> {
	pub(super) fn new(inner: R, f: F) -> Self { Self { inner, f } }
}

impl<R: Read, F: FnMut(u64) -> io::Result<()>> Read for Meter<R, F> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let n = self.inner.read(buf)?;
		(self.f)(n as u64)?;
		Ok(n)
	}
}
