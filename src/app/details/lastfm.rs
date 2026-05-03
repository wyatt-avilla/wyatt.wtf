#![allow(clippy::must_use_candidate)]

use crate::models::LastfmTrack;

pub(super) fn detail_lines(track: &LastfmTrack) -> Vec<String> {
    let credit = match &track.album {
        Some(album) => format!("by {} from {album}", track.artist),
        None => format!("by {}", track.artist),
    };
    let action = if track.now_playing {
        "now playing"
    } else {
        "played"
    };

    vec![credit, action.to_string()]
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn lastfm_track(album: Option<&str>, now_playing: bool) -> LastfmTrack {
        LastfmTrack {
            id: "track".to_string(),
            title: "Track".to_string(),
            artist: "Artist".to_string(),
            album: album.map(str::to_string),
            album_art_url: None,
            url: "https://lastfm.example/track".to_string(),
            played_at: Some(Utc.with_ymd_and_hms(2026, 5, 2, 12, 0, 0).unwrap()),
            now_playing,
            artist_mbid: None,
            album_mbid: None,
        }
    }

    #[test]
    fn puts_artist_and_action_on_separate_lines() {
        assert_eq!(
            detail_lines(&lastfm_track(Some("Album"), false)),
            vec!["by Artist from Album", "played"]
        );
        assert_eq!(
            detail_lines(&lastfm_track(None, true)),
            vec!["by Artist", "now playing"]
        );
    }
}
