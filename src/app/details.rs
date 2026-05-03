#![allow(clippy::must_use_candidate)]

mod goodreads;
mod lastfm;
mod letterboxd;

use crate::models::ActivityDetails;

pub(super) fn activity_detail_lines(details: &ActivityDetails) -> Vec<String> {
    match details {
        ActivityDetails::FilmWatch(watch) => letterboxd::detail_lines(watch),
        ActivityDetails::BookUpdate(update) => goodreads::detail_lines(update),
        ActivityDetails::TrackPlay(track) => lastfm::detail_lines(track),
    }
}
