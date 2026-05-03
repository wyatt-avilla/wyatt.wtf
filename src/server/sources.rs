pub(crate) mod goodreads;
pub(crate) mod lastfm;
pub(crate) mod letterboxd;

use chrono::{DateTime, Utc};
use roxmltree::Node;

use super::error::{BackendError, Result};

pub use goodreads::fetch_goodreads;
pub use lastfm::fetch_lastfm;
pub use letterboxd::fetch_letterboxd;

fn child_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .and_then(|child| child.text())
        .map(clean_text)
        .filter(|text| !text.is_empty())
}

fn parse_rfc2822_child(node: Node<'_, '_>, name: &'static str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc2822(
        &child_text(node, name).ok_or(BackendError::MissingField(name))?,
    )?
    .with_timezone(&Utc))
}

fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}
