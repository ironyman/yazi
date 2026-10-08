use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use tokio::{select, time};
use yazi_config::popup::ConfirmCfg;
use yazi_core::app::QuitOpt;
use yazi_macro::succ;
use yazi_parser::{app::QuitForm, spark::SparkKind};
use yazi_proxy::{AppProxy, ConfirmProxy};
use yazi_shared::{data::Data, strand::{Strand, StrandLike, ToStrandJoin}, url::AsUrl};
use yazi_vfs::engine::archive::Summary;

use crate::{Actor, Ctx, act};

pub struct Quit;

impl Actor for Quit {
	type Form = QuitForm;

	const NAME: &str = "quit";

	fn act(cx: &mut Ctx, Self::Form { opt, .. }: Self::Form) -> Result<Data> {
		let ongoing = cx.tasks.scheduler.ongoing.clone();
		let (left, left_titles) = {
			let ongoing = ongoing.lock();
			(ongoing.len(), ongoing.values().take(11).map(|t| t.title.clone()).collect())
		};

		let staged = yazi_vfs::engine::archive::dirty();
		if left == 0 && staged.is_empty() {
			return act!(app:quit, cx, opt);
		} else if !staged.is_empty() {
			return Self::confirm_staged(staged, left_titles, opt);
		}

		tokio::spawn(async move {
			let mut i = 0;
			let token = ConfirmProxy::show_sync(ConfirmCfg::quit(left, left_titles));
			loop {
				select! {
					_ = time::sleep(Duration::from_millis(50)) => {
						i += 1;
						if i > 40 { break }
						else if ongoing.lock().is_empty() {
							AppProxy::quit(opt);
							return;
						}
					}
					b = token.future() => {
						if b {
							AppProxy::quit(opt);
						}
						return;
					}
				}
			}

			if token.future().await {
				AppProxy::quit(opt);
			}
		});
		succ!();
	}

	fn hook(cx: &Ctx, _form: &Self::Form) -> Option<SparkKind> {
		cx.source().is_key().then_some(SparkKind::KeyQuit)
	}
}

impl Quit {
	/// Staged archive changes live only in this process, so unlike tasks, they
	/// never resolve themselves and quitting always waits for the answer.
	fn confirm_staged(
		staged: Vec<(PathBuf, Summary)>,
		tasks: Vec<String>,
		opt: QuitOpt,
	) -> Result<Data> {
		let archives = staged
			.into_iter()
			.map(|(path, s)| {
				let state = if s.committing {
					"commit in progress".to_owned()
				} else {
					format!("+{} ~{} -{}", s.added, s.modified, s.removed)
				};
				format!("{} ({state})", path.display())
			})
			.collect();

		let token = ConfirmProxy::show_sync(ConfirmCfg::quit_staged(archives, tasks));
		tokio::spawn(async move {
			if token.future().await {
				AppProxy::quit(opt);
			}
		});
		succ!();
	}

	pub(super) fn with_selected<I>(selected: I)
	where
		I: IntoIterator,
		I::Item: AsUrl,
	{
		let paths = selected.into_iter().join(Strand::Utf8("\n"));
		if !paths.is_empty() {
			AppProxy::quit(QuitOpt { selected: Some(paths), ..Default::default() });
		}
	}
}
