use std::path::is_separator;

/// Scores `haystack` against `needle` as a case-insensitive subsequence, favoring
/// consecutive matches, matches at word boundaries, and matches in the last path component.
pub(super) fn fuzzy(haystack: &str, needle: &str) -> Option<usize> {
	let base = haystack.rfind(is_separator).map_or(0, |i| i + 1);
	let mut needle = needle.chars().flat_map(char::to_lowercase).peekable();

	let (mut score, mut prev, mut last) = (0, None, '/');
	for (i, c) in haystack.char_indices() {
		let Some(&n) = needle.peek() else { break };
		if c.to_lowercase().eq([n]) {
			needle.next();
			score += 1;
			score += if prev == Some(i) { 4 } else { 0 };
			score += if is_separator(last) || matches!(last, '_' | '-' | '.' | ' ') { 3 } else { 0 };
			score += if i >= base { 2 } else { 0 };
			prev = Some(i + c.len_utf8());
		}
		last = c;
	}

	needle.peek().is_none().then_some(score)
}
