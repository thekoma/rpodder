//! Parse RSS/Atom feed XML into rpodder domain types.

use chrono::{DateTime, Utc};

/// Parsed feed data ready to be persisted.
pub struct ParsedFeed {
    pub title: String,
    pub description: String,
    pub link: Option<String>,
    pub language: Option<String>,
    pub logo_url: Option<String>,
    pub author: Option<String>,
    pub categories: Vec<String>,
    pub episodes: Vec<ParsedEpisode>,
}

pub struct ParsedEpisode {
    pub guid: Option<String>,
    pub title: String,
    pub description: String,
    pub link: Option<String>,
    pub media_url: Option<String>,
    pub released: Option<DateTime<Utc>>,
    pub duration: Option<i64>,
    pub filesize: Option<i64>,
    pub mimetype: Option<String>,
}

/// Parse feed XML into structured data.
pub fn parse_feed(xml: &str) -> Result<ParsedFeed, ParseError> {
    let feed = feed_rs::parser::parse(xml.as_bytes()).map_err(ParseError::Parse)?;

    let title = feed.title.map(|t| t.content).unwrap_or_default();

    let description = feed.description.map(|t| t.content).unwrap_or_default();

    let link = feed.links.first().map(|l| l.href.clone());

    let language = feed.language.clone();

    let logo_url = feed
        .logo
        .as_ref()
        .map(|img| img.uri.clone())
        .or_else(|| feed.icon.as_ref().map(|img| img.uri.clone()));

    let author = feed.authors.first().map(|a| a.name.clone());

    let categories: Vec<String> = feed.categories.iter().map(|c| c.term.clone()).collect();

    let episodes: Vec<ParsedEpisode> = feed
        .entries
        .iter()
        .map(|entry| {
            let guid = entry.id.clone();

            let title = entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_default();

            let description = entry
                .summary
                .as_ref()
                .map(|t| t.content.clone())
                .or_else(|| entry.content.as_ref().and_then(|c| c.body.clone()))
                .unwrap_or_default();

            let link = entry.links.first().map(|l| l.href.clone());

            // Find enclosure (media URL)
            let media = entry.media.first();
            let media_content = media.and_then(|m| m.content.first());

            let media_url = media_content
                .and_then(|mc| mc.url.as_ref())
                .map(|u| u.to_string())
                .or_else(|| {
                    // Fallback: look in links for enclosure type
                    entry
                        .links
                        .iter()
                        .find(|l| l.rel.as_deref() == Some("enclosure"))
                        .map(|l| l.href.clone())
                });

            let released = entry.published.or(entry.updated);

            let duration = media_content
                .and_then(|mc| mc.duration)
                .map(|d| d.as_secs() as i64);

            let filesize = media_content.and_then(|mc| mc.size).map(|s| s as i64);

            let mimetype = media_content
                .and_then(|mc| mc.content_type.as_ref())
                .map(|m| m.to_string());

            ParsedEpisode {
                guid: Some(guid),
                title,
                description,
                link,
                media_url,
                released,
                duration,
                filesize,
                mimetype,
            }
        })
        .collect();

    Ok(ParsedFeed {
        title,
        description,
        link,
        language,
        logo_url,
        author,
        categories,
        episodes,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("feed parse error: {0}")]
    Parse(feed_rs::parser::ParseFeedError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rss_pubdate() {
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0">
        <channel><title>Test Pod</title><description>A test</description>
        <item>
          <title>Episode 1</title>
          <pubDate>Mon, 18 Mar 2026 10:00:00 +0000</pubDate>
          <enclosure url="http://example.com/ep1.mp3" type="audio/mpeg" length="1234"/>
        </item>
        <item>
          <title>Episode 2</title>
          <pubDate>Sun, 17 Mar 2026 09:00:00 +0000</pubDate>
          <enclosure url="http://example.com/ep2.mp3" type="audio/mpeg" length="5678"/>
        </item>
        </channel></rss>"#;

        let feed = parse_feed(rss).unwrap();
        assert_eq!(feed.title, "Test Pod");
        assert_eq!(feed.episodes.len(), 2);
        assert_eq!(feed.episodes[0].title, "Episode 1");
        assert!(
            feed.episodes[0].released.is_some(),
            "Episode 1 should have released date"
        );
        assert!(
            feed.episodes[1].released.is_some(),
            "Episode 2 should have released date"
        );
        // Episode 1 should be newer
        assert!(feed.episodes[0].released.unwrap() > feed.episodes[1].released.unwrap());
    }
}
