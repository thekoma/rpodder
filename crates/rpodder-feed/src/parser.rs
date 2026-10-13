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

    let author = feed.authors.iter().find_map(|a| a.name.clone());

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

            // Find the enclosure (media URL). feed-rs 3 gives an item one media
            // object per source: iTunes tags come first (duration, no URL),
            // the RSS <enclosure> after, so look through all of them.
            // Prefer audio/video: a feed may list an image <media:content> first.
            // (feed-rs drops <enclosure> when an item has <media:content>, so an
            // image-only MediaRSS item still yields the image.)
            let with_url = || {
                entry
                    .media
                    .iter()
                    .flat_map(|m| m.content.iter())
                    .filter(|mc| mc.url.is_some())
            };
            let media_content = with_url()
                .find(|mc| {
                    mc.content_type
                        .as_ref()
                        .is_some_and(|t| matches!(t.ty().as_str(), "audio" | "video"))
                })
                .or_else(|| with_url().next());

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
                .or_else(|| entry.media.iter().find_map(|m| m.duration))
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
    fn finds_the_enclosure_after_itunes_tags() {
        // Megaphone-style item: iTunes tags before the <enclosure>. feed-rs 3
        // turns them into a media object of their own, with no URL.
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
        <channel><title>Tg</title><description>News</description>
        <itunes:author>la7</itunes:author>
        <item>
          <title>Edition 7:30</title>
          <pubDate>Tue, 13 Oct 2026 06:22:46 -0000</pubDate>
          <itunes:duration>907</itunes:duration>
          <itunes:summary>Morning news</itunes:summary>
          <enclosure url="https://traffic.example/ed.mp3" length="0" type="audio/mpeg"/>
        </item>
        </channel></rss>"#;

        let feed = parse_feed(rss).unwrap();
        let ep = &feed.episodes[0];
        assert_eq!(
            ep.media_url.as_deref(),
            Some("https://traffic.example/ed.mp3")
        );
        assert_eq!(ep.mimetype.as_deref(), Some("audio/mpeg"));
        assert_eq!(ep.duration, Some(907));
    }

    #[test]
    fn prefers_audio_over_an_image_media_content() {
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
        <channel><title>Blog</title><description>Posts</description>
        <item>
          <title>Post with audio</title>
          <media:content url="https://blog.example/cover.jpg" type="image/jpeg"/>
          <media:content url="https://blog.example/ep.mp3" type="audio/mpeg"/>
        </item>
        </channel></rss>"#;

        let ep = &parse_feed(rss).unwrap().episodes[0];
        assert_eq!(ep.media_url.as_deref(), Some("https://blog.example/ep.mp3"));
        assert_eq!(ep.mimetype.as_deref(), Some("audio/mpeg"));
    }

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
