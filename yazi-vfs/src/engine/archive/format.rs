use std::path::Path;

use super::Codec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
	Zip,
	SevenZ,
	Tar(Codec),
}

impl Format {
	const ALL: [(&str, &[&str], Self); 8] = [
		("zip", &["zip", "jar", "cbz"], Self::Zip),
		("7z", &["7z", "cb7", "7z.001"], Self::SevenZ),
		("tar", &["tar"], Self::Tar(Codec::None)),
		("tar-gz", &["tar.gz", "tgz"], Self::Tar(Codec::Gzip)),
		("tar-bz2", &["tar.bz2", "tbz", "tbz2"], Self::Tar(Codec::Bzip2)),
		("tar-xz", &["tar.xz", "txz"], Self::Tar(Codec::Xz)),
		("tar-zst", &["tar.zst", "tzst"], Self::Tar(Codec::Zstd)),
		("tar-lz4", &["tar.lz4"], Self::Tar(Codec::Lz4)),
	];

	pub fn from_path(path: &Path) -> Option<Self> {
		let name = path.file_name()?.to_string_lossy().to_lowercase();
		Self::ALL.iter().find_map(|&(_, exts, format)| {
			exts
				.iter()
				.any(|ext| {
					name.len() > ext.len() + 1
						&& name.ends_with(ext)
						&& name[..name.len() - ext.len()].ends_with('.')
				})
				.then_some(format)
		})
	}

	pub fn from_domain(domain: &[u8]) -> Option<Self> {
		Self::ALL.iter().find(|&&(d, ..)| d.as_bytes() == domain).map(|&(.., format)| format)
	}

	pub fn all() -> impl Iterator<Item = Self> { Self::ALL.iter().map(|&(.., f)| f) }

	/// The canonical file extension, without the leading dot.
	pub fn ext(self) -> &'static str {
		Self::ALL.iter().find(|&&(.., f)| f == self).map(|&(_, exts, _)| exts[0]).unwrap_or_default()
	}

	pub fn domain(self) -> &'static str {
		Self::ALL.iter().find(|&&(.., f)| f == self).map(|&(d, ..)| d).unwrap_or_default()
	}
}
