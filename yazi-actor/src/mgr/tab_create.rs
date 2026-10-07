use anyhow::Result;
use yazi_core::{mgr::CdSource, tab::Tab};
use yazi_macro::{emit, relay, render, succ};
use yazi_parser::mgr::TabCreateForm;
use yazi_scheduler::NotifyProxy;
use yazi_shared::{data::Data, url::UrlLike};

use crate::{Actor, Ctx, act};

const MAX_TABS: usize = 9;

pub struct TabCreate;

impl Actor for TabCreate {
	type Form = TabCreateForm;

	const NAME: &str = "tab_create";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		if cx.tabs().len() >= MAX_TABS {
			succ!(NotifyProxy::push_warn(
				"Too many tabs",
				"You can only open up to 9 tabs at the same time."
			));
		}

		// Seed with the current folders, so they keep their scroll position in the new tab
		let mut tab = Tab::default();
		for folder in [Some(cx.current()), cx.parent()].into_iter().flatten() {
			let mut folder = folder.clone();
			folder.entries.set_filter(None);
			tab.history.insert(folder);
		}

		let (cd, url) = if let Some(target) = form.target {
			(true, target)
		} else if let Some(h) = cx.hovered() {
			tab.pref = cx.tab().pref.clone();
			(false, h.to_url())
		} else if !cx.cwd().is_view() {
			tab.pref = cx.tab().pref.clone();
			(true, cx.cwd().clone())
		} else if let Some(u) = tab.backstack.current().cloned() {
			tab.pref = cx.tab().pref.clone();
			(true, u)
		} else {
			tab.pref = cx.tab().pref.clone();
			(true, tab.cwd().clone())
		};

		let tabs = &mut cx.mgr.tabs;
		tabs.items.insert(tabs.cursor + 1, tab);
		tabs.set_idx(tabs.cursor + 1);

		let cx = &mut Ctx::renew(cx);
		if cd {
			act!(mgr:cd, cx, (url, CdSource::Tab))?;
		} else {
			act!(mgr:reveal, cx, (url, CdSource::Tab))?;
		}

		act!(mgr:refresh, cx)?;
		act!(mgr:peek, cx, true)?;
		act!(app:title, cx).ok();

		// The tab bar may have appeared, which changes the layout
		emit!(Call(relay!(app:resize)));
		succ!(render!());
	}
}
