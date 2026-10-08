use std::io;

use yazi_fs::{engine::{DirReader, FileHolder}, file::File, stat::{Stat, StatType}};
use yazi_shared::{path::PathBufDyn, strand::StrandCow, url::{UrlBuf, UrlLike}};

pub struct ReadDir(pub(super) std::vec::IntoIter<DirEntry>);

impl DirReader for ReadDir {
	type Entry = DirEntry;

	async fn next(&mut self) -> io::Result<Option<Self::Entry>> { Ok(self.0.next()) }
}

// --- Entry
pub struct DirEntry {
	pub(super) url:  UrlBuf,
	pub(super) stat: Stat,
}

impl FileHolder for DirEntry {
	async fn file(&self) -> io::Result<File> {
		Ok(File { url: self.url.clone(), stat: self.stat, extra: Default::default() })
	}

	async fn file_type(&self) -> io::Result<StatType> { Ok(*self.stat.mode) }

	async fn metadata(&self) -> io::Result<Stat> { Ok(self.stat) }

	fn name(&self) -> StrandCow<'_> { self.url.name().unwrap_or_default().into() }

	fn path(&self) -> PathBufDyn { self.url.loc().into() }

	fn url(&self) -> UrlBuf { self.url.clone() }
}
