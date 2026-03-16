//! Detect private/paid podcast feed URLs that contain access tokens.
//!
//! These feeds should not be exposed in the public directory to avoid
//! leaking paid subscription tokens.

/// Check if a feed URL likely contains an access token or API key.
///
/// Heuristics:
/// - Path segments with long random-looking strings (base64, hex)
/// - Query parameters with token-like names
/// - Very long query strings
pub fn is_likely_private_url(url: &str) -> bool {
    // Check query parameters for token-like keys
    if let Some(query_start) = url.find('?') {
        let query = &url[query_start + 1..];
        let token_params = [
            "token=", "key=", "auth=", "access=", "api_key=", "apikey=",
            "secret=", "pass=", "password=", "credential=", "sid=",
            "access_token=", "api-key=",
        ];
        let query_lower = query.to_lowercase();
        for param in &token_params {
            if query_lower.contains(param) {
                return true;
            }
        }
    }

    // Parse the path and check for random-looking segments
    let path = if let Some(q) = url.find('?') {
        &url[..q]
    } else {
        url
    };

    // Get path after the host
    let path_part = if let Some(idx) = path.find("://") {
        let after_scheme = &path[idx + 3..];
        if let Some(slash) = after_scheme.find('/') {
            &after_scheme[slash..]
        } else {
            return false;
        }
    } else {
        path
    };

    // Check each path segment
    for segment in path_part.split('/') {
        if segment.is_empty() {
            continue;
        }
        // Skip common non-token segments
        if segment == "rss"
            || segment == "feed"
            || segment == "podcast"
            || segment == "atom"
            || segment == "xml"
            || segment == "api"
            || segment.ends_with(".xml")
            || segment.ends_with(".rss")
            || segment.ends_with(".atom")
        {
            continue;
        }
        // Short numeric IDs are fine (e.g. /podcast/232180/)
        if segment.len() <= 10 && segment.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // Long alphanumeric strings with mixed case or special chars look like tokens
        if looks_like_token(segment) {
            return true;
        }
    }

    false
}

/// Check if a string looks like a random token/key.
fn looks_like_token(s: &str) -> bool {
    // Too short to be a token
    if s.len() < 16 {
        return false;
    }

    let alpha_count = s.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let digit_count = s.chars().filter(|c| c.is_ascii_digit()).count();
    let special_count = s.chars().filter(|c| *c == '_' || *c == '-').count();
    let total_alnum = alpha_count + digit_count + special_count;

    // Must be mostly alphanumeric + dashes/underscores
    if total_alnum < s.len() * 8 / 10 {
        return false;
    }

    // Must have a mix of letters and digits (pure words are not tokens)
    if digit_count == 0 || alpha_count == 0 {
        return false;
    }

    // High entropy: has both upper and lower case, or is very long
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());

    if has_upper && has_lower {
        return true;
    }

    // Very long hex or base64 strings
    if s.len() >= 32 {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_token_in_path() {
        assert!(is_likely_private_url(
            "https://ilpostapi.k8s.one/podcast/232180/rss/wVqzuEhJHWB_Z2VYXDjZ5wKlmj0xxHaacleRBrH1yH8"
        ));
    }

    #[test]
    fn detects_token_in_query() {
        assert!(is_likely_private_url(
            "https://example.com/feed.xml?token=abc123def456"
        ));
        assert!(is_likely_private_url(
            "https://example.com/feed.xml?api_key=mySecretKey123"
        ));
    }

    #[test]
    fn allows_normal_urls() {
        assert!(!is_likely_private_url(
            "https://feeds.simplecast.com/54nAGcIl"
        ));
        assert!(!is_likely_private_url(
            "https://example.com/podcast/feed.xml"
        ));
        assert!(!is_likely_private_url(
            "https://anchor.fm/s/12345678/podcast/rss"
        ));
        assert!(!is_likely_private_url(
            "https://rss.art19.com/the-daily"
        ));
    }

    #[test]
    fn allows_short_ids() {
        assert!(!is_likely_private_url(
            "https://example.com/podcast/12345/rss"
        ));
    }

    #[test]
    fn detects_base64_token() {
        assert!(is_likely_private_url(
            "https://api.example.com/feed/aGVsbG8gd29ybGQgdGhpcyBpcyBhIHRlc3Q="
        ));
    }

    #[test]
    fn allows_slug_like_paths() {
        assert!(!is_likely_private_url(
            "https://example.com/podcast/my-awesome-podcast/feed"
        ));
    }
}
