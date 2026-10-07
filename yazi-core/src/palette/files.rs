use yazi_macro::impl_data_any;
use yazi_shared::id::Id;

use super::Entry;

#[derive(Clone, Debug)]
pub struct PaletteFiles {
	pub ticket:  Id,
	pub entries: Vec<Entry>,
	pub done:    bool,
}

impl_data_any!(PaletteFiles);
