use serde::Deserialize;

/// Which tasks the task manager lists.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskFilter {
	#[default]
	Running,
	Completed,
	Failed,
	Canceled,
}

impl TaskFilter {
	pub const ALL: [Self; 4] = [Self::Running, Self::Completed, Self::Failed, Self::Canceled];

	pub fn label(self) -> &'static str {
		match self {
			Self::Running => "In progress",
			Self::Completed => "Completed",
			Self::Failed => "Failed",
			Self::Canceled => "Canceled",
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Running => "running",
			Self::Completed => "completed",
			Self::Failed => "failed",
			Self::Canceled => "canceled",
		}
	}

	/// Whether the listed tasks are finished, so only kept as history.
	pub fn is_finished(self) -> bool { matches!(self, Self::Completed | Self::Canceled) }

	/// The filter `step` places away, wrapping around.
	pub fn step(self, step: isize) -> Self {
		let i = Self::ALL.iter().position(|&f| f == self).unwrap_or(0) as isize;
		Self::ALL[(i + step).rem_euclid(Self::ALL.len() as isize) as usize]
	}
}
