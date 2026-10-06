use yazi_binding::position::{Offset, Origin, Position};
use yazi_widgets::input::InputOpt;

use crate::popup::{Pick, PickCfg};

pub struct Palette;

impl Palette {
	pub fn position() -> Position { Self::position_with(20) }

	pub fn input(title: impl Into<String>, value: impl Into<String>) -> InputOpt {
		InputOpt {
			name: "palette".to_owned(),
			title: title.into(),
			value: value.into(),
			history: "shared".to_owned(),
			position: Self::position_with(3),
			..Default::default()
		}
	}

	pub fn pick(title: impl Into<String>, items: Vec<String>) -> PickCfg {
		PickCfg {
			title: title.into(),
			position: Self::position_with(20.min(Pick::BORDER.saturating_add(items.len() as u16))),
			items,
		}
	}

	fn position_with(height: u16) -> Position {
		Position::new(Origin::TopCenter, Offset { x: 0, y: 2, width: 80, height })
	}
}
