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
    attempts: u32,
}

impl FeedFetcher {
    pub fn new() -> Self {
        Self::with_limits(std::time::Duration::from_secs(30), 3)
    }

    /// A fetcher that gives up sooner: `timeout` per request and `attempts`
    /// tries in all (at least one).
    pub fn with_limits(timeout: std::time::Duration, attempts: u32) -> Self {
        let client = Client::builder()
            .user_agent("rpodder/0.1 (+https://github.com/TODO/rpodder)")
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            attempts: attempts.max(1),
        }
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
        for attempt in 0..self.attempts {
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

/// Most of a web page's `<head>` fits well within this; the rest is ignored.
const MAX_PAGE_BYTES: usize = 1024 * 1024;

impl FeedFetcher {
    /// GET an HTML page once (no retries), reading at most `MAX_PAGE_BYTES`.
    /// Returns the final URL after redirects and the text read.
    pub async fn fetch_page(&self, url: &str) -> Result<(url::Url, String), FetchError> {
        let mut response = self
            .client
            .get(url)
            .header(header::ACCEPT, "text/html")
            .send()
            .await
            .map_err(FetchError::Http)?;
        if !response.status().is_success() {
            return Err(FetchError::Status(response.status().as_u16()));
        }
        // A missing Content-Type is read as HTML; a declared other type isn't.
        let is_html = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|t| t.to_ascii_lowercase().contains("html"));
        if !is_html {
            return Err(FetchError::NotHtml);
        }
        let final_url = response.url().clone();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(FetchError::Http)? {
            let room = MAX_PAGE_BYTES - body.len();
            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if body.len() >= MAX_PAGE_BYTES {
                break;
            }
        }
        Ok((final_url, String::from_utf8_lossy(&body).into_owned()))
    }

    /// Whether `url` answers with an image (one try, body not read).
    pub async fn is_image(&self, url: &str) -> bool {
        // A one-byte range: servers that honour it send almost nothing.
        let request = self.client.get(url).header(header::RANGE, "bytes=0-0");
        let Ok(response) = request.send().await else {
            return false;
        };
        response.status().is_success()
            && response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|t| t.to_ascii_lowercase().starts_with("image/"))
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
    #[error("not an HTML page")]
    NotHtml,
}
