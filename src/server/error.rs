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
    #[error("no cached data is available after refresh failed")]
    NoCachedData,
    #[error("failed to build HTTP client: {0}")]
    ClientBuild(#[source] reqwest::Error),
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

impl BackendError {
    #[must_use]
    pub fn public_message(&self) -> String {
        match self {
            Self::SecretFileRead { label, path, .. } => {
                format!("failed to read {label} from {}", path.display())
            }
            Self::EmptySecret { label, path } => {
                format!("{label} file at {} is empty", path.display())
            }
            Self::Request(err) => public_request_error(err),
            Self::Xml(_) | Self::Json(_) | Self::Date(_) | Self::Url(_) | Self::MissingField(_) => {
                "upstream response could not be parsed".to_string()
            }
            Self::NoCachedData => "no cached data is available after refresh failed".to_string(),
            Self::ClientBuild(_) => "failed to build HTTP client".to_string(),
        }
    }
}

impl IntoResponse for BackendError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            Self::SecretFileRead { .. } | Self::EmptySecret { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::ClientBuild(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoCachedData | Self::Request(_) => StatusCode::BAD_GATEWAY,
            Self::Xml(_) | Self::Json(_) | Self::Date(_) | Self::Url(_) | Self::MissingField(_) => {
                StatusCode::BAD_GATEWAY
            }
        };

        let body = Json(ApiError {
            error: self.public_message(),
        });
        (status, body).into_response()
    }
}

fn public_request_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "upstream request timed out".to_string()
    } else if let Some(status) = err.status() {
        format!("upstream request failed with status {}", status.as_u16())
    } else {
        "upstream request failed".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[tokio::test]
    async fn request_public_message_does_not_include_secret_url() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });

        let err = reqwest::Client::new()
            .get(format!("http://{addr}/feed?key=secret"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap_err();
        server.await.unwrap();

        let message = BackendError::Request(err).public_message();

        assert_eq!(message, "upstream request failed with status 500");
        assert!(!message.contains("secret"));
        assert!(!message.contains("key="));
    }
}
