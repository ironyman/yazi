use anyhow::anyhow;
use mlua::{ExternalError, FromLua, IntoLua, Lua, Value};
use yazi_core::palette::PaletteFiles;
use yazi_shared::event::ActionCow;

#[derive(Debug)]
pub struct UpdateFilesForm {
	pub files: PaletteFiles,
}

impl TryFrom<ActionCow> for UpdateFilesForm {
	type Error = anyhow::Error;

	fn try_from(mut a: ActionCow) -> Result<Self, Self::Error> {
		Ok(Self {
			files: a.take_any("files").ok_or_else(|| anyhow!("Invalid 'files' in UpdateFilesForm"))?,
		})
	}
}

impl FromLua for UpdateFilesForm {
	fn from_lua(_: Value, _: &Lua) -> mlua::Result<Self> { Err("unsupported".into_lua_err()) }
}

impl IntoLua for UpdateFilesForm {
	fn into_lua(self, _: &Lua) -> mlua::Result<Value> { Err("unsupported".into_lua_err()) }
}
