use std::{path::{Path, PathBuf}, sync::Mutex, time::{Duration, Instant}};

use anyhow::Result;
use yazi_config::popup::ConfirmCfg;
use yazi_core::{palette::{Draft, PaletteMode, Source}, tasks::TaskOpt};
use yazi_macro::{emit, relay, succ};
use yazi_parser::{mgr::{ArchiveForm, ArchiveOp}, palette::ShowForm};
use yazi_proxy::{ConfirmProxy, MgrProxy, TasksProxy};
use yazi_scheduler::{NotifyProxy, custom::{CustomIn, CustomOut}};
use yazi_shared::{data::Data, id::Id, url::{UrlBuf, UrlLike}};
use yazi_vfs::engine::archive::{self, Format, Summary};

use crate::{Actor, Ctx, act};

pub struct Archive;

/// An archive to create: the output, inputs, volume size and password.
pub(crate) type Job = (PathBuf, Vec<PathBuf>, Option<u64>, Option<String>);

impl Actor for Archive {
	type Form = ArchiveForm;

	const NAME: &str = "archive";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		let cwd = cx.cwd().clone();
		let hovered = cx.hovered().map(|h| h.url.clone());

		if form.op == ArchiveOp::Create {
			let Some(draft) = Self::draft(cx) else {
				succ!(NotifyProxy::push_warn(
					"Create archive",
					"Archives can only be created in local directories"
				));
			};

			act!(palette:show, cx, ShowForm { mode: PaletteMode::Archive })?;
			cx.palette.draft = draft;
			cx.palette.filter_apply();
			cx.palette.load_field();
			succ!();
		}

		tokio::spawn(async move {
			let root = match archive::root(&cwd).await {
				Some(path) => Some(path),
				None => match hovered {
					Some(url) => archive::root(&url).await,
					None => None,
				},
			};

			let Some(path) = root else {
				return NotifyProxy::push_warn("Archive", "Not inside an archive with staged changes");
			};

			match form.op {
				ArchiveOp::Status => Self::status(path).await,
				ArchiveOp::Commit => Self::commit(path).await,
				ArchiveOp::Discard => Self::discard(path).await,
				ArchiveOp::Create => unreachable!(),
			}
		});
		succ!();
	}
}

impl Archive {
	/// Lists the pending operations in a modal, offering to commit them.
	async fn status(path: PathBuf) {
		let name = Self::name(&path);
		let ops = archive::pending(&path);
		let committing = Self::summary(&path).committing;
		if ops.is_empty() {
			return NotifyProxy::push_info(name, "No staged changes");
		}

		// Changes stay listed until the commit has fully succeeded
		if ConfirmProxy::show(ConfirmCfg::pending(name, ops, committing)).await && !committing {
			Self::commit(path).await;
		}
	}

	async fn commit(path: PathBuf) {
		let name = Self::name(&path);
		if Self::summary(&path).total() == 0 {
			return NotifyProxy::push_info(name, "Nothing to commit");
		}

		let r#in = CustomIn::new(format!("Commit {name}"), true).with_target(path.as_path());
		let scope = r#in.scope().clone();
		let Ok(task) = TasksProxy::spawn(TaskOpt::Custom(r#in)).await else { return };

		let cancel = Box::new(move || scope.is_cancelled());
		match archive::commit(&path, Self::reporter(task.id), cancel).await {
			Ok(()) => {
				TasksProxy::output(task.id, CustomOut::Succ(vec![]));
				NotifyProxy::push_info(name, "Committed staged changes");
				emit!(Call(relay!(mgr:refresh)));
			}
			Err(e) => {
				TasksProxy::output(task.id, CustomOut::Fail(e.to_string()));
				NotifyProxy::push_error(format!("Failed to commit {name}"), e.to_string());
			}
		}
	}

	async fn discard(path: PathBuf) {
		let name = Self::name(&path);
		let total = Self::summary(&path).total();
		if total == 0 {
			return NotifyProxy::push_info(name, "Nothing to discard");
		} else if !ConfirmProxy::show(ConfirmCfg::discard(name.clone(), total)).await {
			return;
		}

		match archive::discard(&path) {
			Ok(_) => emit!(Call(relay!(mgr:refresh))),
			Err(e) => NotifyProxy::push_error(format!("Failed to discard {name}"), e.to_string()),
		}
	}

	/// The archive form, prefilled from the marked files or the hovered directory.
	fn draft(cx: &Ctx) -> Option<Draft> {
		let cwd = cx.cwd().as_local()?.to_path_buf();
		let marked: Vec<PathBuf> =
			cx.tab().selected.files().filter_map(|f| f.url.as_local()).map(Into::into).collect();
		// Yanked files inside archives or on remote hosts can't be read as local paths
		let yanked = cx.mgr.yanked.urls().filter_map(|u| u.as_local()).map(Into::into).collect();
		let dir = cx.hovered().filter(|h| h.is_dir()).and_then(|h| h.url.as_local()).unwrap_or(&cwd);

		Some(Draft {
			source: if marked.is_empty() { Source::Dir } else { Source::Marked },
			marked,
			yanked,
			dir: dir.to_string_lossy().into_owned(),
			kinds: Format::all().map(Format::ext).collect(),
			cwd,
			..Default::default()
		})
	}

	/// Validates the form, returning the output, inputs, volume size and password.
	pub(crate) fn submit(d: &Draft) -> Result<Job, String> {
		let format = Format::all().nth(d.kind).ok_or("Choose an archive type")?;
		let inputs = if let Some(files) = d.files() {
			files.to_vec()
		} else if d.dir.trim().is_empty() {
			return Err("Choose a directory to archive".to_owned());
		} else {
			vec![d.cwd.join(d.dir.trim())]
		};

		let volume = match d.volume.trim() {
			"" => None,
			_ if format != Format::SevenZ => None,
			s => Some(Self::parse_size(s).ok_or_else(|| format!("Invalid volume size: {s} MB"))?),
		};

		let password = Some(d.password.clone())
			.filter(|p| !p.is_empty() && matches!(format, Format::Zip | Format::SevenZ));

		let name = match d.name.trim() {
			"" => d.default_name(),
			s => s.to_owned(),
		};
		let mut output = d.cwd.join(name);
		if Format::from_path(&output) != Some(format) {
			output.as_mut_os_string().push(format!(".{}", format.ext()));
		}
		Ok((output, inputs, volume, password))
	}

	/// Creates the archive as a task, then reveals it.
	pub(crate) async fn create(
		output: PathBuf,
		inputs: Vec<PathBuf>,
		volume: Option<u64>,
		password: Option<String>,
	) {
		for input in &inputs {
			if tokio::fs::symlink_metadata(input).await.is_err() {
				return NotifyProxy::push_warn("Create archive", format!("Not found: {}", input.display()));
			}
		}

		let name = Self::name(&output);
		let first = match volume {
			Some(_) => PathBuf::from(format!("{}.001", output.display())),
			None => output.clone(),
		};
		let r#in = CustomIn::new(format!("Create {name}"), true).with_target(first.as_path());
		let scope = r#in.scope().clone();
		let Ok(task) = TasksProxy::spawn(TaskOpt::Custom(r#in)).await else { return };

		let cancel = Box::new(move || scope.is_cancelled());
		let progress = Self::reporter(task.id);
		match archive::create(output.clone(), inputs, volume, password, progress, cancel).await {
			Ok(()) => {
				TasksProxy::output(task.id, CustomOut::Succ(vec![]));
				MgrProxy::reveal(UrlBuf::from(first));
			}
			Err(e) => {
				TasksProxy::output(task.id, CustomOut::Fail(e.to_string()));
				NotifyProxy::push_error(format!("Failed to create {name}"), e.to_string());
			}
		}
	}

	/// Parses sizes in megabytes like `700` or `4.5`, or with a unit like `500K` or `4.7G`, in binary units.
	fn parse_size(s: &str) -> Option<u64> {
		let s = s.trim().to_ascii_uppercase();
		let s = s.strip_suffix("IB").or_else(|| s.strip_suffix('B')).unwrap_or(&s);
		let (num, unit) = match s.char_indices().find(|(_, c)| c.is_ascii_alphabetic()) {
			Some((i, _)) => s.split_at(i),
			None => (s, ""),
		};

		let shift = match unit {
			"" => 20,
			"K" => 10,
			"M" => 20,
			"G" => 30,
			"T" => 40,
			_ => return None,
		};

		let n: f64 = num.trim().parse().ok()?;
		let bytes = n * (1u64 << shift) as f64;
		(bytes >= 1.0 && bytes < u64::MAX as f64).then_some(bytes as u64)
	}

	fn summary(path: &Path) -> Summary {
		archive::dirty().into_iter().find(|(p, _)| p == path).map(|(_, s)| s).unwrap_or_default()
	}

	pub(super) fn name(path: &Path) -> String {
		path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
	}

	/// Forwards byte progress to the task, at most every 100ms.
	pub(super) fn reporter(id: Id) -> Box<dyn Fn(u64, u64) + Send + Sync> {
		let pending = Mutex::new((0, 0, Instant::now()));
		Box::new(move |processed, workload| {
			let Ok(mut p) = pending.lock() else { return };
			p.0 += processed;
			p.1 += workload;
			if workload == 0 && p.2.elapsed() < Duration::from_millis(100) {
				return;
			}

			let (processed, workload) = (p.0, p.1);
			*p = (0, 0, Instant::now());
			TasksProxy::output(id, CustomOut::Progress {
				total: 0,
				success: 0,
				failed: 0,
				workload,
				processed,
			});
		})
	}
}
