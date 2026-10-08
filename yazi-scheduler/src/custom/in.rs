use std::borrow::Cow;

use mlua::{FromLua, Lua, Table, Value};
use yazi_binding::Scope;
use yazi_shared::{id::Id, url::UrlBuf};
use yazi_shim::SStr;

use crate::{TaskIn, custom::{CustomPool, CustomProg}};

#[derive(Clone, Debug, Default)]
pub struct CustomIn {
	id:                  Id,
	pub(crate) pool:     CustomPool,
	pub(crate) scope:    Scope,
	title:               SStr,
	pub(crate) track:    bool,
	pub(crate) progress: bool,
	target:              Option<UrlBuf>,
}

impl CustomIn {
	pub fn new(title: impl Into<SStr>, progress: bool) -> Self {
		Self { scope: Scope::new(), title: title.into(), progress, ..Default::default() }
	}

	#[inline]
	pub fn scope(&self) -> &Scope { &self.scope }

	pub fn with_target(mut self, target: impl Into<UrlBuf>) -> Self {
		self.target = Some(target.into());
		self
	}
}

impl TaskIn for CustomIn {
	type Prog = CustomProg;

	fn target(&self) -> Option<UrlBuf> { self.target.clone() }

	fn id(&self) -> Id { self.id }

	fn set_id(&mut self, id: Id) -> &mut Self {
		self.id = id;
		self
	}

	fn title(&self) -> Cow<'_, str> {
		if self.title.is_empty() { "Run custom task".into() } else { Cow::Borrowed(&self.title) }
	}

	fn set_title(&mut self, title: impl Into<SStr>) -> &mut Self {
		self.title = title.into();
		self
	}
}

impl FromLua for CustomIn {
	fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
		let t = Table::from_lua(value, lua)?;

		Ok(Self {
			pool: t.raw_get("pool")?,
			scope: t.raw_get("scope")?,
			track: t.raw_get("track")?,
			progress: t.raw_get("progress")?,
			..Default::default()
		})
	}
}
