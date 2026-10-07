use yazi_binding::position::{Offset, Origin, Position};

pub struct Pager;

impl Pager {
	pub fn position() -> Position {
		Position::new(Origin::Center, Offset { x: 0, y: 0, width: 120, height: 40 })
	}
}
