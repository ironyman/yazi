use std::path::{Path, PathBuf};

/// The fields of the form for creating an archive, filled in the palette.
#[derive(Clone, Debug, Default)]
pub struct Draft {
	pub cwd:      PathBuf,
	pub marked:   Vec<PathBuf>,
	pub yanked:   Vec<PathBuf>,
	pub source:   Source,
	pub dir:      String,
	pub kinds:    Vec<&'static str>,
	pub kind:     usize,
	/// In megabytes, or with a unit like `500K` or `4.7G`; empty for one file.
	pub volume:   String,
	pub password: String,
	/// Empty for the default name.
	pub name:     String,
}

/// What goes into the archive.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Source {
	Marked,
	Yanked,
	#[default]
	Dir,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
	Source,
	Dir,
	Kind,
	Volume,
	Password,
	Name,
	Create,
}

impl Field {
	pub fn label(self) -> &'static str {
		match self {
			Self::Source => "Archive what",
			Self::Dir => "Directory",
			Self::Kind => "Type",
			Self::Volume => "Max volume size (MB)",
			Self::Password => "Password",
			Self::Name => "Name",
			Self::Create => "Create archive",
		}
	}

	pub fn is_text(self) -> bool {
		matches!(self, Self::Dir | Self::Volume | Self::Password | Self::Name)
	}

	pub fn is_choice(self) -> bool { matches!(self, Self::Source | Self::Kind) }
}

impl Draft {
	/// The fields that apply to the current choices, in order.
	pub fn fields(&self) -> Vec<Field> {
		let kind = self.kind();
		[
			(self.sources().len() > 1).then_some(Field::Source),
			(self.source == Source::Dir).then_some(Field::Dir),
			Some(Field::Kind),
			(kind == "7z").then_some(Field::Volume),
			matches!(kind, "zip" | "7z").then_some(Field::Password),
			Some(Field::Name),
			Some(Field::Create),
		]
		.into_iter()
		.flatten()
		.collect()
	}

	/// The sources there is something to archive from.
	pub fn sources(&self) -> Vec<Source> {
		[
			(!self.marked.is_empty()).then_some(Source::Marked),
			(!self.yanked.is_empty()).then_some(Source::Yanked),
			Some(Source::Dir),
		]
		.into_iter()
		.flatten()
		.collect()
	}

	/// The files to archive, if the source is a list of them.
	pub fn files(&self) -> Option<&[PathBuf]> {
		match self.source {
			Source::Marked => Some(&self.marked),
			Source::Yanked => Some(&self.yanked),
			Source::Dir => None,
		}
	}

	pub fn kind(&self) -> &'static str { self.kinds.get(self.kind).copied().unwrap_or_default() }

	/// The archive name used when none is typed: the input's name with the type's extension.
	pub fn default_name(&self) -> String {
		let stem = match self.files() {
			Some([one]) => one.file_stem(),
			Some(_) => self.cwd.file_name(),
			None => Path::new(self.dir.trim()).file_name(),
		};
		format!("{}.{}", stem.unwrap_or_default().to_string_lossy(), self.kind())
	}

	/// The editable text of a field.
	pub fn text(&self, field: Field) -> Option<&str> {
		Some(match field {
			Field::Dir => &self.dir,
			Field::Volume => &self.volume,
			Field::Password => &self.password,
			Field::Name => &self.name,
			_ => return None,
		})
	}

	pub fn set_text(&mut self, field: Field, value: &str) {
		let slot = match field {
			Field::Dir => &mut self.dir,
			Field::Volume => &mut self.volume,
			Field::Password => &mut self.password,
			Field::Name => &mut self.name,
			_ => return,
		};
		value.clone_into(slot);
	}

	/// How the value of a field is shown next to it.
	pub fn display(&self, field: Field) -> String {
		match field {
			Field::Source => match self.source {
				Source::Marked => format!("‹ Marked files ({}) ›", self.marked.len()),
				Source::Yanked => format!("‹ Yanked files ({}) ›", self.yanked.len()),
				Source::Dir => "‹ A directory ›".to_owned(),
			},
			Field::Kind => format!("‹ {} ›", self.kind()),
			Field::Volume => match self.volume.trim() {
				"" => "one file".to_owned(),
				v if v.chars().all(|c| c.is_ascii_digit() || c == '.') => format!("{v} MB"),
				v => v.to_owned(),
			},
			Field::Password if self.password.is_empty() => "none".to_owned(),
			Field::Password => "•".repeat(self.password.chars().count().min(12)),
			Field::Name if self.name.trim().is_empty() => self.default_name(),
			Field::Create => "↵".to_owned(),
			f => self.text(f).unwrap_or_default().to_owned(),
		}
	}

	/// Moves a choice field by `step`, wrapping around.
	pub fn cycle(&mut self, field: Field, step: isize) {
		match field {
			Field::Source => {
				let sources = self.sources();
				let i = sources.iter().position(|&s| s == self.source).unwrap_or(0) as isize;
				self.source = sources[(i + step).rem_euclid(sources.len() as isize) as usize];
			}
			Field::Kind if !self.kinds.is_empty() => {
				let len = self.kinds.len() as isize;
				self.kind = (self.kind as isize + step).rem_euclid(len) as usize;
			}
			_ => {}
		}
	}
}
