use anyhow::Result;
use yazi_core::palette::{Entry, PaletteMode};
use yazi_macro::{render, succ};
use yazi_parser::VoidForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx};

pub struct Unlist;

impl Actor for Unlist {
	type Form = VoidForm;

	const NAME: &str = "unlist";

	/// Unyanks or unmarks the hovered file, depending on the list shown.
	fn act(cx: &mut Ctx, _: Self::Form) -> Result<Data> {
		let Some(Entry::Listed(url)) = cx.palette.hovered().cloned() else { succ!() };

		match cx.palette.mode {
			PaletteMode::Yanked => {
				cx.mgr.yanked.remove_many([&url]);
				render!(cx.mgr.yanked.catchup_revision(false));
			}
			PaletteMode::Marked => _ = cx.tab_mut().selected.remove(&url),
			_ => succ!(),
		}

		let palette = &mut cx.palette;
		palette.listed.retain(|u| *u != url);
		palette.filter_apply();
		palette.cursor = palette.cursor.min(palette.entries.len().saturating_sub(1));
		succ!(render!());
	}
}
