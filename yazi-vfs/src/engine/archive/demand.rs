use std::io;

use yazi_fs::engine::{Attrs, Engine, FileBuilder};
use yazi_shared::url::AsUrl;

use super::Archive;

#[derive(Clone, Copy, Default)]
pub struct Demand(yazi_fs::engine::Demand);

impl FileBuilder for Demand {
	type File = tokio::fs::File;

	fn append(&mut self, append: bool) -> &mut Self {
		self.0.append = append;
		self
	}

	fn attrs(&mut self, attrs: Attrs) -> &mut Self {
		self.0.attrs = attrs;
		self
	}

	fn create(&mut self, create: bool) -> &mut Self {
		self.0.create = create;
		self
	}

	fn create_new(&mut self, create_new: bool) -> &mut Self {
		self.0.create_new = create_new;
		self
	}

	async fn open<U>(&self, url: U) -> io::Result<Self::File>
	where
		U: AsUrl,
	{
		let engine = Archive::new(url.as_url()).await?;
		let (session, key) = (engine.session(), engine.entry_key());

		let d = self.0;
		let path = if d.write || d.append || d.create || d.create_new || d.truncate {
			session.checkin(key, &d).await?
		} else {
			session.checkout(key).await?
		};

		tokio::fs::OpenOptions::new()
			.read(d.read)
			.write(d.write)
			.append(d.append)
			.truncate(d.truncate)
			.open(path)
			.await
	}

	fn read(&mut self, read: bool) -> &mut Self {
		self.0.read = read;
		self
	}

	fn truncate(&mut self, truncate: bool) -> &mut Self {
		self.0.truncate = truncate;
		self
	}

	fn write(&mut self, write: bool) -> &mut Self {
		self.0.write = write;
		self
	}
}
