use anyhow::Result;
use yazi_macro::{render, succ};
use yazi_parser::pager::PushForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Push;

impl Actor for Push {
	type Form = PushForm;

	const NAME: &str = "push";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		succ!(render!(cx.pager.push(form.ticket, form.line)));
	}
}
