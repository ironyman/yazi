use anyhow::Result;
use yazi_config::popup::Palette;
use yazi_core::palette::{Entry, SettingKind};
use yazi_macro::{emit, render, succ};
use yazi_parser::{ArrowForm, help::CloseForm};
use yazi_proxy::PickProxy;
use yazi_shared::{Layer, data::Data};
use yazi_widgets::{Step, input::InputEvent};

use crate::{Actor, Ctx, act, input};

pub struct Close;

impl Actor for Close {
	type Form = CloseForm;

	const NAME: &str = "close";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		cx.palette.visible = false;
		render!();

		let setting = match cx.palette.hovered().filter(|_| form.submit) {
			None => succ!(),
			Some(Entry::Chord(chord)) => succ!(emit!(Seq(chord.to_seq(Layer::Mgr)))),
			Some(&Entry::Setting(setting)) => setting,
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
