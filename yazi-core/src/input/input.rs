use std::{ops::{Deref, DerefMut}, sync::Arc};

use parking_lot::Mutex;
use ratatui_widgets::block::Padding;
use unicode_width::UnicodeWidthStr;
use yazi_binding::{elements::Spatial, position::Position};

use crate::input::{InputGuard, InputHistories, InputMutGuard};

#[derive(Default)]
pub struct Input {
	pub main:      InputMain,
	pub alt:       Option<InputAlt>,
	pub histories: InputHistories,
}

impl Input {
	pub(crate) fn focus(&self) -> bool { self.main.visible || self.alt.is_some() }

	/// A single-row input is bare, with the title as an inline prompt;
	/// otherwise it's bordered.
	pub fn padding(&self) -> Padding {
		if self.main.position.height == 1 {
			Padding::left(self.main.title.width() as u16)
		} else {
			Padding::uniform(1)
		}
	}

	pub(crate) fn position(&self) -> Option<Position> {
		if self.main.visible {
			Some(self.main.position)
		} else if let Some(alt) = &self.alt {
			Some(alt.position)
		} else {
			None
		}
	}

	pub fn lock(&self) -> Option<InputGuard<'_>> {
		if self.main.visible {
			Some(InputGuard::Main(&self.main.inner))
		} else if let Some(alt) = &self.alt {
			Some(InputGuard::Alt(alt.inner.lock()))
		} else {
			None
		}
	}

	pub fn lock_mut(&mut self) -> Option<InputMutGuard<'_>> {
		if self.main.visible {
			Some(InputMutGuard::Main(self))
		} else if let Some(alt) = &mut self.alt {
			let guard = alt.inner.lock_arc();
			Some(InputMutGuard::Alt(self, guard))
		} else {
			None
		}
	}
}

// --- InputMain
#[derive(Default)]
pub struct InputMain {
	pub(super) inner: yazi_widgets::input::Input,
	pub name:         String,
	pub title:        String,
	pub position:     Position,
	pub visible:      bool,
}

impl Deref for InputMain {
	type Target = yazi_widgets::input::Input;

	fn deref(&self) -> &Self::Target { &self.inner }
}

impl DerefMut for InputMain {
	fn deref_mut(&mut self) -> &mut Self::Target { &mut self.inner }
}

// --- InputAlt
pub struct InputAlt {
	inner:    Arc<Mutex<yazi_widgets::input::Input>>,
	position: Position,
}

impl Deref for InputAlt {
	type Target = Arc<Mutex<yazi_widgets::input::Input>>;

	fn deref(&self) -> &Self::Target { &self.inner }
}

impl From<&yazi_widgets::input::InputArc> for InputAlt {
	fn from(value: &yazi_widgets::input::InputArc) -> Self {
		Self { inner: value.deref().clone(), position: value.area().into() }
	}
}
