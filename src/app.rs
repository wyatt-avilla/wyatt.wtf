#![allow(clippy::must_use_candidate)]

use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::models::{
    Activity, ActivityDetails, ActivityFeed, GoodreadsAction, LastfmTrack, LetterboxdWatch, Source,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
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
        <Stylesheet id="leptos" href="/pkg/wyattwtf.css"/>

        <Title text="wyatt.wtf"/>

        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
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
    let feed = Resource::new(|| (), |()| get_activity_feed(30));

    view! {
        <section class="feed-shell">
            <header class="feed-header">
                <h1>"wyatt.wtf"</h1>
                <p>"recent movies, books, and music"</p>
            </header>

            <Suspense fallback=|| view! { <p class="feed-status">"Loading feed..."</p> }>
                {move || {
                    feed.get().map_or_else(
                        || view! { <p class="feed-status">"Loading feed..."</p> }.into_any(),
                        |result| match result {
                            Ok(feed) => view! { <ActivityFeedView feed/> }.into_any(),
                            Err(err) => view! {
                                <p class="feed-status feed-status--error">
                                    "Could not load the activity feed: " {err.to_string()}
                                </p>
                            }.into_any(),
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
    let items = feed.items;
    let content = if items.is_empty() {
        view! { <p class="feed-status">"No activity found."</p> }.into_any()
    } else {
        view! {
            <ol class="feed-list">
                <For
                    each=move || items.clone()
                    key=|activity| activity.id.clone()
                    children=|activity| view! { <ActivityItem activity/> }
                />
            </ol>
        }
        .into_any()
    };

    view! {
        {(!status.is_empty()).then(|| view! {
            <p class="feed-status">{status}</p>
        })}
        {content}
    }
}

#[component]
fn ActivityItem(activity: Activity) -> impl IntoView {
    let source = source_label(activity.source);
    let timestamp = format_timestamp(activity.occurred_at);
    let detail = activity_detail(&activity.details);
    let image = activity.image_url.clone().map_or_else(
        || view! { <div class="activity-image activity-image--empty"></div> }.into_any(),
        |url| {
            view! {
                <img class="activity-image" src=url alt="" loading="lazy"/>
            }
            .into_any()
        },
    );

    view! {
        <li class="activity-item">
            {image}
            <div class="activity-content">
                <div class="activity-meta">
                    <span>{source}</span>
                    <time datetime=activity.occurred_at.to_rfc3339()>{timestamp}</time>
                </div>
                <a class="activity-title" href=activity.external_url target="_blank" rel="noreferrer">
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
