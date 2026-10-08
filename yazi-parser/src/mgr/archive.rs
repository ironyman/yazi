use mlua::{FromLua, IntoLua, Lua, LuaSerdeExt, Value};
use serde::{Deserialize, Serialize};
use yazi_shared::event::ActionCow;
use yazi_shim::mlua::SER_OPT;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ArchiveForm {
	#[serde(alias = "0")]
	pub op: ArchiveOp,
}

impl TryFrom<ActionCow> for ArchiveForm {
	type Error = anyhow::Error;

	fn try_from(a: ActionCow) -> Result<Self, Self::Error> { Ok(a.deserialize()?) }
}

impl FromLua for ArchiveForm {
	fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> { lua.from_value(value) }
}

impl IntoLua for ArchiveForm {
	fn into_lua(self, lua: &Lua) -> mlua::Result<Value> { lua.to_value_with(&self, SER_OPT) }
}

// --- Op
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchiveOp {
	#[default]
	Status,
	Commit,
	Discard,
	Create,
}
