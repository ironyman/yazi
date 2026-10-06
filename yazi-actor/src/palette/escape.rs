use anyhow::Result;
use yazi_config::YAZI;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;
use yazi_widgets::input::InputMode;

use crate::{Actor, Ctx, act};

pub struct Escape;

impl Actor for Escape {
	type Form = VoidForm;

	const NAME: &str = "escape";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		if cx.palette.input.mode() == InputMode::Normal || !YAZI.input.vim_mode.get() {
			return act!(palette:close, cx);
		}

		act!(escape, cx.palette.input)?;
		succ!(render!());
	}
}
