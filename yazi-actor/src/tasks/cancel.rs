use anyhow::Result;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct Cancel;

impl Actor for Cancel {
	type Form = VoidForm;

	const NAME: &str = "cancel";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		let tasks = &mut cx.tasks;

		let Some(id) = tasks.scheduler.ongoing.lock().get_id(tasks.filter, tasks.cursor) else {
			succ!();
		};

		// Finished tasks are only history, so they are removed from it instead
		let done = if tasks.filter.is_finished() || !tasks.scheduler.ongoing.lock().exists(id) {
			tasks.scheduler.ongoing.lock().dismiss(id)
		} else {
			tasks.scheduler.cancel(id)
		};
		if !done {
			succ!();
		}

		tasks.refresh();
		act!(tasks:arrow, cx)?;
		succ!(render!());
	}
}
