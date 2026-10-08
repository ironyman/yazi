use anyhow::Result;
use yazi_core::palette::Entry;
use yazi_macro::succ;
use yazi_parser::ArrowForm;
use yazi_shared::data::Data;
use yazi_widgets::Step;

use crate::{Actor, Ctx};

pub struct Cycle;

impl Actor for Cycle {
	type Form = ArrowForm;

	const NAME: &str = "cycle";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		if let Some(&Entry::Field(f)) = cx.palette.hovered() {
			let step = match form.step {
				Step::Prev => -1,
				Step::Offset(n) => n.signum(),
				_ => 1,
			};
			cx.palette.draft.cycle(f, step);
			succ!(cx.palette.filter_apply());
		}

		let Some(&Entry::Setting(setting)) = cx.palette.hovered() else { succ!() };

		let options = setting.kind().options();
		let value = setting.value(&cx.tab().pref);
		let index = options.iter().position(|&o| o == value).unwrap_or(0);

		if let Some(next) = options.get(form.step.add(index, options.len(), 0, 0, 0)) {
			setting.apply(next);
		}
		succ!();
	}
}
