#![allow(clippy::must_use_candidate)]

use crate::models::LetterboxdWatch;

pub(super) fn detail_lines(watch: &LetterboxdWatch) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(rating) = watch
        .rating_stars
        .as_deref()
        .and_then(normalized_rating_stars)
    {
        lines.push(format!("rated {rating}/5"));
    }
    lines.push(action_line(watch).to_string());

    lines
}

fn action_line(watch: &LetterboxdWatch) -> &'static str {
    if watch.rewatch {
        "rewatched"
    } else if watch.liked {
        "liked"
    } else {
        "watched"
    }
}

fn normalized_rating_stars(rating: &str) -> Option<String> {
    let whole = rating.chars().filter(|&ch| ch == '★').count();
    let half = rating.chars().any(|ch| ch == '½');

    if whole == 0 && !half {
        return None;
    }

    Some(if half {
        format!("{whole}.5")
    } else {
        whole.to_string()
    })
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};

    use super::*;

    fn letterboxd_watch(
        year: Option<u16>,
        rating_stars: Option<&str>,
        liked: bool,
        rewatch: bool,
    ) -> LetterboxdWatch {
        LetterboxdWatch {
            id: "film".to_string(),
            title: "Perfect Blue".to_string(),
            year,
            rating: None,
            rating_stars: rating_stars.map(str::to_string),
            watched_date: NaiveDate::from_ymd_opt(2026, 5, 2),
            rewatch,
            liked,
            poster_url: None,
            tmdb: None,
            url: "https://letterboxd.example/watch".to_string(),
            published_at: Utc.with_ymd_and_hms(2026, 5, 2, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn puts_available_metadata_and_action_on_separate_lines() {
        assert_eq!(
            detail_lines(&letterboxd_watch(Some(1997), Some("★★★★★"), false, true)),
            vec!["rated 5/5", "rewatched"]
        );
        assert_eq!(
            detail_lines(&letterboxd_watch(Some(1997), Some("★★★½"), true, false)),
            vec!["rated 3.5/5", "liked"]
        );
        assert_eq!(
            detail_lines(&letterboxd_watch(None, None, false, false)),
            vec!["watched"]
        );
    }

    #[test]
    fn omits_unrecognized_rating_stars() {
        assert_eq!(
            detail_lines(&letterboxd_watch(Some(1997), Some("unrated"), false, false)),
            vec!["watched"]
        );
    }
}
