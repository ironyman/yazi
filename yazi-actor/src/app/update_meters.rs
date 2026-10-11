use anyhow::Result;
use yazi_macro::{render, succ};
use yazi_parser::app::UpdateMetersForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct UpdateMeters;

impl Actor for UpdateMeters {
	type Form = UpdateMetersForm;

	const NAME: &str = "update_meters";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		cx.meters.update(form.stats);
		succ!(render!());
	}
}
