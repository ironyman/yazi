use std::{fs::File, io::{self, BufReader, BufWriter, Write}, path::{Path, PathBuf}, sync::Arc};

use yazi_config::YAZI;

use super::{Format, Handle, Session};

impl Session {
	/// Extracts the entry at `ordinal` into the extraction cache, returning its
	/// local path. Callers serialize extractions.
	pub(super) async fn extract(self: &Arc<Self>, ordinal: usize) -> io::Result<PathBuf> {
		let path = self.dir.join("x").join(format!("{}-{ordinal}", self.generation()));
		if tokio::fs::try_exists(&path).await? {
			return Ok(path);
		}

		let me = self.clone();
		let dst = path.clone();
		tokio::task::spawn_blocking(move || {
			let tmp = me.tmp();
			std::fs::create_dir_all(dst.parent().unwrap())?;

			let mut out = BufWriter::new(File::create(&tmp)?);
			let result = me.extract_to(ordinal, &mut out).and_then(|()| out.into_inner()?.sync_all());
			match result {
				Ok(()) => std::fs::rename(&tmp, &dst),
				Err(e) => {
					std::fs::remove_file(&tmp).ok();
					Err(e)
				}
			}
		})
		.await??;

		Ok(path)
	}

	/// Writes the content of the entry at `ordinal` into `out`. Blocking.
	pub(super) fn extract_to(&self, ordinal: usize, out: &mut dyn Write) -> io::Result<()> {
		let password = self.password();
		match self.format {
			Format::Zip => {
				let mut handle = self.handle.lock().unwrap();
				if !matches!(*handle, Handle::Zip(_)) {
					*handle = Handle::Zip(zip::ZipArchive::new(BufReader::new(File::open(&self.path)?))?);
				}
				let Handle::Zip(zip) = &mut *handle else { unreachable!() };

				let mut file = match &password {
					Some(p) => zip.by_index_decrypt(ordinal, p.as_bytes()),
					None => zip.by_index(ordinal),
				}
				.map_err(zip_err)?;
				io::copy(&mut file, out)?;
			}
			Format::SevenZ if YAZI.mgr.archive_7z_native.get() => {
				let mut handle = self.handle.lock().unwrap();
				if !matches!(*handle, Handle::SevenZ(_)) {
					*handle = Handle::SevenZ(Box::new(open_7z(&self.path, password.as_deref())?));
				}
				let Handle::SevenZ(archive) = &mut *handle else { unreachable!() };

				out.write_all(&archive.extract_entry_to_vec_by_index(ordinal).map_err(sevenz_err)?)?;
			}
			Format::SevenZ => {
				let mut archive = open_7z_stream(&self.path, &password.unwrap_or_default())?;
				let mut it = archive.entries().map_err(sevenz_err)?;
				for _ in 0..ordinal {
					it.next().transpose().map_err(sevenz_err)?;
				}
				it.next().transpose().map_err(sevenz_err)?.ok_or(io::ErrorKind::NotFound)?;
				it.extract_current_to(&mut &mut *out).map_err(sevenz_err)?;
			}
			Format::Tar(codec) => {
				let reader = codec.reader(BufReader::new(File::open(&self.path)?))?;
				let mut tar = tar::Archive::new(reader);
				let mut entry = tar.entries()?.nth(ordinal).ok_or(io::ErrorKind::NotFound)??;
				io::copy(&mut entry, out)?;
			}
		}
		Ok(())
	}
}

pub(super) fn open_7z(
	path: &Path,
	password: Option<&str>,
) -> io::Result<zesven::Archive<zesven::read::ArchiveSource>> {
	match password {
		Some(p) => zesven::Archive::open_path_with_password(path, p),
		None => zesven::Archive::open_path(path),
	}
	.map_err(sevenz_err)
}

/// Opens a 7z archive for sequential reading, through all its volumes if split.
pub(super) fn open_7z_stream(
	path: &Path,
	password: &str,
) -> io::Result<zesven::StreamingArchive<zesven::read::ArchiveSource>> {
	use zesven::read::ArchiveSource;

	let source = if is_split(path) {
		let volumes = zesven::volume::MultiVolumeReader::open(path).map_err(sevenz_err)?;
		ArchiveSource::Volumes(Box::new(volumes))
	} else {
		ArchiveSource::Single(BufReader::new(File::open(path)?))
	};
	zesven::StreamingArchive::open(source, password).map_err(sevenz_err)
}

/// Whether the path is the first volume of a split archive.
pub(super) fn is_split(path: &Path) -> bool {
	path.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase().ends_with(".001"))
}

pub(super) fn sevenz_err(e: zesven::Error) -> io::Error {
	match e {
		zesven::Error::Io(e) => e,
		e if e.is_encryption_error() => io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()),
		e => io::Error::other(e.to_string()),
	}
}

pub(super) fn zip_err(e: zip::result::ZipError) -> io::Error {
	use zip::result::ZipError;

	match e {
		ZipError::InvalidPassword | ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED) => {
			io::Error::new(io::ErrorKind::PermissionDenied, e.to_string())
		}
		e => e.into(),
	}
}
