use std::io::{self, Read, Write};

/// Keeps the first I/O error of the inner reader or writer and reports success
/// instead, for APIs that panic on I/O failure. A failed reader then reads as
/// EOF and a failed writer discards; [`Sticky::into_inner`] returns the error.
pub struct Sticky<T> {
	inner: T,
	error: Option<io::Error>,
}

impl<T> Sticky<T> {
	pub fn new(inner: T) -> Self { Self { inner, error: None } }

	pub fn into_inner(self) -> io::Result<T> {
		match self.error {
			Some(e) => Err(e),
			None => Ok(self.inner),
		}
	}
}

impl<R: Read> Read for Sticky<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		if self.error.is_some() {
			return Ok(0);
		}
		self.inner.read(buf).or_else(|e| {
			self.error = Some(e);
			Ok(0)
		})
	}
}

impl<W: Write> Write for Sticky<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		if self.error.is_none()
			&& let Err(e) = self.inner.write_all(buf)
		{
			self.error = Some(e);
		}
		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		if self.error.is_none()
			&& let Err(e) = self.inner.flush()
		{
			self.error = Some(e);
		}
		Ok(())
	}
}
