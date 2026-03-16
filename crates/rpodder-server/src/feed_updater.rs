//! Background feed updater — fetches and updates podcast metadata and episodes.

use std::sync::Arc;

use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use rpodder_core::repo::{EpisodeRepo, PodcastRepo, TagRepo};
use rpodder_core::types::{Tag, TagSource};
use rpodder_core::url::normalize_url;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};
use rpodder_feed::{FeedFetcher, parse_feed};

macro_rules! with_repo {
    ($db:expr, |$repo:ident| $body:expr) => {
        match $db {
            Db::Postgres(pool) => {
                let $repo = PgRepo::new(pool.clone());
                $body
            }
            Db::Sqlite(pool) => {
                let $repo = SqliteRepo::new(pool.clone());
                $body
            }
        }
    };
}

/// Update a single podcast feed: fetch, parse, update metadata and episodes.
pub async fn update_podcast_feed(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let podcast = with_repo!(db, |repo| {
        PodcastRepo::find_by_url(&repo, podcast_url).await
    })?;

    let Some(mut podcast) = podcast else {
        warn!(url = podcast_url, "podcast not found for URL");
        return Ok(());
    };

    // Fetch the feed
    let result = fetcher.fetch(podcast_url, None, None).await?;

    let body = match result {
        rpodder_feed::FetchResult::Ok { body, .. } => body,
        rpodder_feed::FetchResult::NotModified => {
            info!(url = podcast_url, "feed not modified");
            return Ok(());
        }
    };

    // Parse the feed
    let parsed = parse_feed(&body)?;

    // Update podcast metadata
    podcast.title = parsed.title;
    podcast.description = parsed.description;
    podcast.link = parsed.link;
    podcast.language = parsed.language;
    podcast.logo_url = parsed.logo_url;
    podcast.author = parsed.author;
    podcast.last_update = Some(Utc::now());
    podcast.updated_at = Utc::now();

    with_repo!(db, |repo| PodcastRepo::update(&repo, &podcast).await)?;

    // Update tags from feed categories
    if !parsed.categories.is_empty() {
        let tags: Vec<Tag> = parsed
            .categories
            .iter()
            .map(|cat| Tag {
                id: Uuid::now_v7(),
                tag: cat.clone(),
                source: TagSource::Feed,
                user_id: None,
                podcast_id: podcast.id,
            })
            .collect();

        if let Err(e) = with_repo!(db, |repo| {
            TagRepo::set_tags_for_podcast(&repo, podcast.id, &tags).await
        }) {
            warn!(url = podcast_url, error = %e, "failed to update tags");
        }
    }

    // Update episodes
    let mut episode_count = 0i64;
    for parsed_ep in &parsed.episodes {
        let ep_url = parsed_ep.media_url.as_deref().or(parsed_ep.link.as_deref());

        let Some(ep_url) = ep_url else {
            continue;
        };

        let normalized = normalize_url(ep_url);

        let (mut episode, _created) = with_repo!(db, |repo| {
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &normalized).await
        })?;

        // Update episode metadata
        episode.title = parsed_ep.title.clone();
        episode.description = parsed_ep.description.clone();
        episode.link = parsed_ep.link.clone();
        episode.guid = parsed_ep.guid.clone();
        episode.released = parsed_ep.released;
        episode.duration = parsed_ep.duration;
        episode.filesize = parsed_ep.filesize;
        episode.mimetype = parsed_ep.mimetype.clone();
        episode.updated_at = Utc::now();

        with_repo!(db, |repo| EpisodeRepo::update(&repo, &episode).await)?;

        episode_count += 1;
    }

    // Update episode count
    podcast.episode_count = episode_count;
    podcast.updated_at = Utc::now();
    with_repo!(db, |repo| PodcastRepo::update(&repo, &podcast).await)?;

    info!(
        url = podcast_url,
        title = podcast.title,
        episodes = episode_count,
        "feed updated"
    );

    Ok(())
}

/// Background task: periodically update all podcast feeds.
pub async fn run_feed_update_loop(db: Arc<Db>, interval_secs: u64) {
    let fetcher = FeedFetcher::new();

    loop {
        info!("starting feed update cycle");

        // Get all podcast URLs that need updating
        let urls = get_all_podcast_urls(&db).await;

        for url in &urls {
            if let Err(e) = update_podcast_feed(&db, &fetcher, url).await {
                error!(url, error = %e, "failed to update feed");
            }
            // Small delay between fetches to be polite
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        info!(count = urls.len(), "feed update cycle complete");
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }
}

async fn get_all_podcast_urls(db: &Db) -> Vec<String> {
    let result: Result<Vec<(String,)>, _> = match db {
        Db::Postgres(pool) => {
            sqlx::query_as(
                "SELECT pu.url FROM podcast_urls pu
                 JOIN podcasts p ON p.id = pu.podcast_id
                 WHERE pu.\"order\" = 0
                 ORDER BY p.subscribers DESC, p.last_update ASC NULLS FIRST",
            )
            .fetch_all(pool)
            .await
        }
        Db::Sqlite(pool) => {
            sqlx::query_as(
                "SELECT pu.url FROM podcast_urls pu
                 JOIN podcasts p ON p.id = pu.podcast_id
                 WHERE pu.\"order\" = 0
                 ORDER BY p.subscribers DESC, p.last_update ASC",
            )
            .fetch_all(pool)
            .await
        }
    };

    match result {
        Ok(rows) => rows.into_iter().map(|(url,)| url).collect(),
        Err(e) => {
            error!(error = %e, "failed to get podcast URLs");
            Vec::new()
        }
    }
}
