use std::sync::Arc;

use chrono::Utc;
use reqwest::Url;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::models::Source;

use super::{config::ServerConfig, error::BackendError};

const RESEND_EMAILS_URL: &str = "https://api.resend.com/emails";

#[derive(Clone)]
pub struct ErrorNotifier {
    client: reqwest::Client,
    config: Arc<ServerConfig>,
    endpoint: Url,
    failing_sources: Arc<Mutex<[bool; 3]>>,
}

impl ErrorNotifier {
    pub fn new(client: reqwest::Client, config: Arc<ServerConfig>) -> Self {
        Self {
            client,
            config,
            endpoint: Url::parse(RESEND_EMAILS_URL).expect("Resend endpoint is a valid URL"),
            failing_sources: Arc::new(Mutex::new([false; 3])),
        }
    }

    pub async fn report_failure(&self, source: Source, error: &BackendError) {
        let diagnostic = error.diagnostic_message();
        eprintln!("{source:?} refresh failed: {diagnostic}");

        if !self.begin_outage(source).await {
            return;
        }

        let notifier = self.clone();
        drop(tokio::spawn(async move {
            if let Err(error) = notifier.send(source, &diagnostic).await {
                eprintln!(
                    "failed to send {source:?} error notification: {}",
                    error.diagnostic_message()
                );
            }
        }));
    }

    pub async fn report_recovery(&self, source: Source) {
        self.failing_sources.lock().await[source_index(source)] = false;
    }

    async fn begin_outage(&self, source: Source) -> bool {
        let mut failing_sources = self.failing_sources.lock().await;
        let failing = &mut failing_sources[source_index(source)];
        if *failing {
            false
        } else {
            *failing = true;
            true
        }
    }

    async fn send(&self, source: Source, diagnostic: &str) -> Result<(), BackendError> {
        let source_name = source_name(source);
        let payload = ResendEmail {
            from: &self.config.error_email_from,
            to: [&self.config.error_email_to],
            subject: format!("[wyatt.wtf] {source_name} source refresh failed"),
            text: format!(
                "The {source_name} activity source failed to refresh at {}.\n\n{diagnostic}",
                Utc::now().to_rfc3339()
            ),
        };

        self.client
            .post(self.endpoint.clone())
            .bearer_auth(&self.config.resend_api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    #[cfg(test)]
    fn with_endpoint(mut self, endpoint: Url) -> Self {
        self.endpoint = endpoint;
        self
    }

    #[cfg(test)]
    pub(crate) fn suppress_all(mut self) -> Self {
        self.failing_sources = Arc::new(Mutex::new([true; 3]));
        self
    }
}

#[derive(Serialize)]
struct ResendEmail<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: String,
    text: String,
}

const fn source_index(source: Source) -> usize {
    match source {
        Source::Letterboxd => 0,
        Source::Goodreads => 1,
        Source::Lastfm => 2,
    }
}

const fn source_name(source: Source) -> &'static str {
    match source {
        Source::Letterboxd => "Letterboxd",
        Source::Goodreads => "Goodreads",
        Source::Lastfm => "Last.fm",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    fn test_config() -> Arc<ServerConfig> {
        Arc::new(ServerConfig {
            lastfm_api_key: "lastfm-key".to_string(),
            resend_api_key: "resend-key".to_string(),
            lastfm_username: "wyattwtf".to_string(),
            letterboxd_rss_url: "https://letterboxd.example/rss".to_string(),
            goodreads_rss_url: "https://goodreads.example/rss".to_string(),
            error_email_from: "wyatt.wtf <notifications@wyatt.wtf>".to_string(),
            error_email_to: "owner@example.com".to_string(),
            upstream_timeout: Duration::from_secs(1),
        })
    }

    async fn test_server(expected_requests: usize) -> (Url, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint =
            Url::parse(&format!("http://{}/emails", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..expected_requests {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = vec![0; 4096];
                let read = socket.read(&mut buffer).await.unwrap();
                requests.push(String::from_utf8_lossy(&buffer[..read]).into_owned());
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                    .await
                    .unwrap();
            }
            requests
        });
        (endpoint, server)
    }

    #[tokio::test]
    async fn sends_resend_email_for_source_failure() {
        let (endpoint, server) = test_server(1).await;
        let notifier =
            ErrorNotifier::new(reqwest::Client::new(), test_config()).with_endpoint(endpoint);

        notifier
            .report_failure(Source::Goodreads, &BackendError::MissingField("title"))
            .await;

        let request = server.await.unwrap().pop().unwrap();
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer resend-key")
        );
        assert!(request.contains("Goodreads source refresh failed"));
        assert!(request.contains("owner@example.com"));
        assert!(request.contains("upstream data is missing title"));
    }

    #[tokio::test]
    async fn sends_once_per_source_outage_and_resets_after_recovery() {
        let (endpoint, server) = test_server(2).await;
        let notifier =
            ErrorNotifier::new(reqwest::Client::new(), test_config()).with_endpoint(endpoint);
        let error = BackendError::MissingField("title");

        notifier.report_failure(Source::Letterboxd, &error).await;
        notifier.report_failure(Source::Letterboxd, &error).await;
        notifier.report_recovery(Source::Letterboxd).await;
        notifier.report_failure(Source::Letterboxd, &error).await;

        assert_eq!(server.await.unwrap().len(), 2);
    }
}
