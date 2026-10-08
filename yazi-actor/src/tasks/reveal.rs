use anyhow::Result;
use yazi_macro::succ;
use yazi_parser::VoidForm;
use yazi_proxy::MgrProxy;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct Reveal;

impl Actor for Reveal {
	type Form = VoidForm;

	const NAME: &str = "reveal";

	/// Closes the task manager and goes to the file the hovered task is about.
	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		let target = {
			let mut ongoing = cx.tasks.scheduler.ongoing.lock();
			let id = ongoing.get_id(cx.tasks.filter, cx.tasks.cursor);
			id.and_then(|id| ongoing.get_mut(id)?.target.clone())
		};

		let Some(target) = target else { succ!() };
		act!(tasks:close, cx)?;
		succ!(MgrProxy::reveal(target));
	}
}
