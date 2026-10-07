use anyhow::Result;
use yazi_actor::Ctx;
use yazi_macro::{render, succ};
use yazi_parser::app::ReflowForm;
use yazi_shared::data::Data;

use crate::{Actor, act};

pub struct Resize;

impl Actor for Resize {
	type Form = ReflowForm;

	const NAME: &str = "resize";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		act!(app:reflow, cx, form)?;

		for tab in cx.tabs_mut().iter_mut() {
			tab.current.arrow(0);
			tab.parent.as_mut().map(|f| f.arrow(0));
		}
		cx.current_mut().sync_page(true);

		act!(mgr:peek, cx)?;
		succ!(render!())
	}
}
