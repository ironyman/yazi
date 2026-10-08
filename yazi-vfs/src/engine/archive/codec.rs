use std::{fs::File, io::{self, BufReader, BufWriter, Read, Write}};

use yazi_shim::io::Sticky;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Codec {
	None,
	Gzip,
	Bzip2,
	Xz,
	Zstd,
	Lz4,
}

impl Codec {
	pub(super) fn reader<'a, R>(self, r: R) -> io::Result<Box<dyn Read + 'a>>
	where
		R: Read + 'a,
	{
		Ok(match self {
			Self::None => Box::new(r),
			Self::Gzip => Box::new(flate2::read::MultiGzDecoder::new(r)),
			Self::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(r)),
			Self::Xz => Box::new(lzma_rust2::XzReader::new(r, true)),
			Self::Zstd => Box::new(
				ruzstd::decoding::StreamingDecoder::new(r)
					.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?,
			),
			Self::Lz4 => Box::new(lz4_flex::frame::FrameDecoder::new(r)),
		})
	}

	/// Compresses `src` into `dst`. Blocking.
	pub(super) fn compress<R: Read>(self, src: R, dst: File) -> io::Result<()> {
		let (mut src, dst) = (BufReader::new(src), BufWriter::new(dst));
		let mut dst = match self {
			Self::None => {
				let mut dst = dst;
				io::copy(&mut src, &mut dst)?;
				dst
			}
			Self::Gzip => {
				let mut enc = flate2::write::GzEncoder::new(dst, flate2::Compression::default());
				io::copy(&mut src, &mut enc)?;
				enc.finish()?
			}
			Self::Bzip2 => {
				let mut enc = bzip2::write::BzEncoder::new(dst, bzip2::Compression::default());
				io::copy(&mut src, &mut enc)?;
				enc.finish()?
			}
			Self::Xz => {
				let mut enc = lzma_rust2::XzWriter::new(dst, lzma_rust2::XzOptions::with_preset(6))?;
				io::copy(&mut src, &mut enc)?;
				enc.finish()?
			}
			Self::Zstd => {
				let (mut src, mut dst) = (Sticky::new(src), Sticky::new(dst));
				ruzstd::encoding::compress(&mut src, &mut dst, ruzstd::encoding::CompressionLevel::Fastest);
				src.into_inner()?;
				dst.into_inner()?
			}
			Self::Lz4 => {
				let mut enc = lz4_flex::frame::FrameEncoder::new(dst);
				io::copy(&mut src, &mut enc)?;
				enc.finish().map_err(io::Error::other)?
			}
		};

		dst.flush()?;
		dst.into_inner().map_err(|e| e.into_error())?.sync_all()
	}
}
