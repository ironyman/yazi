use std::sync::LazyLock;

use yazi_shared::url::{AsUrl, Url, UrlBuf};

static URL: LazyLock<UrlBuf> =
	LazyLock::new(|| "pc://this/".parse().expect("This PC URL is valid"));

/// The Windows page above every drive root, listing drives and cloud storage folders in the order
/// its `pc` provider gives, rather than any tab's sort.
pub struct ThisPc;

impl ThisPc {
	pub fn url() -> &'static UrlBuf { &URL }

	/// The parent of `url`, which is This PC for a drive root on Windows.
	pub fn parent(url: &impl AsUrl) -> Option<Url<'_>> {
		let url = url.as_url();
		url
			.parent()
			.or_else(|| (cfg!(windows) && url.is_regular() && url.is_absolute()).then(|| URL.as_url()))
	}
}
