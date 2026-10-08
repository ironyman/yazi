use std::{io, path::{Path, PathBuf}};

use hashbrown::HashSet;
use yazi_fs::engine::Engine;
use yazi_shared::{auth::{AuthArc, AuthInventory, AuthKind, Scheme}, url::{AsUrl, UrlBuf}};

use super::{Archive, Cancel, Format, Progress, Session, Slot, Source, Stage, split};

const SCHEME: &str = "archive";

/// Staged changes of an archive.
#[derive(Clone, Copy, Debug, Default)]
pub struct Summary {
	pub added:      usize,
	pub modified:   usize,
	pub removed:    usize,
	pub committing: bool,
}

impl Summary {
	#[inline]
	pub fn total(self) -> usize { self.added + self.modified + self.removed }
}

/// Whether the URL addresses the inside of an archive.
pub fn is_archive<U: AsUrl>(url: U) -> bool {
	let url = url.as_url();
	url.kind().is_mount() && url.auth().scheme == SCHEME
}

/// The URL that mounts the archive at `path`, if it's a supported archive.
pub fn mount_url(path: &Path) -> Option<UrlBuf> {
	let format = Format::from_path(path)?;
	let auth = AuthArc::new(AuthKind::Mount, SCHEME.parse::<Scheme>().ok()?, format.domain());
	UrlBuf::from(path).into_mount(auth).ok()
}

/// The archive file the URL is inside of.
pub async fn root<U: AsUrl>(url: U) -> Option<PathBuf> {
	let url = url.as_url();
	if is_archive(url) {
		Some(Archive::new(url).await.ok()?.session().path.clone())
	} else {
		url.as_local().filter(|&p| Session::find(p).is_some()).map(ToOwned::to_owned)
	}
}

/// Indexes the archive at `path` and reports whether it needs a password that
/// hasn't been given yet. A wrong password fails with `PermissionDenied`.
pub async fn mount(path: &Path, password: Option<String>, progress: Progress) -> io::Result<bool> {
	let format = Format::from_path(path)
		.ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "Unsupported archive format"))?;

	let session = Session::get(path, format);
	if password.is_some() {
		*session.password.lock() = password;
		session.reset();
	}

	let index = session.load(progress).await?;
	let locked = index.encrypted && session.password().is_none();
	if !locked
		&& index.encrypted
		&& let Err(e) = session.verify().await
	{
		// Any failure to decrypt with a given password is taken as a wrong one
		*session.password.lock() = None;
		session.reset();
		return Err(io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()));
	}
	Ok(locked)
}

/// Creates an archive at `output`, in the format its name implies, from local
/// files and directories. A 7z archive is split into volumes of `volume` bytes.
/// Zip entries are encrypted with AES-256 if `password` is given, 7z entries
/// with 7z's AES-256.
pub async fn create(
	output: PathBuf,
	inputs: Vec<PathBuf>,
	volume: Option<u64>,
	password: Option<String>,
	progress: Progress,
	cancel: Cancel,
) -> io::Result<()> {
	let format = Format::from_path(&output)
		.ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "Unsupported archive format"))?;

	let first = match volume {
		Some(_) => PathBuf::from(format!("{}.001", output.display())),
		None => output.clone(),
	};
	if tokio::fs::try_exists(&first).await? {
		return Err(io::Error::new(
			io::ErrorKind::AlreadyExists,
			format!("{} already exists", first.display()),
		));
	}

	let session = Session::get(&output, format);
	tokio::task::spawn_blocking(move || {
		session.create_from(&inputs, volume, password, &progress, &cancel)
	})
	.await?
}

/// Writes the staged changes of the archive at `path` into it.
pub async fn commit(path: &Path, progress: Progress, cancel: Cancel) -> io::Result<()> {
	let session = Session::find(path).ok_or(io::ErrorKind::NotFound)?;
	session.commit(progress, cancel).await
}

/// Drops the staged changes of the archive at `path`, returning how many there were.
pub fn discard(path: &Path) -> io::Result<usize> {
	let Some(session) = Session::find(path) else { return Ok(0) };
	session.ensure_mutable()?;

	let total = summarize(&session).total();
	session.stage.lock().slots.clear();
	std::fs::remove_dir_all(session.dir.join("b")).ok();
	Ok(total)
}

/// Staged changes of the archive the URL is inside of.
pub fn summary<U: AsUrl>(url: U) -> Option<Summary> {
	let url = url.as_url();
	if !is_archive(url) {
		return None;
	}

	let (base, ..) = url.triple();
	let path: PathBuf = base.as_os().ok()?.components().collect();
	let session = Session::find(&path)?;
	Some(summarize(&session))
}

/// The staged change to the entry at the URL: `added`, `modified` or `moved`.
pub fn status<U: AsUrl>(url: U) -> Option<&'static str> {
	let url = url.as_url();
	if !is_archive(url) {
		return None;
	}

	let (base, ..) = url.triple();
	let path: PathBuf = base.as_os().ok()?.components().collect();
	let session = Session::find(&path)?;

	let key = Archive::key(url.uri().as_os().ok()?).ok()?;
	let index = session.cached()?;
	session.stage.lock().status(&index, &key)
}

/// The staged operations on the archive at `path`, one line each, in key
/// order: `+` added, `~` modified, `>` moved, `-` removed.
pub fn pending(path: &Path) -> Vec<String> {
	let Some(session) = Session::find(path) else { return vec![] };
	let Some(index) = session.cached() else { return vec![] };

	let stage = session.stage.lock();
	let implied = implied(&stage);
	let mut keys: Vec<_> = stage.slots.keys().collect();
	keys.sort_unstable();

	keys
		.into_iter()
		.filter_map(|key| {
			let based = index.nodes.contains_key(key);
			Some(match &stage.slots[key] {
				Slot::Gone if implied.contains(key.as_str()) => return None,
				Slot::Gone => format!("- {key}"),
				Slot::Dir(_) if based => format!("~ {key}/"),
				Slot::Dir(_) => format!("+ {key}/"),
				Slot::File { source: Source::Base(k), .. } if k != key => format!("> {k} -> {key}"),
				Slot::File { .. } if based => format!("~ {key}"),
				Slot::File { .. } => format!("+ {key}"),
			})
		})
		.collect()
}

/// Archives with staged changes, and their changes.
pub fn dirty() -> Vec<(PathBuf, Summary)> {
	let mut v: Vec<_> = Session::all()
		.into_iter()
		.map(|s| (s.path.clone(), summarize(&s)))
		.filter(|(_, s)| s.total() > 0 || s.committing)
		.collect();
	v.sort_unstable_by(|a, b| a.0.cmp(&b.0));
	v
}

fn summarize(session: &Session) -> Summary {
	let mut summary = Summary { committing: session.is_committing(), ..Default::default() };
	let Some(index) = session.cached() else { return summary };

	let stage = session.stage.lock();
	let implied = implied(&stage);
	for (key, slot) in &stage.slots {
		match slot {
			Slot::Gone if implied.contains(key.as_str()) => {}
			Slot::Gone => summary.removed += 1,
			_ => match stage.status(&index, key) {
				Some("added") => summary.added += 1,
				Some(_) => summary.modified += 1,
				None => {}
			},
		}
	}
	summary
}

/// Removals that are part of a move, or of removing their parent directory.
fn implied(stage: &Stage) -> HashSet<&str> {
	let moved = stage.slots.values().filter_map(|slot| match slot {
		Slot::File { source: Source::Base(k), .. } => Some(k.as_str()),
		_ => None,
	});

	let nested = stage.slots.iter().filter_map(|(k, slot)| {
		let parent = stage.slots.get(split(k).0);
		(matches!(slot, Slot::Gone) && matches!(parent, Some(Slot::Gone))).then_some(k.as_str())
	});

	moved.chain(nested).collect()
}

// --- Inject
inventory::submit! {
	AuthInventory {
		get: |scheme, domain| {
			(*scheme == SCHEME && Format::from_domain(domain).is_some())
				.then(|| AuthArc::new(AuthKind::Mount, scheme.clone(), domain.clone()))
		},
	}
}

impl Session {
	/// Forgets everything derived from the previous password.
	pub(super) fn reset(&self) { *self.handle.lock().unwrap() = Default::default(); }

	/// Decrypts the smallest non-empty file to check the password.
	async fn verify(self: &std::sync::Arc<Self>) -> io::Result<()> {
		let index = self.index().await?;
		// Empty files have no data to decrypt
		let files = index.nodes.values().filter(|n| n.kind == super::NodeKind::File && n.len > 0);
		let Some(ordinal) = files.min_by_key(|n| n.len).and_then(|n| n.ordinal) else {
			return Ok(());
		};

		let me = self.clone();
		tokio::task::spawn_blocking(move || me.extract_to(ordinal, &mut io::sink())).await?
	}
}
