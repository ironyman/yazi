use std::{io, path::{Component, Path, PathBuf}, sync::Arc};

use yazi_fs::{engine::{Attrs, Capabilities, Engine, Transmit}, stat::Stat};
use yazi_shared::{auth::AuthKind, path::{DynPath, PathBufDyn}, strand::AsStrand, url::{Url, UrlBuf, UrlCow}};

use super::{DirEntry, Format, ReadDir, Session};

pub struct Archive<'a> {
	url:     Url<'a>,
	session: Arc<Session>,
	key:     String,
}

impl<'a> Engine for Archive<'a> {
	type Demand = super::Demand;
	type File = tokio::fs::File;
	type Me<'b> = Archive<'b>;
	type ReadDir = ReadDir;
	type UrlCow = UrlCow<'a>;

	async fn absolute(&self) -> io::Result<Self::UrlCow> { Ok(self.url.into()) }

	async fn canonicalize(&self) -> io::Result<UrlBuf> { Ok(self.url.to_owned()) }

	async fn capabilities(&self) -> io::Result<Capabilities> {
		let mut caps = Capabilities::for_kind(AuthKind::Mount);
		caps.copy_to = false;
		caps.reroute = 0;
		caps.trash = true;
		Ok(caps)
	}

	async fn casefold(&self) -> io::Result<UrlBuf> { Ok(self.url.to_owned()) }

	async fn copy_to(&self, _to: Url<'_>, _attrs: Attrs) -> io::Result<Transmit> {
		Ok(Transmit::unsupported())
	}

	async fn create_dir(&self) -> io::Result<()> { self.session.create_dir(&self.key).await }

	async fn hard_link<P>(&self, _to: P) -> io::Result<()>
	where
		P: DynPath,
	{
		Err(io::Error::new(io::ErrorKind::Unsupported, "Hard links are not supported in archives"))
	}

	async fn metadata(&self) -> io::Result<Stat> { self.session.stat(&self.key).await }

	async fn new<'b>(url: Url<'b>) -> io::Result<Self::Me<'b>> {
		let format = Format::from_domain(&url.auth().domain).ok_or_else(|| {
			io::Error::new(io::ErrorKind::InvalidInput, format!("Not an archive URL: {url}"))
		})?;

		let (base, ..) = url.triple();
		let path: PathBuf = base.as_os()?.components().collect();
		let key = Self::key(url.uri().as_os()?)?;

		Ok(Self::Me { url, session: Session::get(&path, format), key })
	}

	async fn read_dir(self) -> io::Result<Self::ReadDir> {
		let entries = self.session.read_dir(&self.key).await?;

		let mut items = Vec::with_capacity(entries.len());
		for (name, stat) in entries {
			items.push(DirEntry { url: self.url.try_join(name)?, stat });
		}
		Ok(ReadDir(items.into_iter()))
	}

	async fn read_link(&self) -> io::Result<PathBufDyn> {
		Ok(PathBuf::from(self.session.read_link(&self.key).await?).into())
	}

	async fn remove_dir(&self) -> io::Result<()> { self.session.remove(&self.key, true, false).await }

	async fn remove_dir_all(&self) -> io::Result<()> {
		self.session.remove(&self.key, true, true).await
	}

	async fn remove_file(&self) -> io::Result<()> {
		self.session.remove(&self.key, false, false).await
	}

	async fn rename<P>(&self, to: P) -> io::Result<()>
	where
		P: DynPath,
	{
		let to = to.dyn_path().as_os()?;
		let Ok(rest) = to.strip_prefix(&self.session.path) else {
			return Err(io::ErrorKind::CrossesDevices.into());
		};

		self.session.rename(&self.key, &Self::key(rest)?).await
	}

	async fn set_attrs(&self, attrs: Attrs) -> io::Result<()> {
		self.session.set_attrs(&self.key, attrs).await
	}

	async fn symlink<S, F>(&self, _original: S, _is_dir: F) -> io::Result<()>
	where
		S: AsStrand,
		F: AsyncFnOnce() -> io::Result<bool>,
	{
		Err(io::Error::new(io::ErrorKind::Unsupported, "Symbolic links are not supported in archives"))
	}

	async fn symlink_metadata(&self) -> io::Result<Stat> { self.metadata().await }

	/// Removals are staged, so they can be undone by discarding until committed.
	async fn trash(&self) -> io::Result<()> {
		let dir = self.metadata().await?.is_dir();
		self.session.remove(&self.key, dir, true).await
	}

	#[inline]
	fn url(&self) -> Url<'_> { self.url }
}

impl Archive<'_> {
	#[inline]
	pub(super) fn session(&self) -> &Arc<Session> { &self.session }

	#[inline]
	pub(super) fn entry_key(&self) -> &str { &self.key }

	pub(super) fn key(path: &Path) -> io::Result<String> {
		let mut key = String::new();
		for c in path.components() {
			let Component::Normal(name) = c else {
				return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid path in archive"));
			};
			if !key.is_empty() {
				key.push('/');
			}
			key.push_str(&name.to_string_lossy());
		}
		Ok(key)
	}
}
