use anyhow::Result;
use yazi_core::palette::Entry;
use yazi_macro::succ;
use yazi_parser::ArrowForm;
use yazi_shared::{data::Data, event::Action};
use yazi_widgets::Step;

use crate::{Actor, Ctx, act};

pub struct Nudge;

impl Actor for Nudge {
	type Form = ArrowForm;

	const NAME: &str = "nudge";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		if let Some(&Entry::Field(f)) = cx.palette.hovered()
			&& f.is_choice()
		{
			return act!(palette:cycle, cx, form);
		}

		let offset = if matches!(form.step, Step::Prev) { -1 } else { 1 };
		cx.palette.input.execute(format!("move {offset}").parse::<Action>()?.into())?;
		succ!();
	}
}
