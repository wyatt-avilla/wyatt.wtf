#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::models::{
    Activity, ActivityDetails, ActivityFeed, GoodreadsAction, LastfmTrack, LetterboxdWatch, Source,
    DEFAULT_ACTIVITY_LIMIT,
};

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
                <p>"recent movies, books, and music"</p>
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
    let timestamp = format_timestamp(activity.occurred_at);
    let detail = activity_detail(&activity.details);
    let image = activity.image_url.clone().map_or_else(
        || view! { <div class="activity-image activity-image--empty"></div> }.into_any(),
        |url| view! { <img class="activity-image" src=url alt="" loading="lazy" /> }.into_any(),
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
                    {activity.title}
                </a>
                <p class="activity-detail">{detail}</p>
            </div>
        </li>
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

fn activity_detail(details: &ActivityDetails) -> String {
    match details {
        ActivityDetails::FilmWatch(watch) => film_detail(watch),
        ActivityDetails::BookUpdate(update) => {
            let action = goodreads_action_label(update.action);
            match (&update.author, update.rating) {
                (Some(author), Some(rating)) => format!("{action} by {author} - {rating}/5"),
                (Some(author), None) => format!("{action} by {author}"),
                (None, Some(rating)) => format!("{action} - {rating}/5"),
                (None, None) => action.to_string(),
            }
        }
        ActivityDetails::TrackPlay(track) => track_detail(track),
    }
}

fn film_detail(watch: &LetterboxdWatch) -> String {
    let mut parts = Vec::new();

    if let Some(year) = watch.year {
        parts.push(year.to_string());
    }
    if let Some(rating) = watch.rating_stars.as_ref() {
        parts.push(rating.clone());
    }
    if watch.liked {
        parts.push("liked".to_string());
    }
    if watch.rewatch {
        parts.push("rewatch".to_string());
    }

    if parts.is_empty() {
        "watched on Letterboxd".to_string()
    } else {
        parts.join(" - ")
    }
}

fn track_detail(track: &LastfmTrack) -> String {
    let prefix = if track.now_playing {
        "now playing"
    } else {
        "played"
    };

    match &track.album {
        Some(album) => format!("{prefix} by {} from {album}", track.artist),
        None => format!("{prefix} by {}", track.artist),
    }
}

fn source_label(source: Source) -> &'static str {
    match source {
        Source::Letterboxd => "Letterboxd",
        Source::Goodreads => "Goodreads",
        Source::Lastfm => "Last.fm",
    }
}

fn goodreads_action_label(action: GoodreadsAction) -> &'static str {
    match action {
        GoodreadsAction::WantsToRead => "wants to read",
        GoodreadsAction::StartedReading => "started reading",
        GoodreadsAction::FinishedReading => "finished reading",
        GoodreadsAction::Rated => "rated",
        GoodreadsAction::Added => "added",
    }
}

fn format_timestamp(timestamp: chrono::DateTime<chrono::Utc>) -> String {
    timestamp.format("%Y-%m-%d %H:%M UTC").to_string()
}
