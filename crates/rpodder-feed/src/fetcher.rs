//! HTTP feed fetcher with conditional GET and redirect handling.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::{Client, StatusCode, header};
use tracing::{debug, warn};
use url::{Host, Url};

use crate::site::is_public_ip;

/// Whether fetches may reach loopback, private, link-local and other
/// non-public addresses. Off by default: feed URLs come from users and from
/// feeds themselves (redirects, `<link>`), so they must not point the server
/// at its own network. Self-hosters with feeds on their LAN turn it on. While
/// it is off, a proxy set in the environment (`HTTP_PROXY`, …) is not used.
static ALLOW_PRIVATE: AtomicBool = AtomicBool::new(false);

/// Allow or forbid non-public addresses for fetchers built from now on.
pub fn allow_private_addresses(allow: bool) {
    ALLOW_PRIVATE.store(allow, Ordering::SeqCst);
}

/// Whether `url` may be fetched: http(s) only, and (unless `allow_private`)
/// not an IP literal outside the public internet. Host names are checked
/// after resolution by [`PublicResolver`], at connect time.
fn check_target(url: &Url, allow_private: bool) -> Result<(), FetchError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(FetchError::Forbidden(format!("scheme {}", url.scheme())));
    }
    let ip = match url.host() {
        Some(Host::Ipv4(ip)) => IpAddr::V4(ip),
        Some(Host::Ipv6(ip)) => IpAddr::V6(ip),
        Some(Host::Domain(_)) => return Ok(()),
        None => return Err(FetchError::Forbidden("no host".into())),
    };
    if allow_private || is_public_ip(ip) {
        Ok(())
    } else {
        Err(FetchError::Forbidden(format!("address {ip}")))
    }
}

fn check_url(url: &str, allow_private: bool) -> Result<(), FetchError> {
    let parsed = Url::parse(url).map_err(|e| FetchError::Forbidden(format!("invalid URL: {e}")))?;
    check_target(&parsed, allow_private)
}

/// System DNS resolution that drops non-public addresses, so a host name
/// can't lead to the server's own network, on the first request, a redirect,
/// or a DNS answer that changes between check and connect.
struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let public: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| is_public_ip(a.ip()))
                .collect();
            if public.is_empty() {
                return Err(format!("{host} has no public address").into());
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

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
    allow_private: bool,
}

impl FeedFetcher {
    pub fn new() -> Self {
        Self::with_limits(std::time::Duration::from_secs(30), 3)
    }

    /// A fetcher that gives up sooner: `timeout` per request and `attempts`
    /// tries in all (at least one).
    pub fn with_limits(timeout: std::time::Duration, attempts: u32) -> Self {
        Self::build(timeout, attempts, ALLOW_PRIVATE.load(Ordering::SeqCst))
    }

    fn build(timeout: std::time::Duration, attempts: u32, allow_private: bool) -> Self {
        // Every redirect hop is checked like the first URL.
        let redirect = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 10 {
                return attempt.error("too many redirects");
            }
            match check_target(attempt.url(), allow_private) {
                Ok(()) => attempt.follow(),
                Err(e) => attempt.error(e),
            }
        });
        let mut builder = Client::builder()
            .user_agent("rpodder/0.1 (+https://github.com/TODO/rpodder)")
            .timeout(timeout)
            .redirect(redirect);
        if !allow_private {
            // A proxy would resolve host names itself, past the resolver.
            builder = builder
                .no_proxy()
                .dns_resolver(std::sync::Arc::new(PublicResolver));
        }
        Self {
            client: builder.build().expect("failed to build HTTP client"),
            attempts: attempts.max(1),
            allow_private,
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
        check_url(url, self.allow_private)?;
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

                let body = read_capped(response, MAX_FEED_BYTES).await?;

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

/// Feeds larger than this are refused rather than read: the whole body is
/// held in memory and parsed, so an unbounded one can exhaust the process.
/// The largest real podcast feeds are a few MB.
pub const MAX_FEED_BYTES: usize = 32 * 1024 * 1024;

/// Read a response body as text, failing once it exceeds `limit` bytes.
async fn read_capped(mut response: reqwest::Response, limit: usize) -> Result<String, FetchError> {
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err(FetchError::TooLarge(limit));
    }
    // Decoded like `Response::text`: the Content-Type charset, else UTF-8.
    let encoding = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|t| {
            t.split(';')
                .filter_map(|p| p.trim().split_once('='))
                .find(|(k, _)| k.trim().eq_ignore_ascii_case("charset"))
                .map(|(_, v)| v.trim().trim_matches('"').to_string())
        })
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(FetchError::Http)? {
        if body.len() + chunk.len() > limit {
            return Err(FetchError::TooLarge(limit));
        }
        body.extend_from_slice(&chunk);
    }
    let (text, _, _) = encoding.decode(&body);
    Ok(text.into_owned())
}

/// Most of a web page's `<head>` fits well within this; the rest is ignored.
const MAX_PAGE_BYTES: usize = 1024 * 1024;

impl FeedFetcher {
    /// GET an HTML page once (no retries), reading at most `MAX_PAGE_BYTES`.
    /// Returns the final URL after redirects and the text read.
    pub async fn fetch_page(&self, url: &str) -> Result<(url::Url, String), FetchError> {
        check_url(url, self.allow_private)?;
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
        if check_url(url, self.allow_private).is_err() {
            return false;
        }
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
    #[error("response larger than {0} bytes")]
    TooLarge(usize),
    #[error("target not allowed: {0}")]
    Forbidden(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Serve one response with `body_len` bytes, optionally without a
    /// Content-Length (chunked), and return its URL.
    async fn serve(body_len: usize, declare_length: bool) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let head = if declare_length {
                format!("HTTP/1.1 200 OK\r\ncontent-length: {body_len}\r\n\r\n")
            } else {
                "HTTP/1.1 200 OK\r\nconnection: close\r\n\r\n".to_string()
            };
            let _ = sock.write_all(head.as_bytes()).await;
            let chunk = vec![b'a'; 64 * 1024];
            let mut sent = 0;
            while sent < body_len {
                let n = chunk.len().min(body_len - sent);
                if sock.write_all(&chunk[..n]).await.is_err() {
                    return;
                }
                sent += n;
            }
        });
        format!("http://{addr}/feed.xml")
    }

    #[tokio::test]
    async fn an_oversized_feed_is_refused() {
        let fetcher = FeedFetcher::build(std::time::Duration::from_secs(30), 1, true);
        for declare_length in [true, false] {
            let url = serve(MAX_FEED_BYTES + 1, declare_length).await;
            let result = fetcher.fetch(&url, None, None).await;
            assert!(
                matches!(result, Err(FetchError::TooLarge(_))),
                "declare_length={declare_length}"
            );
        }
    }

    #[tokio::test]
    async fn a_feed_within_the_limit_is_read() {
        let fetcher = FeedFetcher::build(std::time::Duration::from_secs(30), 1, true);
        let url = serve(1000, false).await;
        match fetcher.fetch(&url, None, None).await {
            Ok(FetchResult::Ok { body, .. }) => assert_eq!(body.len(), 1000),
            _ => panic!("expected the body"),
        }
    }

    /// Serve a 302 to `location` and return its URL.
    async fn redirect_to(location: String) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let head =
                format!("HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\n\r\n");
            let _ = sock.write_all(head.as_bytes()).await;
        });
        format!("http://{addr}/r")
    }

    #[tokio::test]
    async fn non_public_targets_are_refused() {
        let fetcher = FeedFetcher::build(std::time::Duration::from_secs(5), 1, false);
        for url in [
            "http://127.0.0.1:1/feed.xml",
            "http://[::1]:1/feed.xml",
            "http://10.0.0.1/feed.xml",
            "http://169.254.169.254/latest/meta-data/",
            "http://2130706433/feed.xml",
            "http://[::ffff:127.0.0.1]/feed.xml",
            "file:///etc/passwd",
            "ftp://example.com/feed.xml",
        ] {
            let result = fetcher.fetch(url, None, None).await;
            assert!(matches!(result, Err(FetchError::Forbidden(_))), "{url}");
            assert!(fetcher.fetch_page(url).await.is_err(), "{url}");
            assert!(!fetcher.is_image(url).await, "{url}");
        }
    }

    #[tokio::test]
    async fn a_host_name_resolving_to_loopback_is_refused() {
        let fetcher = FeedFetcher::build(std::time::Duration::from_secs(5), 1, false);
        let url = serve(10, true).await.replace("127.0.0.1", "localhost");
        assert!(fetcher.fetch(&url, None, None).await.is_err());
    }

    #[tokio::test]
    async fn a_redirect_to_a_private_address_is_refused() {
        // IP literals skip the resolver, so the strict client can reach the
        // loopback redirector directly (bypassing the first-URL check, as a
        // public host would pass it); the redirect policy must stop the hop.
        let strict = FeedFetcher::build(std::time::Duration::from_secs(5), 1, false);
        for target in [
            serve(10, true).await,
            "http://10.255.255.1/feed.xml".to_string(),
            "http://[fd00::1]/feed.xml".to_string(),
        ] {
            let err = strict
                .client
                .get(redirect_to(target.clone()).await)
                .send()
                .await
                .expect_err(&target);
            assert!(err.is_redirect(), "{target}: {err}");
        }

        // The same redirect is followed when private targets are allowed.
        let open = FeedFetcher::build(std::time::Duration::from_secs(5), 1, true);
        let target = serve(10, true).await;
        assert!(
            open.fetch(&redirect_to(target).await, None, None)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn private_targets_can_be_allowed() {
        let fetcher = FeedFetcher::build(std::time::Duration::from_secs(5), 1, true);
        assert!(
            fetcher
                .fetch(&serve(10, true).await, None, None)
                .await
                .is_ok()
        );
        assert!(matches!(
            fetcher.fetch("file:///etc/passwd", None, None).await,
            Err(FetchError::Forbidden(_))
        ));
    }
}
