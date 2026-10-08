use std::borrow::Cow;

use yazi_shared::{id::Id, url::UrlBuf};
use yazi_shim::SStr;

pub trait TaskIn {
	type Prog;

	fn id(&self) -> Id;

	fn set_id(&mut self, id: Id) -> &mut Self;

	fn title(&self) -> Cow<'_, str>;

	fn set_title(&mut self, _title: impl Into<SStr>) -> &mut Self { self }

	/// The file the task is about, to go to from the task manager.
	fn target(&self) -> Option<UrlBuf> { None }
}
