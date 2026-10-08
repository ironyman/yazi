use std::{fs::File, io::{self, BufReader}, path::Path, time::{Duration, SystemTime, UNIX_EPOCH}};

use hashbrown::HashMap;

use super::{Codec, Format, Meter, Node, NodeKind, Session, open_7z};

pub(super) struct Index {
	pub(super) len:       u64,
	pub(super) mtime:     Option<SystemTime>,
	pub(super) encrypted: bool,
	pub(super) nodes:     HashMap<String, Node>,
	pub(super) children:  HashMap<String, Vec<String>>,
}

impl Index {
	fn new(path: &Path) -> io::Result<Self> {
		let meta = std::fs::metadata(path)?;
		let mut me = Self {
			len:       meta.len(),
			mtime:     meta.modified().ok(),
			encrypted: false,
			nodes:     Default::default(),
			children:  Default::default(),
		};
		me.nodes.insert(String::new(), Node::dir(me.mtime));
		Ok(me)
	}

	/// Builds the index of the archive at `path`. Blocking.
	pub(super) fn build(
		session: &Session,
		progress: &dyn Fn(u64, u64),
		password: Option<&str>,
	) -> io::Result<Self> {
		let mut me = Self::new(&session.path)?;
		progress(0, me.len);

		match session.format {
			Format::Zip => me.scan_zip(&session.path)?,
			Format::SevenZ => me.scan_7z(&session.path, password)?,
			Format::Tar(codec) => me.scan_tar(&session.path, codec, progress)?,
		}

		progress(me.len, 0);
		Ok(me)
	}

	fn scan_zip(&mut self, path: &Path) -> io::Result<()> {
		let mut zip = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
		for i in 0..zip.len() {
			let file = zip.by_index_raw(i)?;
			self.encrypted |= file.encrypted();

			let kind = if file.is_dir() {
				NodeKind::Dir
			} else if file.is_symlink() {
				NodeKind::Link
			} else {
				NodeKind::File
			};

			self.insert(file.name(), Node {
				kind,
				len: file.size(),
				mtime: file.last_modified().and_then(zip_time),
				mode: file.unix_mode().map(|m| m & 0o7777),
				ordinal: Some(i),
				link: None,
			});
		}
		Ok(())
	}

	fn scan_7z(&mut self, path: &Path, password: Option<&str>) -> io::Result<()> {
		let archive = open_7z(path, password)?;

		for (i, entry) in archive.entries().iter().enumerate() {
			self.encrypted |= entry.is_encrypted;

			let kind = if entry.is_directory {
				NodeKind::Dir
			} else if entry.is_symlink {
				NodeKind::Link
			} else {
				NodeKind::File
			};

			self.insert(entry.path.as_str(), Node {
				kind,
				len: entry.size,
				mtime: entry.modified(),
				mode: entry.unix_mode().map(|m| m & 0o7777),
				ordinal: Some(i),
				link: None,
			});
		}
		Ok(())
	}

	fn scan_tar(&mut self, path: &Path, codec: Codec, progress: &dyn Fn(u64, u64)) -> io::Result<()> {
		let reader = Meter::new(BufReader::new(File::open(path)?), |n| Ok(progress(n, 0)));
		let mut tar = tar::Archive::new(codec.reader(reader)?);

		for (i, entry) in tar.entries()?.enumerate() {
			let entry = entry?;
			let header = entry.header();

			let kind = match header.entry_type() {
				tar::EntryType::Directory => NodeKind::Dir,
				tar::EntryType::Symlink | tar::EntryType::Link => NodeKind::Link,
				_ => NodeKind::File,
			};

			let link = entry.link_name_bytes().map(|b| String::from_utf8_lossy(&b).into_owned());
			self.insert(&String::from_utf8_lossy(&entry.path_bytes()), Node {
				kind,
				len: header.size()?,
				mtime: header.mtime().ok().map(|s| UNIX_EPOCH + Duration::from_secs(s)),
				mode: header.mode().ok().map(|m| m & 0o7777),
				ordinal: Some(i),
				link,
			});
		}
		Ok(())
	}

	fn insert(&mut self, name: &str, node: Node) {
		let Some(key) = normalize(name).filter(|k| !k.is_empty()) else { return };
		self.link(&key);
		self.nodes.insert(key, node);
	}

	/// Registers `key` as a child of its parent, creating implied parents.
	fn link(&mut self, key: &str) {
		if self.nodes.contains_key(key) {
			return;
		}

		let (parent, name) = split(key);
		self.children.entry(parent.to_owned()).or_default().push(name.to_owned());

		if !self.nodes.contains_key(parent) {
			self.link(parent);
			self.nodes.insert(parent.to_owned(), Node::dir(None));
		}
	}
}

/// Splits a key into its parent key and name.
pub(super) fn split(key: &str) -> (&str, &str) { key.rsplit_once('/').unwrap_or(("", key)) }

/// Normalizes an entry name into a `/`-separated key without leading, trailing
/// or empty components, rejecting names that escape the archive root.
pub(super) fn normalize(name: &str) -> Option<String> {
	let mut key = String::with_capacity(name.len());
	for part in name.split(['/', '\\']) {
		match part {
			"" | "." => continue,
			".." => return None,
			_ if cfg!(windows) && part.contains(':') => return None,
			_ => {}
		}
		if !key.is_empty() {
			key.push('/');
		}
		key.push_str(part);
	}
	Some(key)
}

fn zip_time(dt: zip::DateTime) -> Option<SystemTime> {
	let date = chrono::NaiveDate::from_ymd_opt(dt.year() as _, dt.month() as _, dt.day() as _)?;
	let time = date.and_hms_opt(dt.hour() as _, dt.minute() as _, dt.second() as _)?;
	let secs = u64::try_from(time.and_local_timezone(chrono::Local).earliest()?.timestamp()).ok()?;
	Some(UNIX_EPOCH + Duration::from_secs(secs))
}
