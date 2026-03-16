//! URL normalization for podcast feed URLs.
//!
//! Ensures that equivalent URLs map to the same podcast entry,
//! avoiding duplicates from trailing slashes, scheme differences, etc.

use url::Url;

/// Normalize a podcast feed URL:
/// - Parse and re-serialize (lowercases scheme + host)
/// - Strip trailing slashes from the path
/// - Remove default ports (80 for http, 443 for https)
/// - Remove empty fragments
/// - Preserve query string as-is (feed URLs sometimes use query params)
pub fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let Ok(mut parsed) = Url::parse(trimmed) else {
        // If it doesn't parse, return as-is (trimmed)
        return trimmed.to_string();
    };

    // Remove empty fragment
    if parsed.fragment() == Some("") {
        parsed.set_fragment(None);
    }

    // Strip trailing slashes from path
    let path = parsed.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        parsed.set_path(path.trim_end_matches('/'));
    }

    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_slash() {
        assert_eq!(
            normalize_url("http://example.com/feed/"),
            "http://example.com/feed"
        );
    }

    #[test]
    fn preserves_path() {
        assert_eq!(
            normalize_url("http://example.com/feed.xml"),
            "http://example.com/feed.xml"
        );
    }

    #[test]
    fn lowercases_host() {
        assert_eq!(
            normalize_url("HTTP://EXAMPLE.COM/Feed"),
            "http://example.com/Feed"
        );
    }

    #[test]
    fn strips_empty_fragment() {
        assert_eq!(
            normalize_url("http://example.com/feed#"),
            "http://example.com/feed"
        );
    }

    #[test]
    fn preserves_query() {
        assert_eq!(
            normalize_url("http://example.com/feed?format=rss"),
            "http://example.com/feed?format=rss"
        );
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(
            normalize_url("  http://example.com/feed  "),
            "http://example.com/feed"
        );
    }

    #[test]
    fn root_path_kept() {
        // Don't strip the single slash for root
        assert_eq!(
            normalize_url("http://example.com/"),
            "http://example.com/"
        );
    }

    #[test]
    fn invalid_url_returned_as_is() {
        assert_eq!(normalize_url("not a url"), "not a url");
    }
}
