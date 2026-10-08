use anyhow::Result;
use yazi_core::mgr::Yanked;
use yazi_macro::render;
use yazi_parser::mgr::YankForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct Yank;

impl Actor for Yank {
	type Form = YankForm;

	const NAME: &str = "yank";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		cx.mark_recent();

		act!(mgr:escape_visual, cx)?;

		let files: Vec<_> = cx.tab().selected_or_hovered_files().map(Into::into).collect();
		if form.add && !cx.mgr.yanked.is_empty() {
			cx.mgr.yanked.extend(files);
		} else {
			cx.mgr.yanked = Yanked::new(form.cut, files.into_iter().collect());
		}
		render!(cx.mgr.yanked.catchup_revision(true));

		act!(mgr:escape_select, cx)
	}
}
