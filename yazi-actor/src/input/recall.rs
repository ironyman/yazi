use anyhow::Result;
use yazi_core::input::InputMutGuard;
use yazi_macro::succ;
use yazi_parser::{input::RecallForm, mgr::FindArrowForm};
use yazi_shared::data::Data;
use yazi_widgets::Step;

use crate::{Actor, Ctx, act};

pub struct Recall;

impl Actor for Recall {
	type Form = RecallForm;

	const NAME: &str = "recall";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		if form.found && cx.input.main.visible && cx.input.main.name == "find" {
			let prev = matches!(form.step, Step::Prev | Step::Offset(..0));
			return act!(mgr:find_arrow, cx, FindArrowForm { prev });
		}

		let Some(input) = cx.input.lock_mut() else {
			succ!();
		};

		match input {
			InputMutGuard::Main(input) => {
				let entries = input.histories.get(&input.main.history.name);
				input.main.recall(entries, form.step)
			}
			InputMutGuard::Alt(input, mut guard) => {
				let entries = input.histories.get(&guard.history.name);
				guard.recall(entries, form.step)
			}
		}
	}
}
