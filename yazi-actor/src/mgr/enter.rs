use std::{io, path::PathBuf};

use anyhow::Result;
use yazi_config::YAZI;
use yazi_core::{mgr::CdSource, tasks::TaskOpt};
use yazi_macro::succ;
use yazi_parser::VoidForm;
use yazi_proxy::{InputProxy, MgrProxy, TasksProxy};
use yazi_scheduler::{NotifyProxy, TaskHandle, custom::{CustomIn, CustomOut}};
use yazi_shared::{data::Data, url::{UrlBuf, UrlLike}};
use yazi_vfs::engine::archive;
use yazi_widgets::input::InputEvent;

use crate::{Actor, Ctx, act, mgr::Archive};

pub struct Enter;

impl Actor for Enter {
	type Form = VoidForm;

	const NAME: &str = "enter";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		let Some(h) = cx.hovered() else { succ!() };

		if h.is_dir() {
			let url = h.physical().to_owned();
			act!(mgr:cd, cx, (url, CdSource::Enter))
		} else if let Some(path) = h.url.as_local()
			&& let Some(url) = archive::mount_url(path)
		{
			tokio::spawn(Self::mount(path.to_owned(), url));
			succ!()
		} else {
			succ!()
		}
	}
}

impl Enter {
	/// Indexes the archive, asking for its password if needed, then enters it.
	async fn mount(path: PathBuf, url: UrlBuf) {
		const SLOW: u64 = 64 * 1024 * 1024;

		let name = Archive::name(&path);
		let slow = url.auth().domain.starts_with(b"tar")
			|| tokio::fs::metadata(&path).await.is_ok_and(|m| m.len() > SLOW);

		let task = if slow {
			TasksProxy::spawn(TaskOpt::Custom(CustomIn::new(format!("Index {name}"), true))).await.ok()
		} else {
			None
		};

		let mut password = None;
		let result = loop {
			let progress = match &task {
				Some(t) => Archive::reporter(t.id),
				None => Box::new(|_, _| ()),
			};

			let wrong = match archive::mount(&path, password.take(), progress).await {
				Ok(false) => break Ok(()),
				Ok(true) => false,
				Err(e) if e.kind() == io::ErrorKind::PermissionDenied => true,
				Err(e) => break Err(e),
			};

			let title = if wrong { format!("{name} (wrong password)") } else { name.clone() };
			let mut rx = InputProxy::show(YAZI.input.password(&title));
			match rx.recv().await {
				Some(InputEvent::Submit(s)) => password = Some(s),
				_ => break Err(io::ErrorKind::Interrupted.into()),
			}
		};

		Self::finish(task, &result);
		match result {
			Ok(()) => MgrProxy::cd(url, CdSource::Enter),
			Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
			Err(e) => NotifyProxy::push_error(format!("Failed to open {name}"), e.to_string()),
		}
	}

	fn finish(task: Option<TaskHandle>, result: &io::Result<()>) {
		let Some(task) = task else { return };
		TasksProxy::output(task.id, match result {
			Ok(()) => CustomOut::Succ(vec![]),
			Err(e) => CustomOut::Fail(e.to_string()),
		});
	}
}
