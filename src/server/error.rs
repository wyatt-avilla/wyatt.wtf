use std::{io, path::PathBuf};

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, BackendError>;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("failed to read {label} from {path}: {source}")]
    SecretFileRead {
        label: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error("{label} file at {path} is empty")]
    EmptySecret { label: &'static str, path: PathBuf },
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("xml parse failed: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("date parse failed: {0}")]
    Date(#[from] chrono::ParseError),
    #[error("url parse failed: {0}")]
    Url(#[from] url::ParseError),
    #[error("upstream data is missing {0}")]
    MissingField(&'static str),
    #[error("no cached data is available after refresh failed: {0}")]
    NoCachedData(String),
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

impl IntoResponse for BackendError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            Self::SecretFileRead { .. } | Self::EmptySecret { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::NoCachedData(_) | Self::Request(_) => StatusCode::BAD_GATEWAY,
            Self::Xml(_) | Self::Json(_) | Self::Date(_) | Self::Url(_) | Self::MissingField(_) => {
                StatusCode::BAD_GATEWAY
            }
        };

        let body = Json(ApiError {
            error: self.to_string(),
        });
        (status, body).into_response()
    }
}
