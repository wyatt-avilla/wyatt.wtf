#![allow(clippy::must_use_candidate)]

mod details;

use details::activity_detail_lines;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::models::{Activity, ActivityDetails, ActivityFeed, Source, DEFAULT_ACTIVITY_LIMIT};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/wyattwtf.css" />

        <Title text="wyatt.wtf" />

        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                </Routes>
            </main>
        </Router>
    }
}

#[server(prefix = "/api", endpoint = "activity-feed")]
pub async fn get_activity_feed(limit: usize) -> Result<ActivityFeed, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let state = use_context::<crate::server::AppState>()
            .ok_or_else(|| ServerFnError::new("missing app state"))?;
        Ok(state.activity_feed(limit).await)
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "activity feed server function ran without SSR",
        ))
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let feed = Resource::new(|| (), |()| get_activity_feed(DEFAULT_ACTIVITY_LIMIT));

    view! {
        <section class="feed-shell">
            <header class="feed-header">
                <h1>"wyatt.wtf"</h1>
                <p>"recent music, books, and movies"</p>
            </header>

            <Suspense fallback=|| {
                view! { <p class="feed-status">"Loading feed..."</p> }
            }>
                {move || {
                    feed.get()
                        .map_or_else(
                            || view! { <p class="feed-status">"Loading feed..."</p> }.into_any(),
                            |result| match result {
                                Ok(feed) => view! { <ActivityFeedView feed /> }.into_any(),
                                Err(err) => {
                                    view! {
                                        <p class="feed-status feed-status--error">
                                            "Could not load the activity feed: " {err.to_string()}
                                        </p>
                                    }
                                        .into_any()
                                }
                            },
                        )
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn ActivityFeedView(feed: ActivityFeed) -> impl IntoView {
    let status = feed_status(&feed);
    let feed_is_empty = feed.items.is_empty();
    let items = StoredValue::new(feed.items);
    let (source_filters, set_source_filters) = signal(SourceFilters::all());
    let visible_items = move || {
        let filters = source_filters.get();
        items.with_value(|items| filtered_activities(items, filters))
    };
    let source_selection_is_empty = move || {
        let filters = source_filters.get();
        !feed_is_empty
            && items
                .with_value(|items| filtered_activities(items, filters))
                .is_empty()
    };

    view! {
        {(!status.is_empty()).then(|| view! { <p class="feed-status">{status}</p> })}
        <SourceFilterControls filters=source_filters set_filters=set_source_filters />
        {feed_is_empty.then(|| view! { <p class="feed-status">"No activity found."</p> })}
        <p class="feed-status" hidden=move || !source_selection_is_empty()>
            "No activity found for the selected sources."
        </p>
        <ol class="feed-list">
            <For
                each=visible_items
                key=|activity| activity.id.clone()
                children=|activity| view! { <ActivityItem activity /> }
            />
        </ol>
    }
}

fn filtered_activities(items: &[Activity], filters: SourceFilters) -> Vec<Activity> {
    items
        .iter()
        .filter(|activity| filters.includes(activity.source))
        .cloned()
        .collect()
}

#[derive(Clone, Copy)]
struct SourceFilters {
    lastfm: bool,
    goodreads: bool,
    letterboxd: bool,
}

impl SourceFilters {
    const fn all() -> Self {
        Self {
            lastfm: true,
            goodreads: true,
            letterboxd: true,
        }
    }

    const fn includes(self, source: Source) -> bool {
        match source {
            Source::Letterboxd => self.letterboxd,
            Source::Goodreads => self.goodreads,
            Source::Lastfm => self.lastfm,
        }
    }

    fn set_source(&mut self, source: Source, enabled: bool) {
        match source {
            Source::Letterboxd => self.letterboxd = enabled,
            Source::Goodreads => self.goodreads = enabled,
            Source::Lastfm => self.lastfm = enabled,
        }
    }
}

#[component]
fn SourceFilterControls(
    filters: ReadSignal<SourceFilters>,
    set_filters: WriteSignal<SourceFilters>,
) -> impl IntoView {
    view! {
        <fieldset class="source-filters" aria-label="Activity source filters">
            <SourceFilterOption source=Source::Lastfm filters set_filters />
            <SourceFilterOption source=Source::Goodreads filters set_filters />
            <SourceFilterOption source=Source::Letterboxd filters set_filters />
        </fieldset>
    }
}

#[component]
fn SourceFilterOption(
    source: Source,
    filters: ReadSignal<SourceFilters>,
    set_filters: WriteSignal<SourceFilters>,
) -> impl IntoView {
    let label = source_label(source);

    view! {
        <label class="source-filter">
            <input
                type="checkbox"
                checked=move || filters.get().includes(source)
                on:change=move |event| {
                    let enabled = event_target_checked(&event);
                    set_filters.update(|filters| filters.set_source(source, enabled));
                }
            />
            <span>{label}</span>
        </label>
    }
}

#[component]
fn ActivityItem(activity: Activity) -> impl IntoView {
    let source = source_label(activity.source);
    let image_class = format!("activity-image {}", activity_image_class(activity.source));
    let empty_image_class = format!("{image_class} activity-image--empty");
    let title = activity_title(&activity);
    let timestamp = format_timestamp(activity.occurred_at);
    let details = activity_detail_lines(&activity.details);
    let image = activity.image_url.clone().map_or_else(
        || view! { <div class=empty_image_class></div> }.into_any(),
        |url| view! { <img class=image_class src=url alt="" loading="lazy" /> }.into_any(),
    );

    view! {
        <li class="activity-item">
            {image} <div class="activity-content">
                <div class="activity-meta">
                    <span>{source}</span>
                    <time datetime=activity.occurred_at.to_rfc3339()>{timestamp}</time>
                </div>
                <a
                    class="activity-title"
                    href=activity.external_url
                    target="_blank"
                    rel="noreferrer"
                >
                    {title}
                </a>
                <div class="activity-detail">
                    {details.into_iter().map(|detail| view! { <p>{detail}</p> }).collect_view()}
                </div>
            </div>
        </li>
    }
}

fn activity_title(activity: &Activity) -> String {
    match &activity.details {
        ActivityDetails::FilmWatch(watch) => match watch.year {
            Some(year) => format!("{} ({year})", activity.title),
            None => activity.title.clone(),
        },
        ActivityDetails::BookUpdate(_) | ActivityDetails::TrackPlay(_) => activity.title.clone(),
    }
}

fn feed_status(feed: &ActivityFeed) -> String {
    let mut status = Vec::new();

    if !feed.stale_sources.is_empty() {
        let sources = feed
            .stale_sources
            .iter()
            .copied()
            .map(source_label)
            .collect::<Vec<_>>()
            .join(", ");
        status.push(format!("Showing cached data for {sources}."));
    }

    if !feed.errors.is_empty() {
        status.push(format!(
            "{} source error{}.",
            feed.errors.len(),
            if feed.errors.len() == 1 { "" } else { "s" }
        ));
    }

    status.join(" ")
}

fn source_label(source: Source) -> &'static str {
    match source {
        Source::Letterboxd => "Letterboxd",
        Source::Goodreads => "Goodreads",
        Source::Lastfm => "Last.fm",
    }
}

fn activity_image_class(source: Source) -> &'static str {
    match source {
        Source::Letterboxd => "activity-image--letterboxd",
        Source::Goodreads => "activity-image--goodreads",
        Source::Lastfm => "activity-image--lastfm",
    }
}

fn format_timestamp(timestamp: chrono::DateTime<chrono::Utc>) -> String {
    timestamp.format("%Y-%m-%d %H:%M UTC").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::models::LetterboxdWatch;

    fn letterboxd_activity(year: Option<u16>) -> Activity {
        let watch = LetterboxdWatch {
            id: "film".to_string(),
            title: "Perfect Blue".to_string(),
            year,
            rating: None,
            rating_stars: None,
            watched_date: None,
            rewatch: false,
            liked: false,
            poster_url: None,
            tmdb: None,
            url: "https://letterboxd.example/watch".to_string(),
            published_at: Utc.with_ymd_and_hms(2026, 5, 2, 12, 0, 0).unwrap(),
        };

        Activity {
            id: watch.id.clone(),
            source: Source::Letterboxd,
            occurred_at: watch.published_at,
            external_url: watch.url.clone(),
            title: watch.title.clone(),
            image_url: None,
            details: ActivityDetails::FilmWatch(watch),
        }
    }

    #[test]
    fn letterboxd_title_includes_year_when_available() {
        assert_eq!(
            activity_title(&letterboxd_activity(Some(1997))),
            "Perfect Blue (1997)"
        );
        assert_eq!(activity_title(&letterboxd_activity(None)), "Perfect Blue");
    }
}
