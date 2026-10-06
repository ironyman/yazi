use anyhow::Result;
use yazi_config::YAZI;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;
use yazi_widgets::input::InputOp;

use crate::{Actor, Ctx, act};

pub struct Escape;

impl Actor for Escape {
	type Form = VoidForm;

	const NAME: &str = "escape";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		use yazi_widgets::input::InputMode as M;
		let Some(mut guard) = cx.input.lock_mut() else {
			succ!();
		};

		let (mode, op) = (guard.snap().mode, guard.snap().op);
		let vim = YAZI.input.vim_mode.get();
		if vim || mode != M::Insert {
			act!(escape, guard)?;
		}

		drop(guard);
		match mode {
			M::Normal if op == InputOp::None => act!(input:close, cx),
			M::Insert if vim || cx.cmp.visible => act!(cmp:close, cx),
			M::Insert => act!(input:close, cx),
			M::Normal | M::Replace => Ok(().into()),
		}?;

		succ!(render!());
	}
}
