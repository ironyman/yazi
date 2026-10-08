use std::{sync::Arc, time::Duration};

use tokio::{task::JoinHandle, time::sleep};
use yazi_scheduler::{Scheduler, TaskFilter, TaskSnap, TaskSummary};
use yazi_term::TERM;

use super::{TASKS_BORDER, TASKS_PADDING, TASKS_PERCENT};
use crate::AppProxy;

pub struct Tasks {
	pub scheduler: Arc<Scheduler>,
	handle:        JoinHandle<()>,

	pub visible: bool,
	pub cursor:  usize,
	pub filter:  TaskFilter,
	pub snaps:   Vec<TaskSnap>,
	/// How many tasks each filter lists, in the order of `TaskFilter::ALL`.
	pub counts:  [usize; 4],
	pub summary: TaskSummary,
}

impl Tasks {
	pub(crate) fn serve() -> Self {
		let scheduler = Scheduler::serve();
		let ongoing = scheduler.ongoing.clone();

		let handle = tokio::spawn(async move {
			let mut last = TaskSummary::default();
			loop {
				sleep(Duration::from_millis(500)).await;

				let new = TaskSummary::from(&*ongoing.lock());
				if last != new {
					last = new;
					AppProxy::update_progress(new);
				}
			}
		});

		Self {
			scheduler: Arc::new(scheduler),
			handle,

			visible: false,
			cursor: 0,
			filter: Default::default(),
			snaps: Default::default(),
			counts: Default::default(),
			summary: Default::default(),
		}
	}

	pub fn shutdown(&self) {
		self.scheduler.shutdown();
		self.handle.abort();
	}

	pub fn limit() -> usize {
		((TERM.dimension().rows * TASKS_PERCENT / 100).saturating_sub(TASKS_BORDER + TASKS_PADDING)
			as usize)
			/ 3
	}

	/// Re-reads the listed tasks and the counts per filter, returning whether they changed.
	pub fn refresh(&mut self) -> bool {
		let ongoing = self.scheduler.ongoing.lock();
		let snaps: Vec<_> = ongoing.view(self.filter).take(Self::limit()).map(Into::into).collect();
		let counts = TaskFilter::ALL.map(|f| ongoing.view(f).count());
		drop(ongoing);

		let changed = self.snaps != snaps || self.counts != counts;
		(self.snaps, self.counts) = (snaps, counts);
		changed
	}
}
