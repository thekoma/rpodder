//! What a podcast's website offers when its feed leaves gaps: where the site
//! is, and an icon to use as cover art when the feed has no image.

use url::Url;

/// The podcast's website, from the channel `<link>`. Some feeds (Radio
/// Capital's, for one) echo the URL they were requested with as `<link>`;
/// that is the feed again, not a page, so the root of the feed's own site is
/// used instead. No `<link>` means no website: the feed's host may be a
/// hosting platform whose home page has nothing to do with the show.
pub fn website(link: Option<&str>, feed_url: &str) -> Option<String> {
    let link = link?.trim();
    if link.is_empty() {
        return None;
    }
    if same_resource(link, feed_url) {
        return site_root(feed_url).map(String::from);
    }
    Some(link.to_string())
}

/// Whether `link` names the feed itself: same host (ignoring case and a
/// leading `www.`) and path (ignoring a trailing `/`), whatever the scheme or
/// query. A tokenized feed echoing itself must not become the website.
fn same_resource(link: &str, feed_url: &str) -> bool {
    fn key(url: &str) -> Option<(String, String)> {
        let url = Url::parse(url).ok()?;
        let host = url.host_str()?.to_ascii_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
        Some((host, url.path().trim_end_matches('/').to_string()))
    }
    match (key(link), key(feed_url)) {
        (Some(a), Some(b)) => a == b,
        _ => link == feed_url,
    }
}

/// Whether the icon lookup may fetch `website`: http(s), and either on the
/// feed's own host (already fetched for the feed) or on a public host. The
/// `<link>` is written by whoever publishes the feed, so it must not point
/// the server at loopback, private or link-local addresses or LAN names.
pub fn lookup_allowed(website: &str, feed_url: &str) -> bool {
    use std::net::IpAddr;
    use url::Host;

    let Ok(site) = Url::parse(website) else {
        return false;
    };
    if !matches!(site.scheme(), "http" | "https") {
        return false;
    }
    // Same host and port as the feed: a place the server already fetches.
    let origin = |u: &Url| {
        u.host_str()
            .map(|h| (h.to_ascii_lowercase(), u.port_or_known_default()))
    };
    if let Ok(feed) = Url::parse(feed_url)
        && origin(&feed).is_some()
        && origin(&feed) == origin(&site)
    {
        return true;
    }
    match site.host() {
        Some(Host::Ipv4(ip)) => is_public_ip(IpAddr::V4(ip)),
        Some(Host::Ipv6(ip)) => is_public_ip(IpAddr::V6(ip)),
        Some(Host::Domain(domain)) => {
            let d = domain.trim_end_matches('.').to_ascii_lowercase();
            d.contains('.')
                && ![".local", ".lan", ".internal", ".home.arpa", ".localhost"]
                    .iter()
                    .any(|suffix| d.ends_with(suffix))
        }
        None => false,
    }
}

/// Whether an IP literal is a public internet address. IPv6 forms that carry
/// an IPv4 address (mapped, compatible, NAT64) are judged by that address.
pub(crate) fn is_public_ip(ip: std::net::IpAddr) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b))
                || (a == 198 && (18..20).contains(&b))
                || (a == 192 && b == 0 && c == 0))
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments();
            if v6.is_loopback() || v6.is_unspecified() {
                return false;
            }
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_ip(IpAddr::V4(v4));
            }
            // NAT64 (64:ff9b::/96) and IPv4-compatible (::a.b.c.d).
            let embedded = Ipv4Addr::new(
                (seg[6] >> 8) as u8,
                seg[6] as u8,
                (seg[7] >> 8) as u8,
                seg[7] as u8,
            );
            if (seg[0] == 0x64 && seg[1] == 0xff9b && seg[2..6] == [0, 0, 0, 0])
                || seg[..6] == [0, 0, 0, 0, 0, 0]
            {
                return is_public_ip(IpAddr::V4(embedded));
            }
            !((seg[0] & 0xfe00) == 0xfc00 || (seg[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// `https://host/` for an http(s) URL, without credentials, path or query.
fn site_root(url: &str) -> Option<Url> {
    let mut url = Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return None;
    }
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    let _ = url.set_username("");
    let _ = url.set_password(None);
    Some(url)
}

/// The best cover-art candidate an HTML page declares, made absolute against
/// `base`. In order: `apple-touch-icon` (square, ~180 px), the schema.org
/// logo in JSON-LD, `og:image`, then the largest `<link rel="icon">`.
pub fn find_icon(html: &str, base: &Url) -> Option<String> {
    let mut best: Option<(u8, u32, String)> = None;
    let mut offer = |rank: u8, size: u32, href: &str| {
        let Some(url) = absolute(href, base) else {
            return;
        };
        let better = match &best {
            None => true,
            Some((r, s, _)) => rank < *r || (rank == *r && size > *s),
        };
        if better {
            best = Some((rank, size, url));
        }
    };

    for tag in Tags::new(html) {
        match tag.name.as_str() {
            "link" => {
                let rel = tag.attr("rel").unwrap_or_default().to_ascii_lowercase();
                let Some(href) = tag.attr("href") else {
                    continue;
                };
                let size = largest_size(&tag.attr("sizes").unwrap_or_default());
                if rel
                    .split_whitespace()
                    .any(|r| r.starts_with("apple-touch-icon"))
                {
                    offer(0, size, &href);
                } else if rel.split_whitespace().any(|r| r == "icon") {
                    offer(3, size, &href);
                }
            }
            "meta" => {
                let key = tag
                    .attr("property")
                    .or_else(|| tag.attr("name"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if matches!(
                    key.as_str(),
                    "og:image" | "og:image:url" | "og:image:secure_url"
                ) && let Some(content) = tag.attr("content")
                {
                    offer(2, 0, &content);
                }
            }
            "script" => {
                let is_json_ld = tag
                    .attr("type")
                    .is_some_and(|t| t.to_ascii_lowercase().contains("ld+json"));
                if is_json_ld
                    && let Ok(json) = serde_json::from_str::<serde_json::Value>(tag.body)
                    && let Some(logo) = json_ld_logo(&json, 0)
                {
                    offer(1, 0, &logo);
                }
            }
            _ => {}
        }
    }
    best.map(|(_, _, url)| url)
}

fn absolute(href: &str, base: &Url) -> Option<String> {
    let url = base.join(href.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

/// Largest edge in a `sizes` attribute ("16x16 32x32", "any" for SVG).
fn largest_size(sizes: &str) -> u32 {
    sizes
        .split_whitespace()
        .filter_map(|s| {
            if s.eq_ignore_ascii_case("any") {
                return Some(512);
            }
            let lower = s.to_ascii_lowercase();
            let (w, h) = lower.split_once('x')?;
            Some(w.parse::<u32>().ok()?.max(h.parse::<u32>().ok()?))
        })
        .max()
        .unwrap_or(0)
}

/// First `"logo"` in a JSON-LD document: a URL string or an ImageObject.
fn json_ld_logo(value: &serde_json::Value, depth: u8) -> Option<String> {
    use serde_json::Value;
    fn url_of(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => o.get("url").and_then(Value::as_str).map(String::from),
            Value::Array(a) => a.iter().find_map(url_of),
            _ => None,
        }
    }
    if depth > 8 {
        return None;
    }
    match value {
        Value::Object(map) => map
            .get("logo")
            .and_then(url_of)
            .or_else(|| map.values().find_map(|v| json_ld_logo(v, depth + 1))),
        Value::Array(items) => items.iter().find_map(|v| json_ld_logo(v, depth + 1)),
        _ => None,
    }
}

/// A start tag: lowercase name, raw attributes, and for `<script>` its body.
struct Tag<'a> {
    name: String,
    attrs: Vec<(String, String)>,
    body: &'a str,
}

impl Tag<'_> {
    fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| decode_entities(v))
    }
}

/// Minimal start-tag scanner: enough to read `<link>`, `<meta>` and
/// `<script>` attributes from real-world pages without an HTML parser.
struct Tags<'a> {
    html: &'a str,
    pos: usize,
}

impl<'a> Tags<'a> {
    fn new(html: &'a str) -> Self {
        Self { html, pos: 0 }
    }
}

impl<'a> Iterator for Tags<'a> {
    type Item = Tag<'a>;

    fn next(&mut self) -> Option<Tag<'a>> {
        let bytes = self.html.as_bytes();
        loop {
            let start = self.pos + self.html.get(self.pos..)?.find('<')?;
            let rest = &self.html[start..];
            if rest.starts_with("<!--") {
                self.pos = start + rest.find("-->").map_or(rest.len(), |e| e + 3);
                continue;
            }
            let mut i = start + 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                i += 1;
            }
            if i == start + 1 {
                self.pos = start + 1;
                continue;
            }
            let name = self.html[start + 1..i].to_ascii_lowercase();
            let (attrs, end) = parse_attrs(self.html, i);
            let end = end.min(self.html.len());
            self.pos = end;
            let mut body = "";
            if name == "script" {
                let rest = &self.html[end..];
                let close = find_ascii_ci(rest, "</script").unwrap_or(rest.len());
                body = &rest[..close];
                self.pos = end + close;
            }
            return Some(Tag { name, attrs, body });
        }
    }
}

/// Byte offset of `needle` (ASCII, lowercase) in `haystack`, ignoring ASCII case.
fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Attributes from `pos` up to the closing `>`; returns them and the index
/// after `>`.
fn parse_attrs(html: &str, mut pos: usize) -> (Vec<(String, String)>, usize) {
    let bytes = html.as_bytes();
    let mut attrs = Vec::new();
    loop {
        while pos < bytes.len() && (bytes[pos].is_ascii_whitespace() || bytes[pos] == b'/') {
            pos += 1;
        }
        if pos >= bytes.len() {
            return (attrs, pos);
        }
        if bytes[pos] == b'>' {
            return (attrs, pos + 1);
        }
        let key_start = pos;
        while pos < bytes.len()
            && !bytes[pos].is_ascii_whitespace()
            && !matches!(bytes[pos], b'=' | b'>' | b'/')
        {
            pos += 1;
        }
        let key = html[key_start..pos].to_ascii_lowercase();
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let mut value = String::new();
        if pos < bytes.len() && bytes[pos] == b'=' {
            pos += 1;
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos < bytes.len() && matches!(bytes[pos], b'"' | b'\'') {
                let quote = bytes[pos];
                let value_start = pos + 1;
                let value_end = bytes[value_start..]
                    .iter()
                    .position(|&b| b == quote)
                    .map_or(bytes.len(), |e| value_start + e);
                value = html[value_start..value_end].to_string();
                pos = (value_end + 1).min(bytes.len());
            } else {
                let value_start = pos;
                while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'>' {
                    pos += 1;
                }
                value = html[value_start..pos].to_string();
            }
        }
        // A stray `=value` with no name was consumed above; just drop it.
        if !key.is_empty() {
            attrs.push((key, value));
        }
    }
}

fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x2F;", "/")
        .replace("&#47;", "/")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://www.example.it/").unwrap()
    }

    #[test]
    fn website_replaces_a_link_that_echoes_the_feed() {
        let feed = "https://www.capital.it/api/pub/v1/rss/google-assistant/833";
        assert_eq!(
            website(Some(feed), feed).as_deref(),
            Some("https://www.capital.it/")
        );
        assert_eq!(
            website(Some("https://show.example/"), feed).as_deref(),
            Some("https://show.example/")
        );
        // No <link>: the feed's host may be a hosting platform, not the show.
        assert_eq!(website(None, "https://feeds.megaphone.fm/ABC"), None);
        assert_eq!(website(Some("  "), feed), None);
    }

    #[test]
    fn apple_touch_icon_wins_over_the_rest() {
        let html = r#"<html><head>
            <link rel="icon" href="/favicon.ico" sizes="32x32">
            <meta property="og:image" content="https://cdn.example.it/share.jpg">
            <link rel="apple-touch-icon" sizes="180x180" href="/apple.png">
        </head></html>"#;
        assert_eq!(
            find_icon(html, &base()).as_deref(),
            Some("https://www.example.it/apple.png")
        );
    }

    #[test]
    fn schema_org_logo_beats_og_image_and_favicon() {
        // What www.capital.it serves: a 32 px favicon and a JSON-LD logo.
        let html = r#"<head>
            <link rel="icon" href="https://www.capital.it/wp-content/themes/network-capital/favicon.ico" />
            <SCRIPT type="application/ld+json">
            {"@context":"https://schema.org","publisher":[{"@type":"Organization",
              "logo":{"@type":"ImageObject","url":"https://www.capital.it/img/logo_rss.png"}}]}
            </script>
            <meta content="https://www.capital.it/share.jpg" property="og:image">
        </head>"#;
        assert_eq!(
            find_icon(html, &base()).as_deref(),
            Some("https://www.capital.it/img/logo_rss.png")
        );
    }

    #[test]
    fn largest_declared_icon_and_entities() {
        let html = r#"<link rel="shortcut icon" href="/i16.png" sizes="16x16">
            <link rel=icon href="/i?s=192&amp;v=2" sizes="192x192">
            <!-- <link rel="apple-touch-icon" href="/commented.png"> -->"#;
        assert_eq!(
            find_icon(html, &base()).as_deref(),
            Some("https://www.example.it/i?s=192&v=2")
        );
    }

    #[test]
    fn a_tokenized_feed_echoing_itself_does_not_leak_its_token() {
        let feed = "https://private.example/feed.xml?token=s3cret";
        assert_eq!(
            website(Some("http://www.private.example/feed.xml/"), feed).as_deref(),
            Some("https://private.example/")
        );
    }

    #[test]
    fn lookups_stay_off_internal_hosts() {
        let feed = "https://www.capital.it/rss/833";
        assert!(lookup_allowed("https://www.capital.it/", feed));
        assert!(lookup_allowed("https://show.example.org/", feed));
        // The feed's own host and port are fine even when local (already
        // fetched); another port on it is not.
        assert!(lookup_allowed(
            "http://127.0.0.1:8080/",
            "http://127.0.0.1:8080/feed.xml"
        ));
        assert!(lookup_allowed(
            "https://www.capital.it/",
            "https://www.capital.it:443/rss"
        ));
        assert!(!lookup_allowed(
            "http://127.0.0.1:6379/",
            "http://127.0.0.1:8080/feed.xml"
        ));
        for internal in [
            "http://127.0.0.1/",
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.64.0.1/",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://[::ffff:192.168.1.1]/",
            "http://[::ffff:100.64.0.1]/",
            "http://[::ffff:0.0.0.0]/",
            "http://[64:ff9b::a00:1]/",
            "http://[::7f00:1]/",
            "http://0.1.2.3/",
            "http://198.18.0.1/",
            "http://192.0.0.8/",
            "http://240.0.0.1/",
            "http://localhost:3005/",
            "http://router/",
            "http://nas.lan/",
            "http://printer.local/",
            "file:///etc/passwd",
            "not a url",
        ] {
            assert!(!lookup_allowed(internal, feed), "{internal}");
        }
    }

    #[test]
    fn malformed_markup_never_panics() {
        for html in [
            "<link =\"x\"é rel=icon href=/a.png>",
            "<script =",
            "<script =\"x",
            "<a =>x<link rel=icon href=/b.png>",
            "<link rel='icon' href='/c.png",
            "<!-- unterminated",
            "<script type=application/ld+json>{\"logo\":\"/d.png\"}",
            "<<<>>>==\"\"''é€😀<",
        ] {
            let _ = find_icon(html, &base());
        }
        // Deterministic byte soup with multibyte characters.
        let alphabet = [
            '<', '>', '=', '"', '\'', ' ', '/', '-', '!', 'a', 'l', 'é', '😀', 'x',
        ];
        let mut seed: u32 = 12345;
        for _ in 0..2000 {
            let mut doc = String::new();
            for _ in 0..40 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                doc.push(alphabet[(seed >> 16) as usize % alphabet.len()]);
            }
            let _ = find_icon(&doc, &base());
        }
    }

    #[test]
    fn nothing_usable() {
        let html = r#"<link rel="stylesheet" href="/a.css"><meta property="og:image" content="data:image/png;base64,AAA">
            <script type="application/ld+json">{not json</script><img src="<">"#;
        assert_eq!(find_icon(html, &base()), None);
        assert_eq!(find_icon("", &base()), None);
        assert_eq!(find_icon("<", &base()), None);
        assert_eq!(
            find_icon("<link rel='icon' href='x", &base()).as_deref(),
            Some("https://www.example.it/x")
        );
    }
}
