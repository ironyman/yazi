use anyhow::Result;
use yazi_macro::succ;
use yazi_parser::palette::UpdateFilesForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct UpdateFiles;

impl Actor for UpdateFiles {
	type Form = UpdateFilesForm;

	const NAME: &str = "update_files";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		succ!(cx.palette.update_files(form.files))
	}
}
