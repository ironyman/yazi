use mlua::{IntoLua, Lua, Value};

/// A piece of the system meters as drawn in the header, with the load it shows
/// (to color it by) if it's a value rather than a label or a space.
#[derive(Clone, Debug, PartialEq)]
pub struct MeterPart {
	pub text:    String,
	pub percent: Option<f64>,
}

impl MeterPart {
	pub(super) fn label(text: impl Into<String>) -> Self { Self { text: text.into(), percent: None } }

	pub(super) fn value(text: impl Into<String>, percent: f64) -> Self {
		Self { text: text.into(), percent: Some(percent) }
	}
}

impl IntoLua for MeterPart {
	fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
		let table = lua.create_table()?;
		table.raw_set("text", self.text)?;
		table.raw_set("percent", self.percent)?;
		table.into_lua(lua)
	}
}
