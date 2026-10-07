use anyhow::Result;
use yazi_core::{mgr::OpenOpt, tab::Finder};
use yazi_macro::{render, succ};
use yazi_parser::mgr::FindDoForm;
use yazi_shared::data::Data;

use crate::{Actor, Ctx, act};

pub struct FindDo;

impl Actor for FindDo {
	type Form = FindDoForm;

	const NAME: &str = "find_do";

	fn act(cx: &mut Ctx, Self::Form { opt }: Self::Form) -> Result<Data> {
		if opt.query.is_empty() {
			return act!(mgr:escape_find, cx);
		}

		let finder = Finder::new(&opt.query, opt.case)?;
		if !matches!(&cx.tab().finder, Some(f) if f.filter == finder.filter) {
			let step = if opt.prev {
				finder.prev(&cx.current().entries, cx.current().cursor, true)
			} else {
				finder.next(&cx.current().entries, cx.current().cursor, true)
			};

			if let Some(step) = step {
				act!(mgr:arrow, cx, step)?;
			}

			cx.tab_mut().finder = Some(finder);
			render!();
		}

		let unique =
			opt.auto && cx.tab().finder.as_ref().is_some_and(|f| f.unique(&cx.current().entries));
		if unique {
			act!(input:close, cx)?;
		} else if !opt.enter {
			succ!();
		}

		act!(mgr:escape_find, cx)?;
		if cx.hovered().is_some_and(|h| h.is_dir()) {
			act!(mgr:enter, cx)
		} else {
			act!(mgr:open, cx, OpenOpt { hovered: true, ..Default::default() })
		}
	}
}
