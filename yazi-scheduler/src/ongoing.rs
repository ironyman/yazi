use std::collections::VecDeque;

use hashbrown::{HashMap, hash_map::Entry};
use yazi_config::YAZI;
use yazi_shared::id::{Id, Ids};

use super::{Progress, Task, TaskFilter};
use crate::{TaskHandle, TaskIn, TaskProg, hook::HookIn};

/// How many finished tasks are kept for the task manager.
const HISTORY: usize = 200;

#[derive(Default)]
pub struct Ongoing {
	inner:    HashMap<Id, Task>,
	/// Finished user tasks, oldest first, with how they ended.
	finished: VecDeque<(Task, TaskFilter)>,
}

impl Ongoing {
	pub(super) fn add<T>(&mut self, r#in: &mut T) -> &mut Task
	where
		T: TaskIn,
		T::Prog: Into<TaskProg> + Default,
	{
		static IDS: Ids = Ids::new();
		let id = IDS.next();

		let title = r#in.set_id(id).title().into_owned();
		let prog = T::Prog::default().into();

		let mut task = Task::new(id, title, prog);
		task.target = r#in.target();
		self.inner.entry(id).insert(task).into_mut()
	}

	pub(super) fn cancel(&mut self, id: Id) -> Option<HookIn> {
		match self.inner.entry(id) {
			Entry::Occupied(mut oe) => {
				let task = oe.get_mut();
				task.cancel();

				if let Some(hook) = task.hook.take() {
					return Some(hook);
				}

				let task = oe.remove();
				let outcome = if task.prog.failed() { TaskFilter::Failed } else { TaskFilter::Canceled };
				self.finish(task, outcome);
			}
			Entry::Vacant(_) => {}
		}
		None
	}

	pub(super) fn fulfill(&mut self, id: Id) {
		let Some(task) = self.inner.remove(&id) else { return };
		task.succeed();

		// Canceled tasks with a cleanup hook end here too, once it ran
		let outcome = if task.is_canceled() { TaskFilter::Canceled } else { TaskFilter::Completed };
		self.finish(task, outcome);
	}

	fn finish(&mut self, task: Task, outcome: TaskFilter) {
		if !task.prog.is_user() {
			return;
		}
		if self.finished.len() >= HISTORY {
			self.finished.pop_front();
		}
		self.finished.push_back((task, outcome));
	}

	/// Removes a finished task from the history, returning whether it was there.
	pub fn dismiss(&mut self, id: Id) -> bool {
		let len = self.finished.len();
		self.finished.retain(|(t, _)| t.id != id);
		len != self.finished.len()
	}

	#[inline]
	pub fn get_mut(&mut self, id: Id) -> Option<&mut Task> {
		match self.inner.get_mut(&id) {
			Some(task) => Some(task),
			None => self.finished.iter_mut().find(|(t, _)| t.id == id).map(|(t, _)| t),
		}
	}

	pub fn get_id(&self, filter: TaskFilter, idx: usize) -> Option<Id> {
		self.view(filter).nth(idx).map(|t| t.id)
	}

	/// Tasks matching the filter: running tasks unordered, finished ones newest first.
	pub fn view(&self, filter: TaskFilter) -> Box<dyn Iterator<Item = &Task> + '_> {
		let history = move |f: TaskFilter| {
			self.finished.iter().rev().filter(move |&&(_, o)| o == f).map(|(t, _)| t)
		};

		match filter {
			TaskFilter::Running => Box::new(self.values().filter(|t| !t.prog.failed())),
			TaskFilter::Failed => {
				Box::new(self.values().filter(|t| t.prog.failed()).chain(history(TaskFilter::Failed)))
			}
			f => Box::new(history(f)),
		}
	}

	#[inline]
	pub(crate) fn get_handle(&self, id: Id) -> Option<TaskHandle> {
		self.inner.get(&id).map(|t| t.handle.clone())
	}

	pub fn len(&self) -> usize {
		if YAZI.tasks.suppress_preload {
			self.inner.values().filter(|&t| t.prog.is_user()).count()
		} else {
			self.inner.len()
		}
	}

	#[inline]
	pub fn exists(&self, id: Id) -> bool { self.inner.contains_key(&id) }

	#[inline]
	pub(crate) fn intact(&self, id: Id) -> bool {
		self.inner.get(&id).is_some_and(|t| !t.is_canceled())
	}

	pub fn values(&self) -> Box<dyn Iterator<Item = &Task> + '_> {
		if YAZI.tasks.suppress_preload {
			Box::new(self.inner.values().filter(|&t| t.prog.is_user()))
		} else {
			Box::new(self.inner.values())
		}
	}

	#[inline]
	pub fn is_empty(&self) -> bool { self.len() == 0 }
}
