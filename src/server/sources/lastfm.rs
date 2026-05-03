use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::models::LastfmTrack;

use crate::server::error::Result;

pub async fn fetch_lastfm(
    client: &Client,
    username: &str,
    api_key: &str,
) -> Result<Vec<LastfmTrack>> {
    let response = client
        .get("https://ws.audioscrobbler.com/2.0/")
        .query(&[
            ("method", "user.getrecenttracks"),
            ("user", username),
            ("api_key", api_key),
            ("format", "json"),
            ("limit", "50"),
        ])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    parse_lastfm(&response)
}

pub fn parse_lastfm(body: &str) -> Result<Vec<LastfmTrack>> {
    let response: LastfmRecentTracksResponse = serde_json::from_str(body)?;

    Ok(response
        .recenttracks
        .track
        .into_iter()
        .map(LastfmTrack::from_api)
        .collect())
}

#[derive(Debug, Deserialize)]
struct LastfmRecentTracksResponse {
    recenttracks: LastfmRecentTracks,
}

#[derive(Debug, Deserialize)]
struct LastfmRecentTracks {
    track: Vec<LastfmApiTrack>,
}

#[derive(Debug, Deserialize)]
struct LastfmApiTrack {
    name: String,
    artist: LastfmText,
    album: LastfmText,
    image: Vec<LastfmImage>,
    url: String,
    date: Option<LastfmDate>,
    #[serde(rename = "@attr")]
    attr: Option<LastfmAttrs>,
}

#[derive(Debug, Deserialize)]
struct LastfmText {
    mbid: Option<String>,
    #[serde(rename = "#text")]
    text: String,
}

#[derive(Debug, Deserialize)]
struct LastfmImage {
    #[serde(rename = "#text")]
    text: String,
}

#[derive(Debug, Deserialize)]
struct LastfmDate {
    uts: String,
}

#[derive(Debug, Deserialize)]
struct LastfmAttrs {
    nowplaying: Option<String>,
}

impl LastfmTrack {
    fn from_api(track: LastfmApiTrack) -> Self {
        let now_playing = track
            .attr
            .and_then(|attr| attr.nowplaying)
            .is_some_and(|value| value == "true");
        let played_at = track
            .date
            .and_then(|date| date.uts.parse::<i64>().ok())
            .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0));
        let album = none_if_empty(track.album.text);
        let album_mbid = none_if_empty(track.album.mbid.unwrap_or_default());
        let artist_mbid = none_if_empty(track.artist.mbid.unwrap_or_default());
        let album_art_url = track
            .image
            .into_iter()
            .rev()
            .map(|image| image.text)
            .find(|url| !url.is_empty());
        let id_time = played_at.map_or_else(
            || "now-playing".to_string(),
            |time| time.timestamp().to_string(),
        );
        let id = format!("lastfm:{id_time}:{}:{}", track.artist.text, track.name);

        Self {
            id,
            title: track.name,
            artist: track.artist.text,
            album,
            album_art_url,
            url: track.url,
            played_at,
            now_playing,
            artist_mbid,
            album_mbid,
        }
    }
}

fn none_if_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lastfm_now_playing_and_scrobble() {
        let json = r##"{
          "recenttracks": { "track": [
            {
              "name": "Track A",
              "artist": {"mbid": "", "#text": "Artist A"},
              "album": {"mbid": "", "#text": "Album A"},
              "image": [{"size": "small", "#text": ""}, {"size": "extralarge", "#text": "https://example.com/a.jpg"}],
              "url": "https://last.fm/a",
              "@attr": {"nowplaying": "true"}
            },
            {
              "name": "Track B",
              "artist": {"mbid": "artist-mbid", "#text": "Artist B"},
              "album": {"mbid": "album-mbid", "#text": ""},
              "image": [],
              "url": "https://last.fm/b",
              "date": {"uts": "1777762923", "#text": "02 May 2026, 23:02"}
            }
          ] }
        }"##;

        let items = parse_lastfm(json).unwrap();

        assert_eq!(items.len(), 2);
        assert!(items[0].now_playing);
        assert_eq!(items[0].played_at, None);
        assert_eq!(
            items[0].album_art_url.as_deref(),
            Some("https://example.com/a.jpg")
        );
        assert_eq!(items[1].artist_mbid.as_deref(), Some("artist-mbid"));
        assert_eq!(items[1].album, None);
    }
}
