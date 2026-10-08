use std::{fs::File, io::{self, BufReader, BufWriter, Read, Seek, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::Ordering}, time::{Duration, SystemTime, UNIX_EPOCH}};

use hashbrown::HashMap;
use yazi_config::YAZI;

use super::{Codec, Format, Index, Meter, NodeKind, Progress, Session, Slot, Source, Stage, open_7z, open_7z_stream, sevenz_err, zip_err};

/// Checked between entries and chunks; returning `true` aborts the commit.
pub(super) type Cancel = Box<dyn Fn() -> bool + Send + Sync>;

/// The entries of the archive being written, derived from the stage.
struct Plan {
	/// Entries of the original archive to carry over, by ordinal.
	keeps:    HashMap<usize, Keep>,
	adds:     Vec<Add>,
	workload: u64,
}

struct Keep {
	key:   String,
	len:   u64,
	mode:  Option<u32>,
	mtime: Option<SystemTime>,
}

enum Add {
	Dir { key: String, mtime: Option<SystemTime> },
	Blob { key: String, path: PathBuf, mode: Option<u32> },
}

impl Plan {
	fn new(stage: &Stage, index: &Index) -> Self {
		let mut me = Self { keeps: HashMap::new(), adds: vec![], workload: 0 };

		for key in stage.subtree(index, "").into_iter().skip(1) {
			let (source, mode, mtime) = match stage.slots.get(&key) {
				None => (key.clone(), None, None),
				Some(Slot::File { source: Source::Base(k), mode, mtime }) => (k.clone(), *mode, *mtime),
				Some(Slot::File { source: Source::Blob(path), mode, .. }) => {
					me.adds.push(Add::Blob { key, path: path.clone(), mode: *mode });
					continue;
				}
				Some(Slot::Dir(mtime)) => {
					me.adds.push(Add::Dir { key, mtime: *mtime });
					continue;
				}
				Some(Slot::Gone) => continue,
			};

			let Some(node) = index.nodes.get(&source) else { continue };
			if let Some(ordinal) = node.ordinal {
				me.workload += node.len;
				me.keeps.insert(ordinal, Keep { key, len: node.len, mode, mtime });
			}
		}
		me
	}

	/// A plan adding local files and directories, each under its own name.
	/// Symlinks to directories are skipped, so link cycles can't recurse forever.
	fn local(inputs: &[PathBuf]) -> io::Result<Self> {
		let mut me = Self { keeps: HashMap::new(), adds: vec![], workload: 0 };
		let mut stack: Vec<_> = inputs
			.iter()
			.rev()
			.filter_map(|p| Some((p.clone(), p.file_name()?.to_string_lossy().into_owned())))
			.collect();

		while let Some((path, key)) = stack.pop() {
			let meta = std::fs::metadata(&path)?;
			if meta.is_dir() && std::fs::symlink_metadata(&path)?.is_symlink() {
				continue;
			} else if meta.is_dir() {
				let mut children: Vec<_> = std::fs::read_dir(&path)?.collect::<io::Result<_>>()?;
				children.sort_unstable_by_key(|d| std::cmp::Reverse(d.file_name()));
				stack.extend(
					children
						.into_iter()
						.map(|d| (d.path(), format!("{key}/{}", d.file_name().to_string_lossy()))),
				);
				me.adds.push(Add::Dir { key, mtime: meta.modified().ok() });
			} else {
				me.adds.push(Add::Blob { key, path, mode: unix_mode(&meta) });
			}
		}
		Ok(me)
	}

	fn sorted_keeps(&self) -> Vec<(usize, &Keep)> {
		let mut keeps: Vec<_> = self.keeps.iter().map(|(&i, k)| (i, k)).collect();
		keeps.sort_unstable_by_key(|&(i, _)| i);
		keeps
	}
}

/// Writes the plan into a new archive; progress is reported in bytes.
struct Writer<'a> {
	session:  &'a Session,
	plan:     &'a Plan,
	password: Option<String>,
	progress: &'a Progress,
	cancel:   &'a Cancel,
}

impl Session {
	pub(super) async fn commit(
		self: &Arc<Self>,
		progress: Progress,
		cancel: Cancel,
	) -> io::Result<()> {
		let index = self.index().await?;
		let meta = tokio::fs::metadata(&self.path).await?;
		if meta.len() != index.len || meta.modified().ok() != index.mtime {
			return Err(io::Error::other(
				"The archive was modified outside of Yazi after it was opened, discard the staged changes to reload it",
			));
		} else if self.committing.swap(true, Ordering::AcqRel) {
			return Err(io::Error::new(
				io::ErrorKind::ResourceBusy,
				"Archive is already being committed",
			));
		}

		let me = self.clone();
		let result = tokio::task::spawn_blocking(move || {
			let plan = Plan::new(&me.stage.lock(), &index);
			me.write(&plan, &index, &progress, &cancel)
		})
		.await
		.map_err(io::Error::other)
		.flatten();

		if result.is_ok() {
			self.stage.lock().slots.clear();
			tokio::fs::remove_dir_all(self.dir.join("b")).await.ok();
		}
		self.committing.store(false, Ordering::Release);

		result?;
		self.index().await.map(|_| ())
	}

	/// Writes the new archive beside the original and moves it over. Blocking.
	fn write(
		&self,
		plan: &Plan,
		index: &Index,
		progress: &Progress,
		cancel: &Cancel,
	) -> io::Result<()> {
		let name = self.path.file_name().unwrap_or_default().to_string_lossy();
		let tmp =
			self.path.with_file_name(format!(".{name}.{}-{}.%tmp", std::process::id(), self.next_seq()));

		let w = Writer { session: self, plan, password: self.password(), progress, cancel };
		let native = self.format == Format::SevenZ && YAZI.mgr.archive_7z_native.get();

		let mut result = Self::create(&tmp)
			.and_then(|out| if native { w.sevenz_native(out, index) } else { w.write(out) });

		// The archive editor rejects what it cannot express, so stream instead
		if native && result.is_err() && !cancel() {
			std::fs::remove_file(&tmp).ok();
			result = Self::create(&tmp).and_then(|out| w.write(out));
		}

		if let Err(e) = result.and_then(|()| std::fs::rename(&tmp, &self.path)) {
			std::fs::remove_file(&tmp).ok();
			return Err(e);
		}
		Ok(())
	}

	/// Creates the archive from local files and directories. Blocking.
	pub(super) fn create_from(
		&self,
		inputs: &[PathBuf],
		volume: Option<u64>,
		password: Option<String>,
		progress: &Progress,
		cancel: &Cancel,
	) -> io::Result<()> {
		let plan = Plan::local(inputs)?;
		let w = Writer { session: self, plan: &plan, password, progress, cancel };

		if let Some(size) = volume.filter(|_| self.format == Format::SevenZ) {
			let result = w.sevenz_volumes(size);
			if result.is_err() {
				let name = self.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
				(1..)
					.map(|i| self.path.with_file_name(format!("{name}.{i:03}")))
					.map_while(|p| std::fs::remove_file(p).ok())
					.count();
			}
			return result;
		}

		let name = self.path.file_name().unwrap_or_default().to_string_lossy();
		let tmp =
			self.path.with_file_name(format!(".{name}.{}-{}.%tmp", std::process::id(), self.next_seq()));

		let result = Self::create(&tmp).and_then(|out| w.write(out)).and_then(|()| {
			if self.path.exists() {
				Err(io::ErrorKind::AlreadyExists.into())
			} else {
				std::fs::rename(&tmp, &self.path)
			}
		});
		if result.is_err() {
			std::fs::remove_file(&tmp).ok();
		}
		result
	}

	fn create(tmp: &Path) -> io::Result<File> {
		File::options().read(true).write(true).create_new(true).open(tmp)
	}
}

impl Writer<'_> {
	fn write(&self, out: File) -> io::Result<()> {
		if let Ok(meta) = std::fs::metadata(&self.session.path) {
			out.set_permissions(meta.permissions()).ok();
		}

		match self.session.format {
			Format::Zip => self.zip(out),
			Format::SevenZ => self.sevenz(out),
			Format::Tar(codec) => self.tar(out, codec),
		}
	}

	fn zip(&self, out: File) -> io::Result<()> {
		let workload = self.plan.workload + self.blob_workload();
		(self.progress)(0, workload);

		let mut dst = zip::ZipWriter::new(BufWriter::new(out));
		if !self.plan.keeps.is_empty() {
			self.zip_keeps(&mut dst)?;
		}
		self.zip_adds(&mut dst)?;

		let out = dst.finish()?;
		Self::sync(out)
	}

	fn zip_keeps(&self, dst: &mut zip::ZipWriter<BufWriter<File>>) -> io::Result<()> {
		use zip::write::SimpleFileOptions;

		let mut src = zip::ZipArchive::new(BufReader::new(File::open(&self.session.path)?))?;
		for (ordinal, keep) in self.plan.sorted_keeps() {
			self.check()?;
			let raw = src.by_index_raw(ordinal)?;
			let name = if raw.is_dir() { format!("{}/", keep.key) } else { keep.key.clone() };

			if keep.mode.is_none() && keep.mtime.is_none() {
				dst.raw_copy_file_rename(raw, name)?;
				(self.progress)(keep.len, 0);
				continue;
			}

			let (compression, mode, mtime) = (raw.compression(), raw.unix_mode(), raw.last_modified());
			drop(raw);

			let mut options = SimpleFileOptions::default()
				.compression_method(compression)
				.large_file(keep.len >= u32::MAX as u64);
			if let Some(mode) = keep.mode.or(mode) {
				options = options.unix_permissions(mode);
			}
			if let Some(time) = keep.mtime.and_then(zip_time).or(mtime) {
				options = options.last_modified_time(time);
			}

			if name.ends_with('/') {
				dst.add_directory(name, options)?;
				continue;
			}

			let mut file = match &self.password {
				Some(p) => src.by_index_decrypt(ordinal, p.as_bytes()),
				None => src.by_index(ordinal),
			}
			.map_err(zip_err)?;

			match &self.password {
				Some(p) if file.encrypted() => {
					dst.start_file(name, options.with_aes_encryption(zip::AesMode::Aes256, p))?
				}
				_ => dst.start_file(name, options)?,
			}
			self.pump(&mut file, dst)?;
		}
		Ok(())
	}

	fn zip_adds(&self, dst: &mut zip::ZipWriter<BufWriter<File>>) -> io::Result<()> {
		use zip::{CompressionMethod, write::SimpleFileOptions};

		for add in &self.plan.adds {
			self.check()?;
			match add {
				Add::Dir { key, mtime } => {
					let mut options = SimpleFileOptions::default();
					if let Some(time) = mtime.and_then(zip_time) {
						options = options.last_modified_time(time);
					}
					dst.add_directory(format!("{key}/"), options)?;
				}
				Add::Blob { key, path, mode } => {
					let mut file = File::open(path)?;
					let meta = file.metadata()?;

					let mut options = SimpleFileOptions::default()
						.compression_method(CompressionMethod::Deflated)
						.large_file(meta.len() >= u32::MAX as u64);
					if let Some(mode) = mode {
						options = options.unix_permissions(*mode);
					}
					if let Some(time) = meta.modified().ok().and_then(zip_time) {
						options = options.last_modified_time(time);
					}

					match &self.password {
						Some(p) => dst.start_file(key, options.with_aes_encryption(zip::AesMode::Aes256, p))?,
						None => dst.start_file(key, options)?,
					}
					self.pump(&mut file, dst)?;
				}
			}
		}
		Ok(())
	}

	fn tar(&self, out: File, codec: Codec) -> io::Result<()> {
		let blobs = self.blob_workload();
		let passes = if codec == Codec::None { 1 } else { 2 };
		(self.progress)(0, (self.plan.workload + blobs) * passes);

		let plain = if codec == Codec::None { None } else { Some(self.session.tmp()) };
		let sink = match &plain {
			Some(path) => {
				std::fs::create_dir_all(&self.session.dir)?;
				File::create(path)?
			}
			None => out.try_clone()?,
		};

		let result = self.tar_entries(sink).and_then(|sink| {
			let Some(path) = &plain else { return Self::sync(sink) };
			drop(sink);

			let src = Meter::new(File::open(path)?, |n| self.tick(n));
			codec.compress(src, out)
		});

		if let Some(path) = plain {
			std::fs::remove_file(path).ok();
		}
		result
	}

	fn tar_entries(&self, sink: File) -> io::Result<BufWriter<File>> {
		let mut builder = tar::Builder::new(BufWriter::new(sink));
		builder.follow_symlinks(false);

		if !self.plan.keeps.is_empty() {
			self.tar_keeps(&mut builder)?;
		}

		for add in &self.plan.adds {
			self.check()?;
			let mut header = tar::Header::new_gnu();
			match add {
				Add::Dir { key, mtime } => {
					header.set_entry_type(tar::EntryType::Directory);
					header.set_mode(0o755);
					header.set_mtime(mtime.and_then(unix_secs).unwrap_or_default());
					header.set_size(0);
					builder.append_data(&mut header, key, io::empty())?;
				}
				Add::Blob { key, path, mode } => {
					let file = File::open(path)?;
					let meta = file.metadata()?;
					header.set_entry_type(tar::EntryType::Regular);
					header.set_mode(mode.unwrap_or(0o644));
					header.set_mtime(meta.modified().ok().and_then(unix_secs).unwrap_or_default());
					header.set_size(meta.len());
					builder.append_data(&mut header, key, Meter::new(file, |n| self.tick(n)))?;
				}
			}
		}

		builder.into_inner()
	}

	fn tar_keeps(&self, builder: &mut tar::Builder<BufWriter<File>>) -> io::Result<()> {
		let Format::Tar(codec) = self.session.format else { unreachable!() };
		let reader = codec.reader(BufReader::new(File::open(&self.session.path)?))?;
		let mut tar = tar::Archive::new(reader);

		for (ordinal, entry) in tar.entries()?.enumerate() {
			let entry = entry?;
			let Some(keep) = self.plan.keeps.get(&ordinal) else { continue };
			self.check()?;

			let mut header = entry.header().clone();
			if let Some(mode) = keep.mode {
				header.set_mode(mode);
			}
			if let Some(secs) = keep.mtime.and_then(unix_secs) {
				header.set_mtime(secs);
			}

			match header.entry_type() {
				tar::EntryType::Symlink | tar::EntryType::Link => {
					let target = entry.link_name()?.unwrap_or_default().into_owned();
					builder.append_link(&mut header, &keep.key, target)?;
				}
				tar::EntryType::Directory => builder.append_data(&mut header, &keep.key, io::empty())?,
				_ => builder.append_data(&mut header, &keep.key, Meter::new(entry, |n| self.tick(n)))?,
			}
		}
		Ok(())
	}

	fn sevenz(&self, out: File) -> io::Result<()> {
		let mut dst = zesven::Writer::create(BufWriter::new(out)).map_err(sevenz_err)?;
		if let Some(p) = &self.password {
			dst = dst.options(zesven::WriteOptions::new().password(p.as_str()));
		}

		self.sevenz_entries(&mut dst)?;
		let (_, out) = dst.finish_into_inner().map_err(sevenz_err)?;
		Self::sync(out)
	}

	/// Writes a multi-volume 7z archive, as `<path>.001`, `<path>.002`, and so on.
	fn sevenz_volumes(&self, size: u64) -> io::Result<()> {
		let config = zesven::VolumeConfig::new(&self.session.path, size);
		let mut dst = zesven::Writer::create_multivolume(config).map_err(sevenz_err)?;
		if let Some(p) = &self.password {
			dst = dst.options(zesven::WriteOptions::new().password(p.as_str()));
		}
		self.sevenz_entries(&mut dst)?;
		dst.finish().map_err(sevenz_err).map(|_| ())
	}

	fn sevenz_entries<W: Write + Seek + Send>(&self, dst: &mut zesven::Writer<W>) -> io::Result<()> {
		use zesven::{ArchivePath, write::EntryMeta};

		(self.progress)(0, self.plan.workload + self.blob_workload());
		if !self.plan.keeps.is_empty() {
			self.sevenz_keeps(dst)?;
		}

		for add in &self.plan.adds {
			self.check()?;
			match add {
				Add::Dir { key, mtime } => {
					let mut meta = EntryMeta::directory();
					if let Some(time) = mtime {
						meta = meta.modification_time(filetime(*time));
					}
					dst
						.add_directory(ArchivePath::new(key).map_err(sevenz_err)?, meta)
						.map_err(sevenz_err)?;
				}
				Add::Blob { key, path, mode } => {
					let file = File::open(path)?;
					let fs = file.metadata()?;

					let mut meta = EntryMeta::file(fs.len());
					if let Ok(time) = fs.modified() {
						meta = meta.modification_time(filetime(time));
					}
					if let Some(mode) = mode {
						meta = meta.attributes(0x8000 | mode << 16);
					}

					let mut data = Meter::new(file, |n| self.tick(n));
					dst
						.add_stream(ArchivePath::new(key).map_err(sevenz_err)?, &mut data, meta)
						.map_err(sevenz_err)?;
				}
			}
		}
		Ok(())
	}

	fn sevenz_keeps<W: Write + Seek + Send>(&self, dst: &mut zesven::Writer<W>) -> io::Result<()> {
		use zesven::{ArchivePath, write::EntryMeta};

		let password = self.password.clone().unwrap_or_default();
		let mut src = open_7z_stream(&self.session.path, &password)?;

		let mut it = src.entries().map_err(sevenz_err)?;
		let mut ordinal = 0;
		while let Some(entry) = it.next() {
			let entry = entry.map_err(sevenz_err)?;
			ordinal += 1;
			let Some(keep) = self.plan.keeps.get(&(ordinal - 1)) else { continue };
			self.check()?;

			let e = entry.entry();
			let meta = EntryMeta {
				is_directory:      e.is_directory,
				size:              e.size,
				modification_time: keep.mtime.map(filetime).or(e.modification_time),
				creation_time:     e.creation_time,
				access_time:       e.access_time,
				attributes:        match keep.mode {
					Some(mode) => Some(e.attributes.unwrap_or_default() & 0xffff | 0x8000 | mode << 16),
					None => e.attributes,
				},
				is_anti:           false,
			};

			let path = ArchivePath::new(&keep.key).map_err(sevenz_err)?;
			if meta.is_directory {
				dst.add_directory(path, meta).map_err(sevenz_err)?;
			} else {
				let mut data = Meter::new(EntryData(&mut it), |n| self.tick(n));
				dst.add_stream(path, &mut data, meta).map_err(sevenz_err)?;
			}
		}
		Ok(())
	}

	/// Commits with zesven's archive editor, which takes renames, removals,
	/// updates and additions of files only.
	fn sevenz_native(&self, mut out: File, index: &Index) -> io::Result<()> {
		use zesven::{ArchivePath, EditableArchive};

		let workload = self.plan.workload + self.blob_workload();
		(self.progress)(0, workload);

		let unsupported =
			|| io::Error::new(io::ErrorKind::Unsupported, "Not expressible as archive edits");
		if self.plan.keeps.values().any(|k| k.mode.is_some() || k.mtime.is_some()) {
			return Err(unsupported());
		}

		let archive = open_7z(&self.session.path, self.password.as_deref())?;
		let mut editor = archive.edit();
		if let Some(p) = &self.password {
			editor = editor.with_options(zesven::WriteOptions::new().password(p.as_str()));
		}

		let mut blobs: HashMap<&str, &PathBuf> = HashMap::new();
		for add in &self.plan.adds {
			match add {
				Add::Dir { .. } => return Err(unsupported()),
				Add::Blob { key, path, .. } => _ = blobs.insert(key, path),
			}
		}

		// The editor drops directory entries, losing directories left empty
		let keys: Vec<_> =
			self.plan.keeps.values().map(|k| k.key.as_str()).chain(blobs.keys().copied()).collect();
		let inside = |dir: &str| {
			keys
				.iter()
				.any(|k| k.len() > dir.len() && k.starts_with(dir) && k[dir.len()..].starts_with('/'))
		};
		let dirs = index.nodes.values().filter(|n| n.kind == NodeKind::Dir);
		if dirs.filter_map(|n| self.plan.keeps.get(&n.ordinal?)).any(|k| !inside(&k.key)) {
			return Err(unsupported());
		}

		for (key, node) in &index.nodes {
			let Some(ordinal) = node.ordinal.filter(|_| node.kind != NodeKind::Dir) else { continue };
			match self.plan.keeps.get(&ordinal) {
				Some(keep) if keep.key == *key => {}
				Some(keep) => editor.rename(key, &keep.key).map_err(sevenz_err)?,
				None if let Some(path) = blobs.remove(key.as_str()) => {
					editor.update(key, std::fs::read(path)?).map_err(sevenz_err)?
				}
				None => editor.delete(key).map_err(sevenz_err)?,
			}
		}

		for (key, path) in blobs {
			self.check()?;
			editor
				.add(ArchivePath::new(key).map_err(sevenz_err)?, std::fs::read(path)?)
				.map_err(sevenz_err)?;
		}

		self.check()?;
		_ = editor.apply(&mut out).map_err(sevenz_err)?;
		(self.progress)(workload, 0);
		out.sync_all()
	}

	fn pump(&self, src: &mut dyn Read, dst: &mut dyn Write) -> io::Result<()> {
		let mut buf = vec![0; 256 * 1024];
		loop {
			let n = src.read(&mut buf)?;
			if n == 0 {
				return Ok(());
			}
			dst.write_all(&buf[..n])?;
			self.tick(n as u64)?;
		}
	}

	fn tick(&self, n: u64) -> io::Result<()> {
		(self.progress)(n, 0);
		self.check()
	}

	fn check(&self) -> io::Result<()> {
		if (self.cancel)() {
			Err(io::Error::new(io::ErrorKind::Interrupted, "Canceled"))
		} else {
			Ok(())
		}
	}

	fn blob_workload(&self) -> u64 {
		let blob = |a: &Add| match a {
			Add::Blob { path, .. } => std::fs::metadata(path).map_or(0, |m| m.len()),
			Add::Dir { .. } => 0,
		};
		self.plan.adds.iter().map(blob).sum()
	}

	fn sync(out: BufWriter<File>) -> io::Result<()> {
		out.into_inner().map_err(|e| e.into_error())?.sync_all()
	}
}

/// Reads the data of the entry an [`zesven::EntryIterator`] is positioned at.
struct EntryData<'a, 'b, R: Read + Seek + Send>(&'a mut zesven::EntryIterator<'b, R>);

impl<R: Read + Seek + Send> Read for EntryData<'_, '_, R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.0.read_entry_data(buf) }
}

fn unix_secs(time: SystemTime) -> Option<u64> {
	time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Converts to a Windows FILETIME, as 7z stores timestamps.
fn filetime(time: SystemTime) -> u64 {
	const EPOCH_DIFF: Duration = Duration::from_secs(11_644_473_600);
	let d = time.duration_since(UNIX_EPOCH).unwrap_or_default() + EPOCH_DIFF;
	d.as_secs() * 10_000_000 + d.subsec_nanos() as u64 / 100
}

fn zip_time(time: SystemTime) -> Option<zip::DateTime> {
	use chrono::{Datelike, Timelike};

	let t = chrono::DateTime::<chrono::Local>::from(time).naive_local();
	zip::DateTime::from_date_and_time(
		t.year().try_into().ok()?,
		t.month() as _,
		t.day() as _,
		t.hour() as _,
		t.minute() as _,
		t.second() as _,
	)
	.ok()
}

fn unix_mode(meta: &std::fs::Metadata) -> Option<u32> {
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		Some(meta.permissions().mode() & 0o7777)
	}
	#[cfg(not(unix))]
	{
		_ = meta;
		None
	}
}
