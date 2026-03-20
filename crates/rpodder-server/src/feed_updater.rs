//! Background feed updater — fetches and updates podcast metadata and episodes.

use std::sync::Arc;

use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

use rpodder_core::privacy::is_likely_private_url;
use rpodder_core::repo::{EpisodeRepo, PodcastRepo, SubscriptionRepo, TagRepo};
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
/// Errors are logged but do not stop processing — each step is best-effort.
/// If `force` is true, skips the private URL check (for authenticated user access).
pub async fn update_podcast_feed(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    update_podcast_feed_inner(db, fetcher, podcast_url, false).await
}

/// Same as update_podcast_feed but allows forcing private feed fetch.
pub async fn update_podcast_feed_forced(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    update_podcast_feed_inner(db, fetcher, podcast_url, true).await
}

async fn update_podcast_feed_inner(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
    force: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let podcast = with_repo!(db, |repo| {
        PodcastRepo::find_by_url(&repo, podcast_url).await
    })?;

    let mut podcast = match podcast {
        Some(p) => p,
        None if force => {
            // On-demand: create a stub podcast entry so we can fetch the feed
            let (p, _) = with_repo!(db, |repo| {
                PodcastRepo::get_or_create_for_url(&repo, podcast_url).await
            })?;
            info!(url = podcast_url, "created podcast on-demand");
            p
        }
        None => {
            warn!(url = podcast_url, "podcast not found for URL");
            return Ok(());
        }
    };

    // Skip private/paid feeds in background updates — but allow forced fetch for authenticated users
    if !force && is_likely_private_url(podcast_url) {
        info!(
            url = podcast_url,
            "skipping private feed (token detected in URL)"
        );
        return Ok(());
    }

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

    // Dedup: if this is a fresh podcast (no episodes yet), check if the episodes
    // already belong to another podcast. This catches duplicate URLs pointing to
    // the same feed (e.g. /833 vs /833.rss).
    if podcast.episode_count == 0
        && let Some(first_ep) = parsed.episodes.first()
    {
        let ep_url = first_ep
            .media_url
            .as_deref()
            .or(first_ep.link.as_deref())
            .map(normalize_url);
        if let Some(ep_url) = ep_url {
            let existing_podcast_id = with_repo!(db, |repo| {
                EpisodeRepo::find_podcast_id_by_episode_url(&repo, &ep_url).await
            })?;
            if let Some(canonical_id) = existing_podcast_id
                && canonical_id != podcast.id
            {
                info!(
                    url = podcast_url,
                    canonical_id = %canonical_id,
                    duplicate_id = %podcast.id,
                    "detected duplicate podcast, adding URL as alias"
                );
                with_repo!(db, |repo| {
                    PodcastRepo::add_url(&repo, canonical_id, podcast_url).await
                })?;
                with_repo!(db, |repo| {
                    SubscriptionRepo::migrate_podcast(&repo, podcast.id, canonical_id).await
                })
                .unwrap_or_else(|e| {
                    warn!(error = %e, "failed to migrate subscriptions during dedup");
                });
                if let Err(e) = with_repo!(db, |repo| PodcastRepo::delete(&repo, podcast.id).await)
                {
                    warn!(error = %e, "failed to delete duplicate podcast");
                }
                return Ok(());
            }
        }
    }

    // Update podcast metadata (best-effort)
    podcast.title = parsed.title;
    podcast.description = parsed.description;
    podcast.link = parsed.link;
    podcast.language = parsed.language;
    podcast.logo_url = parsed.logo_url;
    podcast.author = parsed.author;
    podcast.last_update = Some(Utc::now());
    podcast.updated_at = Utc::now();

    if let Err(e) = with_repo!(db, |repo| PodcastRepo::update(&repo, &podcast).await) {
        warn!(url = podcast_url, error = %e, "failed to update podcast metadata");
    }

    // Update tags from feed categories (best-effort)
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

    // Update episodes (best-effort per episode)
    let mut episode_count = 0i64;
    for parsed_ep in &parsed.episodes {
        let ep_url = parsed_ep.media_url.as_deref().or(parsed_ep.link.as_deref());

        let Some(ep_url) = ep_url else {
            continue;
        };

        let normalized = normalize_url(ep_url);

        let episode_result = with_repo!(db, |repo| {
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &normalized).await
        });

        let (mut episode, _created) = match episode_result {
            Ok(ep) => ep,
            Err(e) => {
                warn!(url = ep_url, error = %e, "failed to create episode");
                continue;
            }
        };

        episode.title = parsed_ep.title.clone();
        episode.description = parsed_ep.description.clone();
        episode.link = parsed_ep.link.clone();
        episode.guid = parsed_ep.guid.clone();
        episode.released = parsed_ep.released;
        episode.duration = parsed_ep.duration;
        episode.filesize = parsed_ep.filesize;
        episode.mimetype = parsed_ep.mimetype.clone();
        episode.updated_at = Utc::now();

        if let Err(e) = with_repo!(db, |repo| EpisodeRepo::update(&repo, &episode).await) {
            warn!(url = ep_url, error = %e, "failed to update episode");
            continue;
        }

        episode_count += 1;
    }

    // Update episode count and adaptive interval
    podcast.episode_count = episode_count;
    podcast.update_interval_hours = adaptive_interval(podcast.subscribers, episode_count);
    podcast.updated_at = Utc::now();
    if let Err(e) = with_repo!(db, |repo| PodcastRepo::update(&repo, &podcast).await) {
        warn!(url = podcast_url, error = %e, "failed to update episode count");
    }

    info!(
        url = podcast_url,
        title = podcast.title,
        episodes = episode_count,
        "feed updated"
    );

    Ok(())
}

/// Run a single feed update cycle. Only updates feeds that are due.
pub async fn run_one_cycle(db: &Db, fetcher: &FeedFetcher) {
    info!("starting feed update cycle");

    let urls = get_due_podcast_urls(db).await;

    for url in &urls {
        if let Err(e) = update_podcast_feed(db, fetcher, url).await {
            error!(url, error = %e, "failed to update feed");
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    info!(count = urls.len(), "feed update cycle complete");
}

/// Background task: periodically update all podcast feeds.
pub async fn run_feed_update_loop(db: Arc<Db>, interval_secs: u64) {
    let fetcher = FeedFetcher::new();

    loop {
        run_one_cycle(&db, &fetcher).await;
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }
}

/// Calculate adaptive update interval based on subscriber count and activity.
/// More subscribers or more episodes → shorter interval.
fn adaptive_interval(subscribers: i64, episode_count: i64) -> i32 {
    let base = if subscribers >= 100 {
        1 // Very popular: every hour
    } else if subscribers >= 10 {
        6 // Popular: every 6 hours
    } else if subscribers >= 1 {
        24 // Has subscribers: daily
    } else {
        168 // No subscribers: weekly
    };

    // Active feeds (many episodes) get slightly faster updates
    let activity_bonus = if episode_count > 500 {
        base / 2
    } else if episode_count > 100 {
        base * 3 / 4
    } else {
        base
    };

    activity_bonus.clamp(1, 168)
}

/// Get podcast URLs that are due for update based on their update_interval_hours.
/// Podcasts with more subscribers get shorter intervals (min 1h, max 168h/7d).
async fn get_due_podcast_urls(db: &Db) -> Vec<String> {
    // Select podcasts where last_update is NULL (never updated) or
    // last_update + update_interval_hours has passed
    let result: Result<Vec<(String,)>, _> = match db {
        Db::Postgres(pool) => {
            sqlx::query_as(
                "SELECT pu.url FROM podcast_urls pu
                 JOIN podcasts p ON p.id = pu.podcast_id
                 WHERE pu.\"order\" = 0
                   AND (p.last_update IS NULL
                        OR p.last_update + make_interval(hours => p.update_interval_hours) < NOW())
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
                   AND (p.last_update IS NULL
                        OR datetime(p.last_update, '+' || p.update_interval_hours || ' hours') < datetime('now'))
                 ORDER BY p.subscribers DESC, p.last_update ASC",
            )
            .fetch_all(pool)
            .await
        }
    };

    match result {
        Ok(rows) => rows
            .into_iter()
            .map(|(url,)| url)
            .filter(|url| !is_likely_private_url(url))
            .collect(),
        Err(e) => {
            error!(error = %e, "failed to get podcast URLs");
            Vec::new()
        }
    }
}
