use anyhow::Result;
use tokio::{io::{AsyncBufReadExt, BufReader}, select};
use yazi_config::popup::{Pager, Palette};
use yazi_core::{mgr::{CdSource, MgrSnap}, notify::{MessageLevel, MessageOpt}, palette::{Entry, SettingKind}};
use yazi_fs::Splatter;
use yazi_macro::{emit, relay, render, succ};
use yazi_parser::{ArrowForm, palette::CloseForm};
use yazi_proxy::{MgrProxy, PickProxy};
use yazi_scheduler::process::{self, ShellOpt};
use yazi_shared::{Layer, data::Data, event::Action, id::Id};
use yazi_widgets::{Step, input::InputEvent};

use crate::{Actor, Ctx, act, input};

pub struct Close;

impl Actor for Close {
	type Form = CloseForm;

	const NAME: &str = "close";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		let palette = &mut cx.palette;
		let entry = palette.hovered().filter(|_| form.submit).cloned();

		palette.close();
		render!();

		let setting = match entry {
			None => succ!(),
			Some(Entry::Chord(chord)) => succ!(emit!(Seq(chord.to_seq(Layer::Mgr)))),
			Some(Entry::Action(cmd)) => return Self::run(cx, cmd),
			Some(Entry::Run(cmd)) => return Self::run(cx, &cmd),
			Some(Entry::File { url, dir: true, .. }) => succ!(MgrProxy::cd(url, CdSource::Cd)),
			Some(Entry::File { url, dir: false, .. }) => succ!(MgrProxy::reveal(url)),
			Some(Entry::Shell(cmd)) if form.pager => {
				cx.input.histories.remember("shell", &cmd);
				succ!(Self::page(cx, cmd))
			}
			Some(Entry::Shell(cmd)) => {
				cx.input.histories.remember("shell", &cmd);
				succ!(emit!(Call(relay!(mgr:shell, [cmd]))))
			}
			Some(Entry::Setting(setting)) => setting,
		};

		match setting.kind() {
			SettingKind::Toggle => act!(palette:cycle, cx, ArrowForm { step: Step::Next }),
			SettingKind::Select(options) => {
				let cfg = Palette::pick(setting.name(), options.iter().map(|&o| o.to_owned()).collect());
				tokio::spawn(async move {
					if let Some(i) = PickProxy::show(cfg).await {
						setting.apply(options[i]);
					}
				});
				succ!();
			}
			SettingKind::Text => {
				let value = setting.value(&cx.tab().pref).into_owned();
				let mut rx = input!(cx, Palette::input(setting.name(), value))?;
				tokio::spawn(async move {
					if let Some(InputEvent::Submit(value)) = rx.recv().await {
						setting.apply(&value);
					}
				});
				succ!();
			}
		}
	}
}

impl Close {
	fn run(cx: &mut Ctx, cmd: &str) -> Result<Data> {
		match cmd.parse::<Action>() {
			Ok(mut action) => {
				action.layer = action.layer.or(Layer::Mgr);
				succ!(emit!(Seq(vec![action.into()])))
			}
			Err(e) => act!(notify:push, cx, MessageOpt {
				title:   "Command Palette".to_owned(),
				content: format!("Invalid command {cmd:?}: {e}"),
				level:   MessageLevel::Error,
				timeout: std::time::Duration::from_secs(5),
			}),
		}
	}

	fn page(cx: &mut Ctx, cmd: String) {
		let opt = ShellOpt {
			cwd:    cx.cwd().clone(),
			cmd:    Splatter::new(MgrSnap::from(&cx.mgr)).splat(&*cmd),
			block:  false,
			orphan: false,
		};

		let position = Pager::position();
		let height = cx.mgr.area(position).height;

		let pager = &mut cx.pager;
		pager.close();

		pager.visible = true;
		pager.title = cmd;
		pager.position = position;
		pager.height = height;
		pager.offset = 0;
		pager.ticket = Id::unique();
		pager.handle = Some(tokio::spawn(Self::pipe(opt, pager.ticket)).abort_handle());
	}

	async fn pipe(opt: ShellOpt, ticket: Id) {
		let push = |line: String| emit!(Call(relay!(pager:push, [line]).with("ticket", ticket)));

		let mut child = match process::shell(opt).await {
			Ok(child) => child,
			Err(e) => return push(format!("[Failed to run the command: {e}]")),
		};

		let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
		let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();
		let status = loop {
			select! {
				Ok(Some(line)) = stdout.next_line() => push(line),
				Ok(Some(line)) = stderr.next_line() => push(line),
				status = child.wait() => break status,
			}
		};

		while let Ok(Some(line)) = stdout.next_line().await {
			push(line);
		}
		while let Ok(Some(line)) = stderr.next_line().await {
			push(line);
		}

		push(match status.map(|s| s.code()) {
			Ok(Some(code)) => format!("[Exited with status code: {code}]"),
			Ok(None) => "[Process terminated by signal]".to_owned(),
			Err(e) => format!("[Failed to wait for the command: {e}]"),
		});
	}
}
