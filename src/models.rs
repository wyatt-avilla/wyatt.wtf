use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Letterboxd,
    Goodreads,
    Lastfm,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Activity {
    pub id: String,
    pub source: Source,
    pub occurred_at: DateTime<Utc>,
    pub external_url: String,
    pub title: String,
    pub image_url: Option<String>,
    pub details: ActivityDetails,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ActivityFeed {
    pub fetched_at: DateTime<Utc>,
    pub stale_sources: Vec<Source>,
    pub errors: Vec<SourceFailure>,
    pub items: Vec<Activity>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceFailure {
    pub source: Source,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ActivityDetails {
    FilmWatch(LetterboxdWatch),
    BookUpdate(GoodreadsBookUpdate),
    TrackPlay(LastfmTrack),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LetterboxdWatch {
    pub id: String,
    pub title: String,
    pub year: Option<u16>,
    pub rating: Option<f32>,
    pub rating_stars: Option<String>,
    pub watched_date: Option<NaiveDate>,
    pub rewatch: bool,
    pub liked: bool,
    pub poster_url: Option<String>,
    pub tmdb: Option<TmdbRef>,
    pub url: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TmdbRef {
    pub kind: TmdbKind,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TmdbKind {
    Movie,
    Tv,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GoodreadsBookUpdate {
    pub id: String,
    pub action: GoodreadsAction,
    pub title: String,
    pub author: Option<String>,
    pub rating: Option<u8>,
    pub cover_url: Option<String>,
    pub book_url: Option<String>,
    pub author_url: Option<String>,
    pub review_url: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GoodreadsAction {
    WantsToRead,
    StartedReading,
    FinishedReading,
    Rated,
    Added,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LastfmTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_art_url: Option<String>,
    pub url: String,
    pub played_at: Option<DateTime<Utc>>,
    pub now_playing: bool,
    pub artist_mbid: Option<String>,
    pub album_mbid: Option<String>,
}
