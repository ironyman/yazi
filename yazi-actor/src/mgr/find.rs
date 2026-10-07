use std::time::Duration;

use anyhow::Result;
use tokio::pin;
use tokio_stream::{StreamExt, wrappers::UnboundedReceiverStream};
use yazi_config::YAZI;
use yazi_core::mgr::FindDoOpt;
use yazi_macro::succ;
use yazi_parser::mgr::FindForm;
use yazi_proxy::MgrProxy;
use yazi_shared::{Debounce, data::Data};
use yazi_widgets::input::InputEvent;

use crate::{Actor, Ctx, input};

pub struct Find;

impl Actor for Find {
	type Form = FindForm;

	const NAME: &str = "find";

	fn act(cx: &mut Ctx, form: Self::Form) -> Result<Data> {
		let input = input!(cx, YAZI.input.find(form.prev, form.enter))?;

		tokio::spawn(async move {
			let rx = Debounce::new(UnboundedReceiverStream::new(input), Duration::from_millis(50));
			pin!(rx);

			while let Some(event) = rx.next().await {
				let enter = form.enter && event.is_submit();
				let (InputEvent::Submit(s) | InputEvent::Type(s)) = event else { break };

				MgrProxy::find_do(FindDoOpt {
					query: s.into(),
					prev: form.prev,
					case: form.case,
					enter,
					auto: form.auto,
				});
			}
		});
		succ!();
	}
}
