use reqwest::Client;
use roxmltree::{Document, Node};
use scraper::{Html, Selector};
use url::Url;

use crate::models::{GoodreadsAction, GoodreadsBookUpdate};

use super::{child_text, clean_text, parse_rfc2822_child};
use crate::server::error::{BackendError, Result};

const GOODREADS_BASE_URL: &str = "https://www.goodreads.com/";

#[derive(Default)]
struct GoodreadsDescriptionEnrichment {
    author: Option<String>,
    cover_url: Option<String>,
    book_url: Option<String>,
    author_url: Option<String>,
}

pub async fn fetch_goodreads(client: &Client, url: &str) -> Result<Vec<GoodreadsBookUpdate>> {
    let body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_goodreads(&body)
}

pub fn parse_goodreads(body: &str) -> Result<Vec<GoodreadsBookUpdate>> {
    let doc = Document::parse(body)?;

    doc.descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
        .filter_map(|item| parse_goodreads_item(item).transpose())
        .collect()
}

fn parse_goodreads_item(item: Node<'_, '_>) -> Result<Option<GoodreadsBookUpdate>> {
    let title_text = child_text(item, "title").unwrap_or_default();
    let description = child_text(item, "description").unwrap_or_default();
    let Some(action) = goodreads_action(&title_text, &description) else {
        return Ok(None);
    };

    let review_url =
        child_text(item, "link").ok_or(BackendError::MissingField("goodreads link"))?;
    let published_at = parse_rfc2822_child(item, "pubDate")?;
    let id = child_text(item, "guid").unwrap_or_else(|| review_url.clone());
    let enrichment = goodreads_description_enrichment(&description);
    let title = quoted_text(&title_text)
        .map(|title| clean_text(&title))
        .filter(|title| !title.is_empty())
        .ok_or(BackendError::MissingField("goodreads book title"))?;

    Ok(Some(GoodreadsBookUpdate {
        id,
        action,
        title,
        author: enrichment.author,
        rating: goodreads_rating(&description),
        cover_url: enrichment.cover_url,
        book_url: enrichment.book_url,
        author_url: enrichment.author_url,
        review_url,
        published_at,
    }))
}

fn goodreads_action(title: &str, description: &str) -> Option<GoodreadsAction> {
    let lower_title = title.to_ascii_lowercase();
    let lower_description = description.to_ascii_lowercase();

    if lower_title.contains("wants to read") {
        Some(GoodreadsAction::WantsToRead)
    } else if lower_title.contains("started reading") {
        Some(GoodreadsAction::StartedReading)
    } else if lower_title.contains("finished reading") {
        Some(GoodreadsAction::FinishedReading)
    } else if lower_description.contains("gave ") && lower_description.contains(" stars to ") {
        Some(GoodreadsAction::Rated)
    } else if lower_title.contains(" added ") || lower_title.contains(" added '") {
        Some(GoodreadsAction::Added)
    } else {
        None
    }
}

fn goodreads_rating(description: &str) -> Option<u8> {
    let lower = description.to_ascii_lowercase();
    let (_, after_gave) = lower.split_once("gave ")?;
    let (rating, _) = after_gave.split_once(" stars")?;
    rating.trim().parse().ok()
}

fn quoted_text(input: &str) -> Option<String> {
    input
        .split_once('\'')
        .and_then(|(_, rest)| rest.rsplit_once('\'').map(|(value, _)| value.to_string()))
}

fn goodreads_description_enrichment(description: &str) -> GoodreadsDescriptionEnrichment {
    let html = Html::parse_fragment(description);
    let link_selector = Selector::parse("a").expect("valid link selector");
    let image_selector = Selector::parse("img").expect("valid image selector");

    let mut enrichment = GoodreadsDescriptionEnrichment {
        cover_url: html
            .select(&image_selector)
            .next()
            .and_then(|node| node.value().attr("src"))
            .and_then(|src| absolute_goodreads_url(src).ok()),
        ..GoodreadsDescriptionEnrichment::default()
    };

    for link in html.select(&link_selector) {
        let href = link.value().attr("href");
        let text = clean_text(&link.text().collect::<String>());

        if enrichment.book_url.is_none() {
            enrichment.book_url = href
                .filter(|href| href.contains("/book/show/"))
                .and_then(|href| absolute_goodreads_url(href).ok());
        }

        if enrichment.author_url.is_none() {
            enrichment.author_url = href
                .filter(|href| href.contains("/author/show/"))
                .and_then(|href| absolute_goodreads_url(href).ok());
            if enrichment.author_url.is_some() && !text.is_empty() {
                enrichment.author = Some(text);
            }
        }
    }

    enrichment
}

fn absolute_goodreads_url(href: &str) -> Result<String> {
    Ok(Url::parse(GOODREADS_BASE_URL)?.join(href)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_filters_goodreads_book_updates() {
        let xml = r#"<?xml version="1.0"?><rss><channel>
        <item>
            <guid isPermaLink="false">Review1</guid>
            <pubDate>Thu, 19 Mar 2026 11:41:22 -0700</pubDate>
            <title><![CDATA[wyatt added 'Claude's Constitution']]></title>
            <link>https://www.goodreads.com/review/show/1</link>
            <description><![CDATA[
                <a href="/book/show/1"><img src="https://example.com/cover.jpg" /></a>
                wyatt gave 5 stars to <a href="https://www.goodreads.com/book/show/1">Claude's Constitution (Hardcover)</a>
                <span class="by">by</span><a href="/author/show/1">Amanda Askell</a>
            ]]></description>
        </item>
        <item>
            <guid isPermaLink="false">Rating1</guid>
            <pubDate>Mon, 02 Mar 2026 20:38:23 -0800</pubDate>
            <title><![CDATA[wyatt liked a review]]></title>
            <link>https://www.goodreads.com/</link>
            <description><![CDATA[wyatt liked a review]]></description>
        </item>
        </channel></rss>"#;

        let items = parse_goodreads(xml).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].action, GoodreadsAction::Rated);
        assert_eq!(items[0].title, "Claude's Constitution");
        assert_eq!(items[0].author.as_deref(), Some("Amanda Askell"));
        assert_eq!(items[0].rating, Some(5));
        assert_eq!(
            items[0].cover_url.as_deref(),
            Some("https://example.com/cover.jpg")
        );
        assert_eq!(
            items[0].book_url.as_deref(),
            Some("https://www.goodreads.com/book/show/1")
        );
        assert_eq!(
            items[0].author_url.as_deref(),
            Some("https://www.goodreads.com/author/show/1")
        );
    }
}
