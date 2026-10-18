use anyhow::Result;
use yazi_core::mgr::CdSource;
use yazi_macro::succ;
use yazi_parser::ArrowForm;
use yazi_shared::{data::Data, url::UrlLike};

use crate::{Actor, Ctx, act};

pub struct Sibling;

impl Actor for Sibling {
	type Form = ArrowForm;

	const NAME: &str = "sibling";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		let Some(parent) = cx.parent() else { succ!() };

		let dirs: Vec<_> = parent.entries.iter().enumerate().filter(|(_, f)| f.is_dir()).collect();
		let Some(pos) = dirs.iter().position(|&(i, _)| i == parent.cursor) else { succ!() };

		let (_, dir) = dirs[form.step.add(pos, dirs.len(), 0, 0, 0)];
		let url = dir.physical().to_owned();

		act!(mgr:cd, cx, (url, CdSource::Enter))
	}
}
