use std::path::PathBuf;

use anyhow::Result;
use ratatui_core::layout::Margin;
use yazi_config::popup::Palette;
use yazi_macro::{render, succ};
use yazi_parser::palette::ShowForm;
use yazi_shared::{data::Data, url::UrlLike};
use yazi_widgets::input::Input;

use crate::{Actor, Ctx};

pub struct Show;

impl Actor for Show {
	type Form = ShowForm;

	const NAME: &str = "show";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		let position = Palette::position();
		let area = cx.mgr.area(position);
		let input_area = area.inner(Margin::new(1, 1));

		let cwd = cx.cwd().as_local().map(PathBuf::from);
		let history = cx.input.histories.get("shell").to_vec();

		let palette = &mut cx.palette;
		palette.close();
		palette.visible = true;
		palette.mode = form.mode;
		palette.position = position;
		palette.height = area.height;
		palette.history = history;
		palette.cwd = cwd;

		palette.input = Input::default();
		palette.input.repos(input_area);

		palette.keyword.clear();
		palette.offset = 0;
		palette.cursor = 0;
		palette.filter_apply();

		succ!(render!());
	}
}
