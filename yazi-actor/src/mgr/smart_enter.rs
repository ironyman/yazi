use anyhow::Result;
use yazi_core::mgr::OpenOpt;
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct SmartEnter;

impl Actor for SmartEnter {
	type Form = VoidForm;

	const NAME: &str = "smart_enter";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		if cx.hovered().is_some_and(|h| h.is_dir()) {
			act!(mgr:enter, cx)
		} else {
			act!(mgr:open, cx, OpenOpt { hovered: true, ..Default::default() })
		}
	}
}
