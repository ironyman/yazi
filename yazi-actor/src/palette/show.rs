use anyhow::Result;
use ratatui_core::layout::Margin;
use yazi_config::popup::Palette;
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;
use yazi_widgets::input::Input;

use crate::{Actor, Ctx};

pub struct Show;

impl Actor for Show {
	type Form = VoidForm;

	const NAME: &str = "show";

	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		let position = Palette::position();
		let area = cx.mgr.area(position);
		let input_area = area.inner(Margin::new(1, 1));

		let palette = &mut cx.palette;
		palette.visible = true;
		palette.position = position;
		palette.height = area.height;

		palette.input = Input::default();
		palette.input.repos(input_area);

		palette.keyword.clear();
		palette.offset = 0;
		palette.cursor = 0;
		palette.filter_apply();

		succ!(render!());
	}
}
