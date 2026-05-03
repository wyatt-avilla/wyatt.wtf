use std::{future::Future, sync::Arc, time::Duration};

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::models::{
    Activity, ActivityDetails, ActivityFeed, GoodreadsBookUpdate, LastfmTrack, LetterboxdWatch,
    Source, SourceFailure, DEFAULT_ACTIVITY_LIMIT,
};

use super::{
    config::ServerConfig,
    error::{BackendError, Result},
    sources,
};

const LASTFM_TTL: Duration = Duration::from_secs(60);
const RSS_TTL: Duration = Duration::from_secs(60 * 60);
const SOURCE_LIMIT_MAX: usize = 50;
const ACTIVITY_LIMIT_MAX: usize = 100;
const ACTIVITY_SOURCE_COUNT: usize = 3;

#[derive(Clone)]
pub struct AppState {
    config: Arc<ServerConfig>,
    client: reqwest::Client,
    cache: Arc<ActivityCache>,
}

impl AppState {
    pub fn new(config: ServerConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.upstream_timeout)
            .build()
            .map_err(BackendError::ClientBuild)?;

        Ok(Self {
            config: Arc::new(config),
            client,
            cache: Arc::new(ActivityCache::default()),
        })
    }
}

#[derive(Default)]
struct ActivityCache {
    letterboxd: RwLock<Option<Cached<Vec<LetterboxdWatch>>>>,
    goodreads: RwLock<Option<Cached<Vec<GoodreadsBookUpdate>>>>,
    lastfm: RwLock<Option<Cached<Vec<LastfmTrack>>>>,
}

#[derive(Clone)]
struct Cached<T> {
    fetched_at: DateTime<Utc>,
    items: T,
}

#[derive(Clone)]
struct CachedResult<T> {
    fetched_at: DateTime<Utc>,
    stale: bool,
    error: Option<String>,
    items: T,
}

#[derive(Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SourceResponse<T> {
    source: Source,
    fetched_at: DateTime<Utc>,
    stale: bool,
    items: Vec<T>,
}

pub fn api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/letterboxd", get(letterboxd))
        .route("/api/goodreads", get(goodreads))
        .route("/api/lastfm", get(lastfm))
        .route("/api/activity", get(activity))
        .with_state(state)
}

async fn letterboxd(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<SourceResponse<LetterboxdWatch>>> {
    let limit = source_limit(query.limit);
    let cached = state.letterboxd().await?;

    Ok(Json(SourceResponse {
        source: Source::Letterboxd,
        fetched_at: cached.fetched_at,
        stale: cached.stale,
        items: cached.items.into_iter().take(limit).collect(),
    }))
}

async fn goodreads(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<SourceResponse<GoodreadsBookUpdate>>> {
    let limit = source_limit(query.limit);
    let cached = state.goodreads().await?;

    Ok(Json(SourceResponse {
        source: Source::Goodreads,
        fetched_at: cached.fetched_at,
        stale: cached.stale,
        items: cached.items.into_iter().take(limit).collect(),
    }))
}

async fn lastfm(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<SourceResponse<LastfmTrack>>> {
    let limit = source_limit(query.limit);
    let cached = state.lastfm().await?;

    Ok(Json(SourceResponse {
        source: Source::Lastfm,
        fetched_at: cached.fetched_at,
        stale: cached.stale,
        items: cached.items.into_iter().take(limit).collect(),
    }))
}

async fn activity(
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Json<ActivityFeed> {
    Json(
        state
            .activity_feed(query.limit.unwrap_or(DEFAULT_ACTIVITY_LIMIT))
            .await,
    )
}

impl AppState {
    pub async fn activity_feed(&self, limit: usize) -> ActivityFeed {
        let limit = limit.min(ACTIVITY_LIMIT_MAX);
        let source_limit = activity_source_limit(limit);
        let fetched_at = Utc::now();
        let mut stale_sources = Vec::new();
        let mut errors = Vec::new();
        let mut items = Vec::new();

        match self.letterboxd().await {
            Ok(cached) => {
                collect_source_status(
                    Source::Letterboxd,
                    cached.stale,
                    cached.error,
                    &mut stale_sources,
                    &mut errors,
                );
                items.extend(
                    cached
                        .items
                        .into_iter()
                        .take(source_limit)
                        .map(letterboxd_activity),
                );
            }
            Err(err) => errors.push(SourceFailure {
                source: Source::Letterboxd,
                message: err.public_message(),
            }),
        }

        match self.goodreads().await {
            Ok(cached) => {
                collect_source_status(
                    Source::Goodreads,
                    cached.stale,
                    cached.error,
                    &mut stale_sources,
                    &mut errors,
                );
                items.extend(
                    cached
                        .items
                        .into_iter()
                        .take(source_limit)
                        .map(goodreads_activity),
                );
            }
            Err(err) => errors.push(SourceFailure {
                source: Source::Goodreads,
                message: err.public_message(),
            }),
        }

        match self.lastfm().await {
            Ok(cached) => {
                collect_source_status(
                    Source::Lastfm,
                    cached.stale,
                    cached.error,
                    &mut stale_sources,
                    &mut errors,
                );
                items.extend(
                    cached
                        .items
                        .into_iter()
                        .take(source_limit)
                        .map(|item| lastfm_activity(item, cached.fetched_at)),
                );
            }
            Err(err) => errors.push(SourceFailure {
                source: Source::Lastfm,
                message: err.public_message(),
            }),
        }

        items.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        items.truncate(limit);

        ActivityFeed {
            fetched_at,
            stale_sources,
            errors,
            items,
        }
    }

    async fn letterboxd(&self) -> Result<CachedResult<Vec<LetterboxdWatch>>> {
        get_or_fetch(&self.cache.letterboxd, RSS_TTL, || async {
            sources::fetch_letterboxd(&self.client, &self.config.letterboxd_rss_url).await
        })
        .await
    }

    async fn goodreads(&self) -> Result<CachedResult<Vec<GoodreadsBookUpdate>>> {
        get_or_fetch(&self.cache.goodreads, RSS_TTL, || async {
            sources::fetch_goodreads(&self.client, &self.config.goodreads_rss_url).await
        })
        .await
    }

    async fn lastfm(&self) -> Result<CachedResult<Vec<LastfmTrack>>> {
        get_or_fetch(&self.cache.lastfm, LASTFM_TTL, || async {
            sources::fetch_lastfm(
                &self.client,
                &self.config.lastfm_username,
                &self.config.lastfm_api_key,
            )
            .await
        })
        .await
    }
}

async fn get_or_fetch<T, F, Fut>(
    slot: &RwLock<Option<Cached<Vec<T>>>>,
    ttl: Duration,
    fetch: F,
) -> Result<CachedResult<Vec<T>>>
where
    T: Clone,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Vec<T>>>,
{
    let cached = slot.read().await.clone();
    if let Some(cached) = cached.as_ref() {
        let max_age = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX);
        if Utc::now().signed_duration_since(cached.fetched_at) < max_age {
            return Ok(CachedResult {
                fetched_at: cached.fetched_at,
                stale: false,
                error: None,
                items: cached.items.clone(),
            });
        }
    }

    match fetch().await {
        Ok(items) => {
            let fetched_at = Utc::now();
            let cached = Cached { fetched_at, items };
            *slot.write().await = Some(cached.clone());
            Ok(CachedResult {
                fetched_at,
                stale: false,
                error: None,
                items: cached.items,
            })
        }
        Err(err) => {
            if let Some(cached) = cached {
                return Ok(CachedResult {
                    fetched_at: cached.fetched_at,
                    stale: true,
                    error: Some(err.public_message()),
                    items: cached.items,
                });
            }

            Err(BackendError::NoCachedData)
        }
    }
}

fn source_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(10).min(SOURCE_LIMIT_MAX)
}

fn activity_source_limit(limit: usize) -> usize {
    limit.div_ceil(ACTIVITY_SOURCE_COUNT).max(1)
}

fn collect_source_status(
    source: Source,
    stale: bool,
    error: Option<String>,
    stale_sources: &mut Vec<Source>,
    errors: &mut Vec<SourceFailure>,
) {
    if stale {
        stale_sources.push(source);
    }
    if let Some(message) = error {
        errors.push(SourceFailure { source, message });
    }
}

fn letterboxd_activity(item: LetterboxdWatch) -> Activity {
    Activity {
        id: item.id.clone(),
        source: Source::Letterboxd,
        occurred_at: item.published_at,
        external_url: item.url.clone(),
        title: item.title.clone(),
        image_url: item.poster_url.clone(),
        details: ActivityDetails::FilmWatch(item),
    }
}

fn goodreads_activity(item: GoodreadsBookUpdate) -> Activity {
    Activity {
        id: item.id.clone(),
        source: Source::Goodreads,
        occurred_at: item.published_at,
        external_url: item.review_url.clone(),
        title: item.title.clone(),
        image_url: item.cover_url.clone(),
        details: ActivityDetails::BookUpdate(item),
    }
}

fn lastfm_activity(item: LastfmTrack, fetched_at: DateTime<Utc>) -> Activity {
    Activity {
        id: item.id.clone(),
        source: Source::Lastfm,
        occurred_at: item.played_at.unwrap_or(fetched_at),
        external_url: item.url.clone(),
        title: item.title.clone(),
        image_url: item.album_art_url.clone(),
        details: ActivityDetails::TrackPlay(item),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GoodreadsAction;

    fn test_state(
        letterboxd_items: Vec<LetterboxdWatch>,
        goodreads_items: Vec<GoodreadsBookUpdate>,
        lastfm_items: Vec<LastfmTrack>,
    ) -> AppState {
        AppState {
            config: Arc::new(ServerConfig {
                lastfm_api_key: "test".to_string(),
                lastfm_username: "wyattwtf".to_string(),
                letterboxd_rss_url: "https://letterboxd.example/rss".to_string(),
                goodreads_rss_url: "https://goodreads.example/rss".to_string(),
                upstream_timeout: Duration::from_secs(1),
            }),
            client: reqwest::Client::new(),
            cache: Arc::new(ActivityCache {
                letterboxd: RwLock::new(Some(Cached {
                    fetched_at: Utc::now(),
                    items: letterboxd_items,
                })),
                goodreads: RwLock::new(Some(Cached {
                    fetched_at: Utc::now(),
                    items: goodreads_items,
                })),
                lastfm: RwLock::new(Some(Cached {
                    fetched_at: Utc::now(),
                    items: lastfm_items,
                })),
            }),
        }
    }

    fn letterboxd_watch(id: &str, published_at: DateTime<Utc>) -> LetterboxdWatch {
        LetterboxdWatch {
            id: id.to_string(),
            title: "Movie".to_string(),
            year: Some(2026),
            rating: None,
            rating_stars: None,
            watched_date: None,
            rewatch: false,
            liked: false,
            poster_url: None,
            tmdb: None,
            url: format!("https://letterboxd.example/{id}"),
            published_at,
        }
    }

    fn goodreads_update(id: &str, published_at: DateTime<Utc>) -> GoodreadsBookUpdate {
        GoodreadsBookUpdate {
            id: id.to_string(),
            action: GoodreadsAction::Added,
            title: "Book".to_string(),
            author: None,
            rating: None,
            cover_url: None,
            book_url: None,
            author_url: None,
            review_url: format!("https://goodreads.example/{id}"),
            published_at,
        }
    }

    fn lastfm_track(id: &str, played_at: DateTime<Utc>) -> LastfmTrack {
        LastfmTrack {
            id: id.to_string(),
            title: "Track".to_string(),
            artist: "Artist".to_string(),
            album: None,
            album_art_url: None,
            url: format!("https://last.fm/{id}"),
            played_at: Some(played_at),
            now_playing: false,
            artist_mbid: None,
            album_mbid: None,
        }
    }

    #[tokio::test]
    async fn activity_feed_keeps_each_source_from_being_crowded_out() {
        let now = Utc::now();
        let state = test_state(
            vec![letterboxd_watch("movie", now - chrono::Duration::hours(3))],
            vec![goodreads_update("book", now - chrono::Duration::hours(2))],
            (0..5)
                .map(|index| {
                    lastfm_track(
                        &format!("track-{index}"),
                        now - chrono::Duration::minutes(index),
                    )
                })
                .collect(),
        );

        let feed = state.activity_feed(3).await;

        assert_eq!(
            feed.items
                .iter()
                .map(|activity| activity.source)
                .collect::<Vec<_>>(),
            vec![Source::Lastfm, Source::Goodreads, Source::Letterboxd]
        );
    }

    #[tokio::test]
    async fn returns_fresh_cached_values_without_fetching() {
        let cache = RwLock::new(Some(Cached {
            fetched_at: Utc::now(),
            items: vec![1, 2, 3],
        }));

        let result = get_or_fetch(&cache, Duration::from_secs(60), || async {
            Err(BackendError::MissingField("should not fetch"))
        })
        .await
        .unwrap();

        assert!(!result.stale);
        assert_eq!(result.items, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn returns_stale_cached_values_after_refresh_failure() {
        let cache = RwLock::new(Some(Cached {
            fetched_at: Utc::now() - chrono::Duration::seconds(120),
            items: vec![1, 2, 3],
        }));

        let result = get_or_fetch(&cache, Duration::from_secs(60), || async {
            Err(BackendError::MissingField("refresh"))
        })
        .await
        .unwrap();

        assert!(result.stale);
        assert_eq!(result.items, vec![1, 2, 3]);
        assert_eq!(
            result.error.as_deref(),
            Some("upstream response could not be parsed")
        );
    }

    #[test]
    fn clamps_source_limits() {
        assert_eq!(source_limit(None), 10);
        assert_eq!(source_limit(Some(2)), 2);
        assert_eq!(source_limit(Some(200)), SOURCE_LIMIT_MAX);
    }

    #[test]
    fn derives_activity_source_limits_from_overall_limit() {
        assert_eq!(activity_source_limit(1), 1);
        assert_eq!(activity_source_limit(3), 1);
        assert_eq!(activity_source_limit(DEFAULT_ACTIVITY_LIMIT), 20);
    }
}
