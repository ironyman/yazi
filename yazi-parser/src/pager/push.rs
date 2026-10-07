use mlua::{ExternalError, FromLua, IntoLua, Lua, Value};
use yazi_shared::{event::ActionCow, id::Id};

#[derive(Debug)]
pub struct PushForm {
	pub ticket: Id,
	pub line:   String,
}

impl TryFrom<ActionCow> for PushForm {
	type Error = anyhow::Error;

	fn try_from(mut a: ActionCow) -> Result<Self, Self::Error> {
		Ok(Self { ticket: a.get("ticket")?, line: a.take_first()? })
	}
}

impl FromLua for PushForm {
	fn from_lua(_: Value, _: &Lua) -> mlua::Result<Self> { Err("unsupported".into_lua_err()) }
}

impl IntoLua for PushForm {
	fn into_lua(self, _: &Lua) -> mlua::Result<Value> { Err("unsupported".into_lua_err()) }
}
