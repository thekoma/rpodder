//! Podcast Index API client (podcastindex.org)
//!
//! Auth: HMAC-SHA1 of (key + secret + epoch_time)

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::config::AppConfig;

#[derive(Debug, Serialize, Clone)]
pub struct ExternalPodcast {
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct PodcastIndexResponse {
    feeds: Option<Vec<PodcastIndexFeed>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PodcastIndexFeed {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
    author: Option<String>,
    image: Option<String>,
    artwork: Option<String>,
    language: Option<String>,
}

fn auth_headers(config: &AppConfig) -> (String, String, String) {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let mut hasher = Sha1::new();
    hasher.update(config.podcastindex_key.as_bytes());
    hasher.update(config.podcastindex_secret.as_bytes());
    hasher.update(epoch.as_bytes());
    let auth_hash = hasher.finalize().iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    });

    (config.podcastindex_key.clone(), epoch, auth_hash)
}

/// Search Podcast Index for podcasts matching the query.
pub async fn search(config: &AppConfig, query: &str) -> Vec<ExternalPodcast> {
    if !config.podcastindex_configured() {
        return vec![];
    }

    let (key, epoch, auth_hash) = auth_headers(config);
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.podcastindex.org/api/1.0/search/byterm")
        .query(&[("q", query), ("max", "20")])
        .header("X-Auth-Key", &key)
        .header("X-Auth-Date", &epoch)
        .header("Authorization", &auth_hash)
        .header("User-Agent", "rpodder/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Podcast Index search failed");
            return vec![];
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(%status, body = %body.chars().take(200).collect::<String>(), "Podcast Index search returned error");
        return vec![];
    }

    let body: PodcastIndexResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "Podcast Index search response parse failed");
            return vec![];
        }
    };

    feeds_to_podcasts(body)
}

/// Get trending podcasts from Podcast Index.
/// Optional language filter (e.g. "it", "en").
pub async fn trending(config: &AppConfig, max: u32, lang: Option<&str>) -> Vec<ExternalPodcast> {
    if !config.podcastindex_configured() {
        return vec![];
    }

    let (key, epoch, auth_hash) = auth_headers(config);
    let client = reqwest::Client::new();

    let mut params = vec![("max", max.to_string())];
    if let Some(l) = lang {
        params.push(("lang", l.to_string()));
    }

    let resp = client
        .get("https://api.podcastindex.org/api/1.0/podcasts/trending")
        .query(&params)
        .header("X-Auth-Key", &key)
        .header("X-Auth-Date", &epoch)
        .header("Authorization", &auth_hash)
        .header("User-Agent", "rpodder/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "Podcast Index trending failed");
            return vec![];
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(%status, body = %body.chars().take(200).collect::<String>(), "Podcast Index trending returned error");
        return vec![];
    }

    let body: PodcastIndexResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "Podcast Index trending response parse failed");
            return vec![];
        }
    };

    feeds_to_podcasts(body)
}

fn feeds_to_podcasts(body: PodcastIndexResponse) -> Vec<ExternalPodcast> {
    body.feeds
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| {
            let url = f.url?;
            let title = f.title.unwrap_or_default();
            if title.is_empty() || url.is_empty() {
                return None;
            }
            Some(ExternalPodcast {
                title,
                url,
                description: f.description.filter(|d| !d.is_empty()),
                author: f.author.filter(|a| !a.is_empty()),
                logo_url: f.artwork.or(f.image).filter(|u| !u.is_empty()),
                language: f.language.filter(|l| !l.is_empty()),
                source: "podcastindex".to_string(),
            })
        })
        .collect()
}
