use anyhow::bail;
use mlua::{ExternalError, FromLua, IntoLua, Lua, Value};
use yazi_core::meter::MeterStats;
use yazi_shared::event::ActionCow;

#[derive(Debug)]
pub struct UpdateMetersForm {
	pub stats: MeterStats,
}

impl TryFrom<ActionCow> for UpdateMetersForm {
	type Error = anyhow::Error;

	fn try_from(mut a: ActionCow) -> Result<Self, Self::Error> {
		let Some(stats) = a.take_any("stats") else {
			bail!("Invalid 'stats' in UpdateMetersForm");
		};

		Ok(Self { stats })
	}
}

impl FromLua for UpdateMetersForm {
	fn from_lua(_: Value, _: &Lua) -> mlua::Result<Self> { Err("unsupported".into_lua_err()) }
}

impl IntoLua for UpdateMetersForm {
	fn into_lua(self, _: &Lua) -> mlua::Result<Value> { Err("unsupported".into_lua_err()) }
}
