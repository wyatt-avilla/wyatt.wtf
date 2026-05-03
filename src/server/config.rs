use std::{
    fs,
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Parser;

use super::error::{BackendError, Result};

#[derive(Clone, Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(long, value_name = "PATH")]
    pub lastfm_api_key_path: PathBuf,

    #[arg(long, value_name = "PATH")]
    pub goodreads_rss_url_path: PathBuf,

    #[arg(long, default_value = "wyattwtf")]
    pub lastfm_username: String,

    #[arg(long, default_value = "https://letterboxd.com/wyattwtf/rss/")]
    pub letterboxd_rss_url: String,

    #[arg(long, default_value_t = NonZeroU64::new(10).expect("non-zero default"))]
    pub upstream_timeout_seconds: NonZeroU64,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub lastfm_api_key: String,
    pub lastfm_username: String,
    pub letterboxd_rss_url: String,
    pub goodreads_rss_url: String,
    pub upstream_timeout: Duration,
}

impl TryFrom<Cli> for ServerConfig {
    type Error = BackendError;

    fn try_from(cli: Cli) -> Result<Self> {
        Ok(Self {
            lastfm_api_key: read_lastfm_api_key(&cli.lastfm_api_key_path)?,
            lastfm_username: cli.lastfm_username,
            letterboxd_rss_url: cli.letterboxd_rss_url,
            goodreads_rss_url: read_goodreads_rss_url(&cli.goodreads_rss_url_path)?,
            upstream_timeout: Duration::from_secs(cli.upstream_timeout_seconds.get()),
        })
    }
}

fn read_lastfm_api_key(path: &Path) -> Result<String> {
    read_secret_file(path, "Last.fm API key").map(|value| {
        dotenv_value(&value, "LASTFM_API_KEY")
            .unwrap_or(&value)
            .to_string()
    })
}

fn read_goodreads_rss_url(path: &Path) -> Result<String> {
    read_secret_file(path, "Goodreads RSS URL").map(|value| {
        dotenv_value(&value, "GOODREADS_RSS_URL")
            .unwrap_or(&value)
            .to_string()
    })
}

fn read_secret_file(path: &Path, label: &'static str) -> Result<String> {
    let secret = fs::read_to_string(path)
        .map_err(|source| BackendError::SecretFileRead {
            label,
            path: path.to_path_buf(),
            source,
        })?
        .trim()
        .to_string();

    if secret.is_empty() {
        return Err(BackendError::EmptySecret {
            label,
            path: path.to_path_buf(),
        });
    }

    Ok(secret)
}

fn dotenv_value<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    input.lines().find_map(|line| {
        let value = line
            .trim()
            .strip_prefix(key)?
            .trim_start()
            .strip_prefix('=')?
            .trim()
            .trim_matches('"');
        Some(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_trims_secret_file() {
        let path = std::env::temp_dir().join(format!("wyattwtf-secret-{}", std::process::id()));
        fs::write(&path, "  key-value\n").unwrap();

        let secret = read_secret_file(&path, "test").unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(secret, "key-value");
    }

    #[test]
    fn rejects_empty_secret_file() {
        let path =
            std::env::temp_dir().join(format!("wyattwtf-empty-secret-{}", std::process::id()));
        fs::write(&path, "\n\t").unwrap();

        let err = read_secret_file(&path, "test").unwrap_err();

        fs::remove_file(path).unwrap();
        assert!(matches!(err, BackendError::EmptySecret { .. }));
    }

    #[test]
    fn accepts_dotenv_style_lastfm_key_file() {
        let path = std::env::temp_dir().join(format!("wyattwtf-env-secret-{}", std::process::id()));
        fs::write(&path, "LASTFM_API_KEY=key-from-env\n").unwrap();

        let secret = read_lastfm_api_key(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(secret, "key-from-env");
    }

    #[test]
    fn reads_and_trims_goodreads_rss_url_file() {
        let path =
            std::env::temp_dir().join(format!("wyattwtf-goodreads-url-{}", std::process::id()));
        fs::write(
            &path,
            "  https://www.goodreads.com/user/updates_rss/1?key=secret\n",
        )
        .unwrap();

        let url = read_goodreads_rss_url(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(
            url,
            "https://www.goodreads.com/user/updates_rss/1?key=secret"
        );
    }

    #[test]
    fn accepts_dotenv_style_goodreads_rss_url_file() {
        let path =
            std::env::temp_dir().join(format!("wyattwtf-goodreads-env-{}", std::process::id()));
        fs::write(
            &path,
            "GOODREADS_RSS_URL=https://www.goodreads.com/user/updates_rss/1?key=secret\n",
        )
        .unwrap();

        let url = read_goodreads_rss_url(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(
            url,
            "https://www.goodreads.com/user/updates_rss/1?key=secret"
        );
    }
}
