use std::{io, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::SystemTime};

use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock};
use yazi_fs::{FsHash128, Xdg, engine::{Attrs, Demand}, stat::{Stat, StatMode}};
use yazi_shared::url::Url;
use yazi_shim::cell::RoCell;

use super::{Entry, Format, Index, Node, NodeKind, Slot, Source, Stage, is_split, join, split};

static SESSIONS: RoCell<Mutex<HashMap<PathBuf, Arc<Session>>>> = RoCell::new();

pub(crate) fn init() {
	SESSIONS.init(Default::default());

	// Staged and extracted files of previous runs are unreachable now
	tokio::spawn(tokio::fs::remove_dir_all(Session::root()));
}

pub(super) type Progress = Box<dyn Fn(u64, u64) + Send + Sync>;

/// An archive mounted for browsing, with the changes staged against it.
pub(super) struct Session {
	pub(super) path:       PathBuf,
	pub(super) format:     Format,
	pub(super) dir:        PathBuf,
	pub(super) password:   Mutex<Option<String>>,
	pub(super) stage:      Mutex<Stage>,
	pub(super) committing: AtomicBool,
	pub(super) handle:     std::sync::Mutex<Handle>,
	seq:                   AtomicU64,
	index:                 RwLock<Option<Arc<Index>>>,
	generation:            AtomicU64,
	loading:               tokio::sync::Mutex<()>,
	extracting:            tokio::sync::Mutex<()>,
}

/// A reader kept open between extractions, for formats with random access.
#[derive(Default)]
pub(super) enum Handle {
	#[default]
	None,
	Zip(zip::ZipArchive<std::io::BufReader<std::fs::File>>),
	SevenZ(Box<zesven::Archive<zesven::read::ArchiveSource>>),
}

/// What a key resolves to, detached from the session's locks.
enum Resolved {
	Node(Node),
	Blob(PathBuf, Option<u32>),
}

impl Session {
	fn root() -> PathBuf { Xdg::temp_dir().join("archive") }

	pub(super) fn get(path: &Path, format: Format) -> Arc<Self> {
		SESSIONS
			.lock()
			.entry_ref(path)
			.or_insert_with(|| {
				Arc::new(Self {
					path: path.to_owned(),
					format,
					dir: Self::root().join(Url::regular(path).hash_u128_str(&mut [0; 26])),
					password: Default::default(),
					stage: Default::default(),
					committing: Default::default(),
					handle: Default::default(),
					seq: Default::default(),
					index: Default::default(),
					generation: Default::default(),
					loading: Default::default(),
					extracting: Default::default(),
				})
			})
			.clone()
	}

	pub(super) fn find(path: &Path) -> Option<Arc<Self>> { SESSIONS.lock().get(path).cloned() }

	pub(super) fn all() -> Vec<Arc<Self>> { SESSIONS.lock().values().cloned().collect() }

	#[inline]
	pub(super) fn cached(&self) -> Option<Arc<Index>> { self.index.read().clone() }

	#[inline]
	pub(super) fn generation(&self) -> u64 { self.generation.load(Ordering::Relaxed) }

	#[inline]
	pub(super) fn is_committing(&self) -> bool { self.committing.load(Ordering::Acquire) }

	pub(super) fn password(&self) -> Option<String> { self.password.lock().clone() }

	pub(super) async fn index(self: &Arc<Self>) -> io::Result<Arc<Index>> {
		self.load(Box::new(|_, _| ())).await
	}

	/// Returns the index, building it if absent, or rebuilding it if the archive
	/// changed on disk and there is nothing staged against the old one.
	pub(super) async fn load(self: &Arc<Self>, progress: Progress) -> io::Result<Arc<Index>> {
		if let Some(index) = self.reusable().await {
			return Ok(index);
		}

		let _guard = self.loading.lock().await;
		if let Some(index) = self.reusable().await {
			return Ok(index);
		}

		let me = self.clone();
		let index = tokio::task::spawn_blocking(move || {
			let password = me.password();
			Index::build(&me, &progress, password.as_deref())
		})
		.await??;

		Ok(self.replace(index))
	}

	pub(super) fn replace(&self, index: Index) -> Arc<Index> {
		let index = Arc::new(index);
		*self.handle.lock().unwrap() = Handle::None;
		*self.index.write() = Some(index.clone());
		self.generation.fetch_add(1, Ordering::Relaxed);
		index
	}

	async fn reusable(&self) -> Option<Arc<Index>> {
		let index = self.cached()?;
		if self.is_committing() || !self.stage.lock().is_empty() {
			return Some(index);
		}

		let meta = tokio::fs::metadata(&self.path).await.ok()?;
		(meta.len() == index.len && meta.modified().ok() == index.mtime).then_some(index)
	}

	pub(super) fn ensure_mutable(&self) -> io::Result<()> {
		if is_split(&self.path) {
			Err(io::Error::new(
				io::ErrorKind::ReadOnlyFilesystem,
				"Split archives are read-only, extract the files to change them",
			))
		} else if self.is_committing() {
			Err(io::Error::new(io::ErrorKind::ResourceBusy, "Archive is being committed"))
		} else {
			Ok(())
		}
	}

	pub(super) fn tmp(&self) -> PathBuf { self.dir.join(format!("{}.%tmp", self.next_seq())) }

	#[inline]
	pub(super) fn next_seq(&self) -> u64 { self.seq.fetch_add(1, Ordering::Relaxed) }
}

// --- Operations
impl Session {
	pub(super) async fn stat(self: &Arc<Self>, key: &str) -> io::Result<Stat> {
		let index = self.index().await?;
		let resolved = self.resolve(&index, key)?;
		Self::finish(split(key).1, resolved).await
	}

	pub(super) async fn read_dir(self: &Arc<Self>, key: &str) -> io::Result<Vec<(String, Stat)>> {
		let index = self.index().await?;
		let resolved: Vec<_> = {
			let stage = self.stage.lock();
			if !stage.get(&index, key).ok_or(io::ErrorKind::NotFound)?.is_dir() {
				Err(io::ErrorKind::NotADirectory)?;
			}

			stage
				.children(&index, key)
				.into_iter()
				.filter_map(|name| Some((Self::resolve_in(&stage, &index, &join(key, &name)).ok()?, name)))
				.collect()
		};

		let mut entries = Vec::with_capacity(resolved.len());
		for (resolved, name) in resolved {
			if let Ok(stat) = Self::finish(&name, resolved).await {
				entries.push((name, stat));
			}
		}
		Ok(entries)
	}

	pub(super) async fn read_link(self: &Arc<Self>, key: &str) -> io::Result<String> {
		let index = self.index().await?;
		match self.resolve(&index, key)? {
			Resolved::Node(Node { link: Some(link), .. }) => Ok(link),
			_ => Err(io::Error::new(io::ErrorKind::InvalidInput, "Not a symbolic link")),
		}
	}

	pub(super) async fn create_dir(self: &Arc<Self>, key: &str) -> io::Result<()> {
		let index = self.index().await?;
		self.ensure_mutable()?;

		let mut stage = self.stage.lock();
		if stage.get(&index, key).is_some() {
			Err(io::ErrorKind::AlreadyExists)?;
		} else if !stage.get(&index, split(key).0).is_some_and(|e| e.is_dir()) {
			Err(io::ErrorKind::NotFound)?;
		}

		stage.put(&index, key.to_owned(), Slot::Dir(Some(SystemTime::now())));
		Ok(())
	}

	pub(super) async fn remove(self: &Arc<Self>, key: &str, dir: bool, all: bool) -> io::Result<()> {
		let index = self.index().await?;
		self.ensure_mutable()?;

		let mut stage = self.stage.lock();
		let Some(entry) = stage.get(&index, key) else {
			return if all { Ok(()) } else { Err(io::ErrorKind::NotFound.into()) };
		};

		if key.is_empty() {
			Err(io::ErrorKind::PermissionDenied)?;
		} else if entry.is_dir() != dir {
			Err(if dir { io::ErrorKind::NotADirectory } else { io::ErrorKind::IsADirectory })?;
		} else if !all && dir && !stage.children(&index, key).is_empty() {
			Err(io::ErrorKind::DirectoryNotEmpty)?;
		}

		for key in stage.subtree(&index, key) {
			stage.remove(&index, &key);
		}
		Ok(())
	}

	pub(super) async fn rename(self: &Arc<Self>, from: &str, to: &str) -> io::Result<()> {
		let index = self.index().await?;
		self.ensure_mutable()?;

		let mut stage = self.stage.lock();
		if !stage.get(&index, split(to).0).is_some_and(|e| e.is_dir()) {
			Err(io::ErrorKind::NotFound)?;
		}
		stage.rename(&index, from, to)
	}

	pub(super) async fn set_attrs(self: &Arc<Self>, key: &str, attrs: Attrs) -> io::Result<()> {
		let index = self.index().await?;
		self.ensure_mutable()?;

		let mode = attrs.mode.map(|m| (m - StatMode::T_MASK).bits() as u32);
		let blob = {
			let mut stage = self.stage.lock();
			if stage.get(&index, key).is_none() {
				Err(io::ErrorKind::NotFound)?;
			}

			let (slot, blob) = match stage.materialize(&index, key) {
				Slot::Dir(mtime) => (Slot::Dir(attrs.mtime.or(mtime)), None),
				Slot::File { source: Source::Blob(p), mode: m, mtime } => {
					(Slot::File { source: Source::Blob(p.clone()), mode: mode.or(m), mtime }, Some(p))
				}
				Slot::File { source, mode: m, mtime } => {
					(Slot::File { source, mode: mode.or(m), mtime: attrs.mtime.or(mtime) }, None)
				}
				Slot::Gone => Err(io::ErrorKind::NotFound)?,
			};

			stage.put(&index, key.to_owned(), slot);
			blob
		};

		if let Some(blob) = blob
			&& let Ok(times) = std::fs::FileTimes::try_from(attrs)
		{
			let file = tokio::fs::OpenOptions::new().write(true).open(blob).await?.into_std().await;
			tokio::task::spawn_blocking(move || file.set_times(times)).await??;
		}
		Ok(())
	}

	/// Returns the local file holding the content of `key`, for reading.
	pub(super) async fn checkout(self: &Arc<Self>, key: &str) -> io::Result<PathBuf> {
		let index = self.index().await?;
		match self.resolve(&index, key)? {
			Resolved::Blob(path, _) => Ok(path),
			Resolved::Node(Node { kind: NodeKind::Dir, .. }) => Err(io::ErrorKind::IsADirectory.into()),
			Resolved::Node(Node { ordinal: Some(ordinal), .. }) => self.extracted(ordinal).await,
			Resolved::Node(_) => Err(io::ErrorKind::NotFound.into()),
		}
	}

	/// Returns the local file the content of `key` is written to, staging it.
	pub(super) async fn checkin(self: &Arc<Self>, key: &str, demand: &Demand) -> io::Result<PathBuf> {
		let index = self.index().await?;
		self.ensure_mutable()?;

		let (blob, base) = {
			let mut stage = self.stage.lock();
			let exists = match stage.get(&index, key) {
				Some(e) if e.is_dir() => Err(io::ErrorKind::IsADirectory)?,
				Some(_) if demand.create_new => Err(io::ErrorKind::AlreadyExists)?,
				Some(Entry::Slot(Slot::File { source: Source::Blob(p), .. })) => return Ok(p.clone()),
				Some(_) => true,
				None if !demand.create && !demand.create_new => Err(io::ErrorKind::NotFound)?,
				None if !stage.get(&index, split(key).0).is_some_and(|e| e.is_dir()) => {
					Err(io::ErrorKind::NotFound)?
				}
				None => false,
			};

			let (mode, base) = match stage.materialize(&index, key) {
				Slot::File { source: Source::Base(k), mode, .. } if exists => (mode, Some(k)),
				_ => (None, None),
			};

			let blob = stage.blob(&self.dir.join("b"));
			stage.put(&index, key.to_owned(), Slot::File {
				source: Source::Blob(blob.clone()),
				mode,
				mtime: None,
			});
			(blob, base.filter(|_| !demand.truncate))
		};

		tokio::fs::create_dir_all(blob.parent().unwrap()).await?;
		match base.and_then(|k| index.nodes.get(&k)?.ordinal) {
			Some(ordinal) => _ = tokio::fs::copy(self.extracted(ordinal).await?, &blob).await?,
			None => _ = tokio::fs::File::create(&blob).await?,
		}
		Ok(blob)
	}

	async fn extracted(self: &Arc<Self>, ordinal: usize) -> io::Result<PathBuf> {
		let _guard = self.extracting.lock().await;
		self.extract(ordinal).await
	}

	fn resolve(&self, index: &Index, key: &str) -> io::Result<Resolved> {
		Self::resolve_in(&self.stage.lock(), index, key)
	}

	fn resolve_in(stage: &Stage, index: &Index, key: &str) -> io::Result<Resolved> {
		Ok(match stage.get(index, key).ok_or(io::ErrorKind::NotFound)? {
			Entry::Base(node) => Resolved::Node(node.clone()),
			Entry::Slot(Slot::Dir(mtime)) => Resolved::Node(Node::dir(*mtime)),
			Entry::Slot(Slot::File { source: Source::Base(k), mode, mtime }) => {
				let mut node = index.nodes.get(k).cloned().ok_or(io::ErrorKind::NotFound)?;
				node.mode = mode.or(node.mode);
				node.mtime = mtime.or(node.mtime);
				Resolved::Node(node)
			}
			Entry::Slot(Slot::File { source: Source::Blob(p), mode, .. }) => {
				Resolved::Blob(p.clone(), *mode)
			}
			Entry::Slot(Slot::Gone) => Err(io::ErrorKind::NotFound)?,
		})
	}

	async fn finish(name: &str, resolved: Resolved) -> io::Result<Stat> {
		Ok(match resolved {
			Resolved::Node(node) => node.stat(name),
			Resolved::Blob(path, mode) => {
				let meta = tokio::fs::metadata(path).await?;
				let node = Node {
					kind: NodeKind::File,
					len: meta.len(),
					mtime: meta.modified().ok(),
					mode,
					ordinal: None,
					link: None,
				};
				node.stat(name)
			}
		})
	}
}
