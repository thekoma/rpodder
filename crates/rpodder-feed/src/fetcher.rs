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

    /// Fetch a feed URL. Optionally provide ETag/Last-Modified from a previous fetch
    /// to enable conditional GET (returns NotModified if the feed hasn't changed).
    pub async fn fetch(
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
