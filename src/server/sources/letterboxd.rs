use chrono::NaiveDate;
use reqwest::Client;
use roxmltree::{Document, Node};
use scraper::{Html, Selector};

use crate::models::{LetterboxdWatch, TmdbKind, TmdbRef};

use super::{child_text, parse_rfc2822_child};
use crate::server::error::{BackendError, Result};

const POSTER_WIDTH: u16 = 150;
const POSTER_HEIGHT: u16 = 225;

pub async fn fetch_letterboxd(client: &Client, url: &str) -> Result<Vec<LetterboxdWatch>> {
    let body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_letterboxd(&body)
}

pub fn parse_letterboxd(body: &str) -> Result<Vec<LetterboxdWatch>> {
    let doc = Document::parse(body)?;

    doc.descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .filter_map(|item| parse_letterboxd_item(item).transpose())
        .collect()
}

fn parse_letterboxd_item(item: Node<'_, '_>) -> Result<Option<LetterboxdWatch>> {
    let url = child_text(item, "link").ok_or(BackendError::MissingField("letterboxd link"))?;
    let published_at = parse_rfc2822_child(item, "pubDate")?;
    let id = child_text(item, "guid").unwrap_or_else(|| url.clone());
    let title = child_text(item, "filmTitle")
        .or_else(|| {
            child_text(item, "title").and_then(|title| title.split(',').next().map(str::to_string))
        })
        .ok_or(BackendError::MissingField("letterboxd film title"))?;
    let year = child_text(item, "filmYear").and_then(|year| year.parse().ok());
    let rating = child_text(item, "memberRating").and_then(|rating| rating.parse().ok());
    let rating_stars = child_text(item, "title")
        .and_then(|title| {
            title
                .rsplit(" - ")
                .next()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|rating| rating.chars().any(|c| c == '★'));
    let watched_date = child_text(item, "watchedDate")
        .and_then(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok());
    let rewatch =
        child_text(item, "rewatch").is_some_and(|value| value.eq_ignore_ascii_case("yes"));
    let liked =
        child_text(item, "memberLike").is_some_and(|value| value.eq_ignore_ascii_case("yes"));
    let poster_url = child_text(item, "description")
        .and_then(|description| first_image_src(&description))
        .map(|url| resized_poster_url(&url));
    let tmdb = child_text(item, "movieId")
        .map(|id| TmdbRef {
            kind: TmdbKind::Movie,
            id,
        })
        .or_else(|| {
            child_text(item, "tvId").map(|id| TmdbRef {
                kind: TmdbKind::Tv,
                id,
            })
        });

    Ok(Some(LetterboxdWatch {
        id,
        title,
        year,
        rating,
        rating_stars,
        watched_date,
        rewatch,
        liked,
        poster_url,
        tmdb,
        url,
        published_at,
    }))
}

fn first_image_src(description: &str) -> Option<String> {
    let html = Html::parse_fragment(description);
    let selector = Selector::parse("img").expect("valid image selector");
    html.select(&selector)
        .next()
        .and_then(|node| node.value().attr("src"))
        .map(str::to_string)
}

fn resized_poster_url(url: &str) -> String {
    if !url.contains("ltrbxd.com/resized/") {
        return url.to_string();
    }

    let (path, query) = url
        .split_once('?')
        .map_or((url, None), |(path, query)| (path, Some(query)));
    let Some(crop_index) = path.rfind("-crop.") else {
        return url.to_string();
    };

    let prefix_and_dimensions = &path[..crop_index];
    let crop_suffix = &path[crop_index..];
    let mut parts = prefix_and_dimensions.rsplitn(5, '-');
    let Some(height) = parts.next() else {
        return url.to_string();
    };
    let Some(y_offset) = parts.next() else {
        return url.to_string();
    };
    let Some(width) = parts.next() else {
        return url.to_string();
    };
    let Some(x_offset) = parts.next() else {
        return url.to_string();
    };
    let Some(prefix) = parts.next() else {
        return url.to_string();
    };

    if x_offset != "0"
        || y_offset != "0"
        || width.parse::<u16>().is_err()
        || height.parse::<u16>().is_err()
    {
        return url.to_string();
    }

    let resized = format!("{prefix}-0-{POSTER_WIDTH}-0-{POSTER_HEIGHT}{crop_suffix}");
    match query {
        Some(query) => format!("{resized}?{query}"),
        None => resized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_letterboxd_watch() {
        let xml = r#"<?xml version="1.0"?><rss xmlns:letterboxd="https://letterboxd.com" xmlns:tmdb="https://themoviedb.org"><channel><item>
            <title>Perfect Blue, 1997 - ★★★★★</title>
            <link>https://letterboxd.com/wyattwtf/film/perfect-blue/1/</link>
            <guid isPermaLink="false">letterboxd-watch-1139305751</guid>
            <pubDate>Sun, 4 Jan 2026 20:01:04 +1300</pubDate>
            <letterboxd:watchedDate>2026-01-03</letterboxd:watchedDate>
            <letterboxd:rewatch>Yes</letterboxd:rewatch>
            <letterboxd:filmTitle>Perfect Blue</letterboxd:filmTitle>
            <letterboxd:filmYear>1997</letterboxd:filmYear>
            <letterboxd:memberRating>5.0</letterboxd:memberRating>
            <letterboxd:memberLike>No</letterboxd:memberLike>
            <tmdb:movieId>10494</tmdb:movieId>
            <description><![CDATA[<p><img src="https://a.ltrbxd.com/resized/film-poster/4/6/1/7/5/46175-perfect-blue-0-600-0-900-crop.jpg?v=1ed5878cce"/></p>]]></description>
        </item></channel></rss>"#;

        let items = parse_letterboxd(xml).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Perfect Blue");
        assert_eq!(items[0].year, Some(1997));
        assert_eq!(items[0].rating, Some(5.0));
        assert!(items[0].rewatch);
        assert_eq!(
            items[0].poster_url.as_deref(),
            Some("https://a.ltrbxd.com/resized/film-poster/4/6/1/7/5/46175-perfect-blue-0-150-0-225-crop.jpg?v=1ed5878cce")
        );
    }

    #[test]
    fn resizes_letterboxd_poster_urls() {
        assert_eq!(
            resized_poster_url(
                "https://a.ltrbxd.com/resized/sm/upload/y2/i8/9s/h1/tNr2KYytArZNSh0K8PUcHU3JnX4-0-600-0-900-crop.jpg?v=45f739a242"
            ),
            "https://a.ltrbxd.com/resized/sm/upload/y2/i8/9s/h1/tNr2KYytArZNSh0K8PUcHU3JnX4-0-150-0-225-crop.jpg?v=45f739a242"
        );
    }

    #[test]
    fn leaves_unknown_poster_urls_alone() {
        assert_eq!(
            resized_poster_url("https://example.com/poster.jpg"),
            "https://example.com/poster.jpg"
        );
        assert_eq!(
            resized_poster_url("https://a.ltrbxd.com/image/poster.jpg"),
            "https://a.ltrbxd.com/image/poster.jpg"
        );
    }
}
