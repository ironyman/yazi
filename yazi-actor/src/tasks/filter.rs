use anyhow::Result;
use yazi_macro::{render, succ};
use yazi_parser::ArrowForm;
use yazi_shared::data::Data;
use yazi_widgets::Step;

use crate::{Actor, Ctx, act};

pub struct Filter;

impl Actor for Filter {
	type Form = ArrowForm;

	const NAME: &str = "filter";

	/// Switches to the previous or next list of tasks: in progress, completed, failed, canceled.
	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		let tasks = &mut cx.tasks;
		let step = match form.step {
			Step::Prev => -1,
			Step::Offset(n) => n.signum(),
			_ => 1,
		};

		tasks.filter = tasks.filter.step(step);
		tasks.cursor = 0;
		tasks.refresh();

		act!(tasks:arrow, cx)?;
		succ!(render!());
	}
}
