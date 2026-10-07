use std::path::MAIN_SEPARATOR;

use anyhow::Result;
use yazi_core::palette::Entry;
use yazi_macro::{render, succ};
use yazi_parser::{ArrowForm, VoidForm};
use yazi_shared::data::Data;
use yazi_widgets::Step;

use crate::{Actor, Ctx, act};

pub struct Complete;

impl Actor for Complete {
	type Form = VoidForm;

	const NAME: &str = "complete";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		let palette = &mut cx.palette;
		let value = match palette.hovered() {
			None => succ!(),
			Some(Entry::Setting(_)) => return act!(palette:cycle, cx, ArrowForm { step: Step::Next }),
			Some(Entry::Chord(c)) => c.desc_or_run().into_owned(),
			Some(Entry::Action(a)) => format!("{a} "),
			Some(Entry::File { label, dir: true, .. }) => format!("{label}{MAIN_SEPARATOR}"),
			Some(Entry::File { label, .. } | Entry::Shell(label) | Entry::Run(label)) => label.clone(),
			Some(Entry::Recent(r)) => r.url.to_string(),
		};

		let value = format!("{}{value}", palette.prefix());
		palette.input.set_value(value)?;
		palette.filter_apply();
		succ!(render!());
	}
}
