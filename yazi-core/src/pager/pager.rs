use tokio::task::AbortHandle;
use yazi_binding::position::Position;
use yazi_shared::id::Id;
use yazi_widgets::Step;

#[derive(Default)]
pub struct Pager {
	pub visible:  bool,
	pub title:    String,
	pub lines:    Vec<String>,
	pub position: Position,
	pub height:   u16,
	pub offset:   usize,
	pub ticket:   Id,
	pub handle:   Option<AbortHandle>,
}

impl Pager {
	pub const BORDER: u16 = 2;

	pub fn close(&mut self) {
		if let Some(handle) = self.handle.take() {
			handle.abort();
		}
		self.visible = false;
		self.lines = vec![];
	}

	pub fn push(&mut self, ticket: Id, line: String) -> bool {
		if ticket != self.ticket || !self.visible {
			return false;
		}
		self.lines.push(line);
		true
	}

	pub fn scroll(&mut self, step: Step) -> bool {
		let limit = self.limit();
		let max = self.lines.len().saturating_sub(limit);

		let old = self.offset;
		self.offset = step.add(old, max + 1, limit, 0, 0);
		self.offset != old
	}

	pub fn limit(&self) -> usize { self.height.saturating_sub(Self::BORDER) as usize }
}
