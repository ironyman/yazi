use ratatui_core::{buffer::Buffer, layout::{self, Alignment, Constraint, Rect}, text::{Line, Span}, widgets::Widget};
use ratatui_widgets::{block::Block, borders::BorderType};
use yazi_config::{KEYMAP, THEME};
use yazi_core::{Core, tasks::TASKS_PERCENT};
use yazi_scheduler::TaskFilter;
use yazi_shared::{Layer, event::Action};

use crate::tasks::List;

pub(crate) struct Tasks<'a> {
	core: &'a mut Core,
}

impl<'a> Tasks<'a> {
	pub(crate) fn new(core: &'a mut Core) -> Self { Self { core } }

	pub(super) fn area(area: Rect) -> Rect {
		let chunk = layout::Layout::vertical([
			Constraint::Percentage((100 - TASKS_PERCENT) / 2),
			Constraint::Percentage(TASKS_PERCENT),
			Constraint::Percentage((100 - TASKS_PERCENT) / 2),
		])
		.split(area)[1];

		layout::Layout::horizontal([
			Constraint::Percentage((100 - TASKS_PERCENT) / 2),
			Constraint::Percentage(TASKS_PERCENT),
			Constraint::Percentage((100 - TASKS_PERCENT) / 2),
		])
		.split(chunk)[1]
	}
}

impl Tasks<'_> {
	/// The lists of tasks, with their counts and the shown one highlighted.
	fn tabs(&self) -> Line<'static> {
		let tasks = &self.core.tasks;
		let mut spans = vec![Span::styled(" Tasks ", THEME.tasks.title.get()), Span::raw("─")];
		for (f, n) in TaskFilter::ALL.into_iter().zip(tasks.counts) {
			let style =
				if f == tasks.filter { THEME.tasks.hovered.get() } else { THEME.tasks.border.get() };
			spans.push(Span::styled(format!(" {} ({n}) ", f.label()), style));
		}
		Line::from(spans)
	}

	fn hints(filter: TaskFilter) -> String {
		let finished = filter.is_finished();
		let hints = [
			Self::key(|a| a.name == "arrow" && a.first::<&str>().is_ok_and(|s| s == "prev"))
				.zip(Self::key(|a| a.name == "arrow" && a.first::<&str>().is_ok_and(|s| s == "next")))
				.map(|(p, n)| format!("{p}/{n} select")),
			Self::key(|a| a.name == "inspect").map(|k| format!("{k} log")),
			Self::key(|a| a.name == "reveal").map(|k| format!("{k} go to file")),
			Self::key(|a| a.name == "cancel")
				.map(|k| format!("{k} {}", if finished { "remove" } else { "cancel" })),
			Self::key(|a| a.name == "filter" && a.first::<&str>().is_ok_and(|s| s == "next"))
				.map(|k| format!("{k} next list")),
			Self::key(|a| a.name == "close").map(|k| format!("{k} close")),
		];
		format!(" {} ", hints.into_iter().flatten().collect::<Vec<_>>().join("  "))
	}

	/// The key bound to the action, preferring special keys like `<Enter>` over characters.
	fn key(f: impl Fn(&Action) -> bool) -> Option<String> {
		let chords = KEYMAP.chords(Layer::Tasks);
		let mut it = chords.iter().filter(|c| matches!(&c.run[..], [a] if f(a))).map(|c| c.on());

		let first = it.next()?;
		let key =
			if first.starts_with('<') { first } else { it.find(|k| k.starts_with('<')).unwrap_or(first) };

		Some(match key.as_str() {
			"<Enter>" => "↵".to_owned(),
			"<Tab>" => "⇥".to_owned(),
			"<Up>" => "↑".to_owned(),
			"<Down>" => "↓".to_owned(),
			"<Esc>" => "esc".to_owned(),
			_ => key,
		})
	}
}

impl Widget for Tasks<'_> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		let area = Self::area(area);

		yazi_widgets::clear::Clear::default().render(area, buf);

		let block = Block::bordered()
			.title(self.tabs())
			.title_alignment(Alignment::Center)
			.title_bottom(Line::styled(Self::hints(self.core.tasks.filter), THEME.tasks.border.get()))
			.border_type(BorderType::Rounded)
			.border_style(THEME.tasks.border.get());
		(&block).render(area, buf);

		List::new(self.core).render(block.inner(area), buf);
	}
}
