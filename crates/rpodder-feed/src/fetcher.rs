//! HTTP feed fetcher with conditional GET and redirect handling.

use reqwest::{Client, StatusCode, header};
use tracing::{debug, warn};

/// Result of fetching a feed.
pub enum FetchResult {
    /// Feed content was retrieved (new or updated).
    Ok {
        body: String,
        etag: Option<String>,
        last_modified: Option<String>,
        /// If the server redirected, this is the final URL.
        final_url: Option<String>,
    },
    /// Feed has not been modified since last fetch (304).
    NotModified,
}

/// HTTP feed fetcher with conditional GET support.
pub struct FeedFetcher {
    client: Client,
}

impl FeedFetcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("rpodder/0.1 (+https://github.com/TODO/rpodder)")
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// Fetch a feed URL with retry (up to 3 attempts with exponential backoff).
    /// Optionally provide ETag/Last-Modified from a previous fetch
    /// to enable conditional GET (returns NotModified if the feed hasn't changed).
    pub async fn fetch(
        &self,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FetchResult, FetchError> {
        let mut last_err = None;
        for attempt in 0..3u32 {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                debug!(
                    url,
                    attempt,
                    delay_ms = delay.as_millis(),
                    "retrying feed fetch"
                );
                tokio::time::sleep(delay).await;
            }
            match self.fetch_once(url, etag, last_modified).await {
                Ok(result) => return Ok(result),
                Err(FetchError::Status(code)) if code >= 500 => {
                    warn!(url, status = code, attempt, "server error, will retry");
                    last_err = Some(FetchError::Status(code));
                }
                Err(FetchError::Http(e)) if e.is_timeout() || e.is_connect() => {
                    warn!(url, attempt, error = %e, "connection error, will retry");
                    last_err = Some(FetchError::Http(e));
                }
                Err(e) => return Err(e), // Non-retryable error
            }
        }
        Err(last_err.unwrap())
    }

    async fn fetch_once(
        &self,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FetchResult, FetchError> {
        let mut req = self.client.get(url);

        if let Some(etag) = etag {
            req = req.header(header::IF_NONE_MATCH, etag);
        }
        if let Some(lm) = last_modified {
            req = req.header(header::IF_MODIFIED_SINCE, lm);
        }

        let response = req.send().await.map_err(FetchError::Http)?;

        // Check for redirect — compare final URL with original
        let final_url = {
            let final_str = response.url().as_str();
            if final_str != url {
                debug!(original = url, final_url = final_str, "feed URL redirected");
                Some(final_str.to_string())
            } else {
                None
            }
        };

        match response.status() {
            StatusCode::NOT_MODIFIED => {
                debug!(url, "feed not modified (304)");
                Ok(FetchResult::NotModified)
            }
            status if status.is_success() => {
                let etag = response
                    .headers()
                    .get(header::ETAG)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let last_modified = response
                    .headers()
                    .get(header::LAST_MODIFIED)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let body = response.text().await.map_err(FetchError::Http)?;

                Ok(FetchResult::Ok {
                    body,
                    etag,
                    last_modified,
                    final_url,
                })
            }
            status => {
                warn!(url, %status, "feed fetch failed");
                Err(FetchError::Status(status.as_u16()))
            }
        }
    }
}

impl Default for FeedFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP error: {0}")]
    Http(reqwest::Error),
    #[error("HTTP status {0}")]
    Status(u16),
}
