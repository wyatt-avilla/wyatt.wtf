#![allow(clippy::must_use_candidate)]

use crate::models::{GoodreadsAction, GoodreadsBookUpdate};

pub(super) fn detail_lines(update: &GoodreadsBookUpdate) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(author) = update.author.as_ref() {
        lines.push(format!("by {author}"));
    }

    let action = action_label(update.action);
    lines.push(match update.rating {
        Some(rating) => format!("{action} {rating}/5"),
        None => action.to_string(),
    });

    lines
}

fn action_label(action: GoodreadsAction) -> &'static str {
    match action {
        GoodreadsAction::WantsToRead => "wants to read",
        GoodreadsAction::StartedReading => "started reading",
        GoodreadsAction::FinishedReading => "finished reading",
        GoodreadsAction::Rated => "rated",
        GoodreadsAction::Added => "added",
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn goodreads_update(
        action: GoodreadsAction,
        author: Option<&str>,
        rating: Option<u8>,
    ) -> GoodreadsBookUpdate {
        GoodreadsBookUpdate {
            id: "book".to_string(),
            action,
            title: "Book".to_string(),
            author: author.map(str::to_string),
            rating,
            cover_url: None,
            book_url: None,
            author_url: None,
            review_url: "https://goodreads.example/review".to_string(),
            published_at: Utc.with_ymd_and_hms(2026, 5, 2, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn puts_author_and_action_on_separate_lines() {
        assert_eq!(
            detail_lines(&goodreads_update(
                GoodreadsAction::StartedReading,
                Some("Brian Christian"),
                None,
            )),
            vec!["by Brian Christian", "started reading"]
        );
        assert_eq!(
            detail_lines(&goodreads_update(
                GoodreadsAction::Rated,
                Some("Amanda Askell"),
                Some(5),
            )),
            vec!["by Amanda Askell", "rated 5/5"]
        );
        assert_eq!(
            detail_lines(&goodreads_update(
                GoodreadsAction::WantsToRead,
                Some("Nag Hammadi"),
                None,
            )),
            vec!["by Nag Hammadi", "wants to read"]
        );
    }

    #[test]
    fn omits_missing_values() {
        assert_eq!(
            detail_lines(&goodreads_update(GoodreadsAction::Rated, None, None)),
            vec!["rated"]
        );
        assert_eq!(
            detail_lines(&goodreads_update(
                GoodreadsAction::Rated,
                Some("Amanda Askell"),
                None,
            )),
            vec!["by Amanda Askell", "rated"]
        );
    }
}
