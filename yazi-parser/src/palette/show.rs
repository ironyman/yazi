use mlua::{ExternalError, FromLua, IntoLua, Lua, Value};
use serde::Deserialize;
use yazi_core::palette::PaletteMode;
use yazi_shared::event::ActionCow;

#[derive(Debug, Default, Deserialize)]
pub struct ShowForm {
	#[serde(default, alias = "0")]
	pub mode: PaletteMode,
}

impl TryFrom<ActionCow> for ShowForm {
	type Error = anyhow::Error;

	fn try_from(a: ActionCow) -> Result<Self, Self::Error> { Ok(a.deserialize()?) }
}

impl FromLua for ShowForm {
	fn from_lua(_: Value, _: &Lua) -> mlua::Result<Self> { Err("unsupported".into_lua_err()) }
}

impl IntoLua for ShowForm {
	fn into_lua(self, _: &Lua) -> mlua::Result<Value> { Err("unsupported".into_lua_err()) }
}
