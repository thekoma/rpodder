//! Feed fetching and parsing for rpodder.
//!
//! Fetches RSS/Atom feeds via HTTP with conditional GET support,
//! parses them with feed-rs, and maps the results to rpodder domain types.

mod fetcher;
mod parser;
mod site;

pub use fetcher::{FeedFetcher, FetchError, FetchResult, MAX_FEED_BYTES, allow_private_addresses};
pub use parser::{ParsedEpisode, parse_feed};
pub use site::{lookup_allowed, website};

/// Cover art for a podcast whose feed (at `feed_url`) has none, found on its
/// `website`: the icon or logo the page declares (see `site::find_icon`), else,
/// when `favicon_fallback`, the site's `/favicon.ico` if it serves one. Nothing
/// is fetched unless [`lookup_allowed`] says so.
pub async fn discover_site_icon(
    fetcher: &FeedFetcher,
    website: &str,
    feed_url: &str,
    favicon_fallback: bool,
) -> Option<String> {
    if !lookup_allowed(website, feed_url) {
        tracing::debug!(website, "site icon lookup skipped: not a public host");
        return None;
    }
    match fetcher.fetch_page(website).await {
        Ok((page_url, html)) => {
            if let Some(icon) = site::find_icon(&html, &page_url) {
                return Some(icon);
            }
        }
        Err(e) => tracing::debug!(website, error = %e, "site page for icon not read"),
    }
    if !favicon_fallback {
        return None;
    }
    let favicon = url::Url::parse(website).ok()?.join("/favicon.ico").ok()?;
    fetcher
        .is_image(favicon.as_str())
        .await
        .then(|| favicon.to_string())
}
