//! Background feed updater — fetches and updates podcast metadata and episodes.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration as StdDuration, Instant};

use chrono::{DateTime, Duration, Utc};
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

/// What a feed update did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedOutcome {
    /// Not fetched: unknown podcast, or a private feed in a background run.
    Skipped,
    /// The feed answered 304 Not Modified.
    NotModified,
    /// Read and stored; `new_episodes` were not known before.
    Updated { new_episodes: i64 },
    /// The URL turned out to be another address of a known podcast.
    Merged,
}

/// Episodes stored from one read of a feed, newest first. Long-running shows
/// list a few thousand; a feed with more is cut, so one URL can't fill the
/// database.
pub const MAX_EPISODES_PER_FEED: usize = 5000;

/// Keep the newest [`MAX_EPISODES_PER_FEED`] episodes (undated ones last).
fn cap_episodes(episodes: &mut Vec<rpodder_feed::ParsedEpisode>, url: &str) {
    if episodes.len() <= MAX_EPISODES_PER_FEED {
        return;
    }
    warn!(
        url,
        total = episodes.len(),
        kept = MAX_EPISODES_PER_FEED,
        "feed lists too many episodes, keeping the newest"
    );
    episodes.sort_by_key(|e| std::cmp::Reverse(e.released));
    episodes.truncate(MAX_EPISODES_PER_FEED);
}

/// Update a single podcast feed: fetch, parse, update metadata and episodes.
/// Errors are logged but do not stop processing — each step is best-effort.
/// If `force` is true, skips the private URL check (for authenticated user access).
pub async fn update_podcast_feed(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
) -> Result<FeedOutcome, Box<dyn std::error::Error + Send + Sync>> {
    update_podcast_feed_inner(db, fetcher, podcast_url, false).await
}

/// Same as update_podcast_feed but allows forcing private feed fetch.
pub async fn update_podcast_feed_forced(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
) -> Result<FeedOutcome, Box<dyn std::error::Error + Send + Sync>> {
    update_podcast_feed_inner(db, fetcher, podcast_url, true).await
}

/// Download the feed in full and store it, without ETag / Last-Modified: an
/// admin's "Refresh feed" re-reads it even if the server would answer 304.
pub async fn reread_podcast_feed(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
) -> Result<FeedOutcome, Box<dyn std::error::Error + Send + Sync>> {
    update_podcast_feed_with(db, fetcher, podcast_url, true, false).await
}

async fn update_podcast_feed_inner(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
    force: bool,
) -> Result<FeedOutcome, Box<dyn std::error::Error + Send + Sync>> {
    update_podcast_feed_with(db, fetcher, podcast_url, force, true).await
}

async fn update_podcast_feed_with(
    db: &Db,
    fetcher: &FeedFetcher,
    podcast_url: &str,
    force: bool,
    conditional: bool,
) -> Result<FeedOutcome, Box<dyn std::error::Error + Send + Sync>> {
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
            return Ok(FeedOutcome::Skipped);
        }
    };

    // Skip private/paid feeds in background updates — but allow forced fetch for authenticated users
    if !force && is_likely_private_url(podcast_url) {
        info!(
            url = podcast_url,
            "skipping private feed (token detected in URL)"
        );
        return Ok(FeedOutcome::Skipped);
    }

    // Fetch the feed with conditional GET (ETag / Last-Modified)
    let result = fetcher
        .fetch(
            podcast_url,
            podcast.etag.as_deref().filter(|_| conditional),
            podcast
                .http_last_modified
                .as_deref()
                .filter(|_| conditional),
        )
        .await?;

    let (body, new_etag, new_last_modified) = match result {
        rpodder_feed::FetchResult::Ok {
            body,
            etag,
            last_modified,
            ..
        } => (body, etag, last_modified),
        rpodder_feed::FetchResult::NotModified => {
            info!(url = podcast_url, "feed not modified (304)");
            // Still update last_update so we don't re-check too soon, and
            // re-plan the next check: time since the newest episode grows and
            // the subscriber count may have changed since the last download.
            podcast.last_update = Some(Utc::now());
            match with_repo!(db, |repo| {
                // More than the 11 dates used: future-dated items sort first
                // on PostgreSQL and are dropped by update_interval.
                EpisodeRepo::list_for_podcast(&repo, podcast.id, 50, 0).await
            }) {
                Ok(newest) => {
                    let released: Vec<DateTime<Utc>> =
                        newest.iter().filter_map(|e| e.released).collect();
                    podcast.update_interval_hours =
                        update_interval(podcast.subscribers, &released, Utc::now());
                }
                Err(e) => warn!(url = podcast_url, error = %e, "failed to list episodes after 304"),
            }
            if let Err(e) = with_repo!(db, |repo| PodcastRepo::update(&repo, &podcast).await) {
                warn!(url = podcast_url, error = %e, "failed to update last_update after 304");
            }
            return Ok(FeedOutcome::NotModified);
        }
    };

    // Parse the feed
    let mut parsed = parse_feed(&body)?;
    drop(body);
    cap_episodes(&mut parsed.episodes, podcast_url);

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
                return Ok(FeedOutcome::Merged);
            }
        }
    }

    // Update podcast metadata — only if content actually changed
    podcast.title = parsed.title;
    podcast.description = parsed.description;
    podcast.link = rpodder_feed::website(parsed.link.as_deref(), podcast_url);
    podcast.language = parsed.language;
    podcast.logo_url = match parsed.logo_url {
        Some(logo) => Some(logo),
        // No art in the feed: look on the website (at most daily), keeping
        // the cover we had if nothing turns up.
        None => match &podcast.link {
            // An admin re-read (`!conditional`) looks again right away.
            Some(site) => {
                site_icon(
                    podcast.id,
                    site,
                    podcast_url,
                    !conditional,
                    // A tiny favicon must not replace a real cover.
                    podcast.logo_url.is_none(),
                )
                .await
            }
            None => None,
        }
        .or(podcast.logo_url.take()),
    };
    podcast.author = parsed.author;
    podcast.last_update = Some(Utc::now());
    podcast.etag = new_etag;
    podcast.http_last_modified = new_last_modified;

    let new_hash = podcast.compute_content_hash();
    if new_hash != podcast.content_hash {
        podcast.content_hash = new_hash;
        podcast.updated_at = Utc::now();
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

    // Update episodes — only write when content_hash differs
    let mut episode_count = 0i64;
    let mut episodes_updated = 0i64;
    let mut new_episodes = 0i64;
    // For the cleanup below: guid and normalized <link> of the items now stored
    // under their enclosure, and every episode this read stored.
    let mut enclosure_guids: Vec<String> = Vec::new();
    let mut enclosure_links: Vec<String> = Vec::new();
    let mut stored: Vec<Uuid> = Vec::new();
    // A failed item may be missing from `stored`: then skip the cleanup.
    let mut incomplete = false;
    for parsed_ep in &parsed.episodes {
        let ep_url = parsed_ep.media_url.as_deref().or(parsed_ep.link.as_deref());

        let Some(ep_url) = ep_url else {
            continue;
        };

        let normalized = normalize_url(ep_url);

        let episode_result = with_repo!(db, |repo| {
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &normalized).await
        });

        let (mut episode, created) = match episode_result {
            Ok(ep) => ep,
            Err(e) => {
                warn!(url = ep_url, error = %e, "failed to create episode");
                incomplete = true;
                continue;
            }
        };
        stored.push(episode.id);

        episode.title = parsed_ep.title.clone();
        episode.description = parsed_ep.description.clone();
        episode.link = parsed_ep.link.clone();
        episode.guid = parsed_ep.guid.clone();
        episode.released = parsed_ep.released;
        episode.duration = parsed_ep.duration;
        episode.filesize = parsed_ep.filesize;
        episode.mimetype = parsed_ep.mimetype.clone();

        if created {
            new_episodes += 1;
        }
        let new_hash = episode.compute_content_hash();
        if created || new_hash != episode.content_hash {
            episode.content_hash = new_hash;
            episode.updated_at = Utc::now();

            if let Err(e) = with_repo!(db, |repo| EpisodeRepo::update(&repo, &episode).await) {
                warn!(url = ep_url, error = %e, "failed to update episode");
                incomplete = true;
                continue;
            }
            episodes_updated += 1;
        }

        if parsed_ep.media_url.is_some()
            && let (Some(guid), Some(link)) = (&parsed_ep.guid, &parsed_ep.link)
        {
            enclosure_guids.push(guid.clone());
            enclosure_links.push(normalize_url(link));
        }
        episode_count += 1;
    }

    // 2026.10.1-2026.10.2 missed enclosures that followed iTunes tags and stored
    // those items under their <link> page: drop those copies now.
    let cleanup = if incomplete {
        Ok(0)
    } else {
        with_repo!(db, |repo| {
            EpisodeRepo::delete_superseded(
                &repo,
                podcast.id,
                &enclosure_guids,
                &enclosure_links,
                &stored,
            )
            .await
        })
    };
    match cleanup {
        Ok(0) => {}
        Ok(n) => info!(
            url = podcast_url,
            removed = n,
            "removed episodes stored under their web page"
        ),
        Err(e) => warn!(url = podcast_url, error = %e, "failed to remove superseded episodes"),
    }

    // Update episode count and the interval to the next check
    podcast.episode_count = episode_count;
    let released: Vec<DateTime<Utc>> = parsed.episodes.iter().filter_map(|e| e.released).collect();
    podcast.update_interval_hours = update_interval(podcast.subscribers, &released, Utc::now());
    // Write the podcast once (metadata + episode_count + etag + last_update)
    if let Err(e) = with_repo!(db, |repo| PodcastRepo::update(&repo, &podcast).await) {
        warn!(url = podcast_url, error = %e, "failed to update podcast");
    }

    info!(
        url = podcast_url,
        title = podcast.title,
        episodes = episode_count,
        episodes_updated,
        new_episodes,
        "feed updated"
    );

    Ok(FeedOutcome::Updated { new_episodes })
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

/// Site icon lookups: one short try, and at most one per podcast a day (a
/// site without an icon would otherwise be asked again on every read).
static SITE_FETCHER: LazyLock<FeedFetcher> =
    LazyLock::new(|| FeedFetcher::with_limits(StdDuration::from_secs(5), 1));
const SITE_ICON_RETRY: StdDuration = StdDuration::from_secs(24 * 60 * 60);
static SITE_ICON_TRIED: LazyLock<Mutex<HashMap<Uuid, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Cover art from the podcast's website, unless it was looked up in the last
/// day (`force` looks anyway).
async fn site_icon(
    podcast_id: Uuid,
    website: &str,
    feed_url: &str,
    force: bool,
    favicon_fallback: bool,
) -> Option<String> {
    {
        let mut tried = SITE_ICON_TRIED.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        tried.retain(|_, at| now.duration_since(*at) < SITE_ICON_RETRY);
        if tried.contains_key(&podcast_id) && !force {
            return None;
        }
        tried.insert(podcast_id, now);
    }
    rpodder_feed::discover_site_icon(&SITE_FETCHER, website, feed_url, favicon_fallback).await
}

/// Hours until a feed is checked again.
///
/// Feeds nobody on this server subscribes to are checked weekly. Subscribed
/// feeds follow their own publishing rhythm: a quarter of the typical gap
/// between recent episodes, so a news show with three editions a day is
/// checked hourly and a weekly show daily (between 1 and 24 hours). The gap is
/// never taken shorter than the time since the newest episode, so a show that
/// stopped publishing drifts back to daily checks. Popular feeds keep the
/// subscriber-based ceiling (hourly from 100 subscribers, 6 hours from 10).
fn update_interval(subscribers: i64, released: &[DateTime<Utc>], now: DateTime<Utc>) -> i32 {
    if subscribers <= 0 {
        return 168;
    }
    let ceiling: i64 = if subscribers >= 100 {
        1
    } else if subscribers >= 10 {
        6
    } else {
        24
    };

    // Newest first, ignoring dates in the future (scheduled or mis-dated items).
    let mut dates: Vec<DateTime<Utc>> = released
        .iter()
        .copied()
        .filter(|d| *d <= now + Duration::hours(1))
        .collect();
    dates.sort_unstable_by(|a, b| b.cmp(a));
    // Repeated timestamps (feeds that stamp every item with the build time,
    // back catalogues published at once) are not a publishing rhythm.
    dates.dedup();
    dates.truncate(11);
    if dates.len() < 2 {
        return ceiling as i32;
    }

    let mut gaps: Vec<i64> = dates
        .windows(2)
        .map(|w| (w[0] - w[1]).num_minutes())
        .collect();
    gaps.sort_unstable();
    let median = gaps[gaps.len() / 2];
    let since_newest = (now - dates[0]).num_minutes();
    let rhythm_hours = median.max(since_newest) / 4 / 60;

    rhythm_hours.clamp(1, ceiling) as i32
}

/// How long a feed may go unchecked before opening its page re-reads it.
pub const STALE_AFTER: Duration = Duration::hours(1);

/// Page opens don't retry a feed more often than this, even when the last
/// attempt failed (and so left `last_update` unchanged).
const RETRY_AFTER: StdDuration = StdDuration::from_secs(10 * 60);

/// How long a page request waits for the refresh before answering with what
/// it has.
const PAGE_WAIT: StdDuration = StdDuration::from_secs(4);

/// Page-open refreshes running at once, server-wide. Beyond this a request
/// answers with what it has: a crawler walking the directory can't pile up
/// outbound fetches.
const PAGE_REFRESH_SLOTS: usize = 4;

static PAGE_REFRESH_PERMITS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(PAGE_REFRESH_SLOTS)));

/// One try with a short timeout: a dead host must not hold a slot for the
/// background fetcher's 3 × 30 s. A missed refresh is retried after
/// `RETRY_AFTER`.
static PAGE_FETCHER: LazyLock<FeedFetcher> =
    LazyLock::new(|| FeedFetcher::with_limits(StdDuration::from_secs(10), 1));

/// A page-open refresh of one podcast: when it started, and whether it ended.
struct PageAttempt {
    at: Instant,
    done: Arc<std::sync::atomic::AtomicBool>,
}

/// Podcasts whose feed a page open re-read recently, by podcast id.
static RECENT_PAGE_REFRESH: LazyLock<Mutex<HashMap<Uuid, PageAttempt>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Record a re-read started elsewhere (an admin's "Refresh feed"), so page
/// opens meanwhile report it as running instead of starting another fetch.
/// Set the returned flag when it ends.
pub fn track_refresh(podcast_id: Uuid) -> Arc<std::sync::atomic::AtomicBool> {
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    RECENT_PAGE_REFRESH
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            podcast_id,
            PageAttempt {
                at: Instant::now(),
                done: done.clone(),
            },
        );
    done
}

/// Outcome of [`refresh_if_stale`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRefresh {
    /// Checked recently enough (or already being re-read): nothing to do.
    Fresh,
    /// Re-read within the wait; the stored data is current.
    Done,
    /// Still running in the background; the stored data may be behind.
    Pending,
}

/// Re-read the feed of a podcast whose page is being opened, when it hasn't
/// been checked for [`STALE_AFTER`]. Waits up to a few seconds, then lets the
/// update finish in the background.
pub async fn refresh_if_stale(
    db: Arc<Db>,
    podcast_id: Uuid,
    url: &str,
    last_update: Option<DateTime<Utc>>,
) -> PageRefresh {
    use std::sync::atomic::Ordering;

    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let permit = {
        let mut recent = RECENT_PAGE_REFRESH
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        recent.retain(|_, a| now.duration_since(a.at) < RETRY_AFTER);
        if let Some(attempt) = recent.get(&podcast_id) {
            // A re-read is still running (another viewer's, or an admin's
            // even on a fresh feed): check back too. A finished one isn't
            // retried for a while.
            return if attempt.done.load(Ordering::SeqCst) {
                PageRefresh::Fresh
            } else {
                PageRefresh::Pending
            };
        }
        if last_update.is_some_and(|t| Utc::now() - t < STALE_AFTER) {
            return PageRefresh::Fresh;
        }
        let Ok(permit) = PAGE_REFRESH_PERMITS.clone().try_acquire_owned() else {
            return PageRefresh::Fresh;
        };
        recent.insert(
            podcast_id,
            PageAttempt {
                at: now,
                done: done.clone(),
            },
        );
        permit
    };

    let url = url.to_string();
    let mut task = tokio::spawn(async move {
        if let Err(e) = update_podcast_feed_forced(&db, &PAGE_FETCHER, &url).await {
            warn!(url, error = %e, "page refresh of feed failed");
        }
        done.store(true, Ordering::SeqCst);
        drop(permit);
    });
    match tokio::time::timeout(PAGE_WAIT, &mut task).await {
        Ok(_) => PageRefresh::Done,
        Err(_) => PageRefresh::Pending,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours_ago: f64, now: DateTime<Utc>) -> DateTime<Utc> {
        now - Duration::minutes((hours_ago * 60.0) as i64)
    }

    #[test]
    fn unsubscribed_feeds_are_checked_weekly() {
        let now = Utc::now();
        let daily: Vec<_> = (0..10).map(|d| at(d as f64 * 24.0, now)).collect();
        assert_eq!(update_interval(0, &daily, now), 168);
    }

    #[test]
    fn a_news_show_with_three_editions_a_day_is_checked_hourly() {
        // TG La7: 7:30, 13:30 and 20:00, newest edition 4 hours ago.
        let now = Utc::now();
        let mut editions = Vec::new();
        for day in 0..4 {
            for h in [4.0, 10.5, 17.0] {
                editions.push(at(h + day as f64 * 24.0, now));
            }
        }
        assert_eq!(update_interval(1, &editions, now), 1);
    }

    #[test]
    fn a_daily_show_is_checked_every_six_hours() {
        let now = Utc::now();
        let daily: Vec<_> = (0..10).map(|d| at(2.0 + d as f64 * 24.0, now)).collect();
        assert_eq!(update_interval(1, &daily, now), 6);
    }

    #[test]
    fn weekly_and_finished_shows_are_checked_daily() {
        let now = Utc::now();
        let weekly: Vec<_> = (0..10).map(|w| at(30.0 + w as f64 * 168.0, now)).collect();
        assert_eq!(update_interval(1, &weekly, now), 24);
        // Published daily, but the newest episode is a month old.
        let ended: Vec<_> = (0..10).map(|d| at(720.0 + d as f64 * 24.0, now)).collect();
        assert_eq!(update_interval(1, &ended, now), 24);
    }

    #[test]
    fn popular_feeds_keep_the_subscriber_ceiling() {
        let now = Utc::now();
        let weekly: Vec<_> = (0..10).map(|w| at(30.0 + w as f64 * 168.0, now)).collect();
        assert_eq!(update_interval(10, &weekly, now), 6);
        assert_eq!(update_interval(100, &weekly, now), 1);
    }

    #[test]
    fn identical_dates_are_not_a_rhythm() {
        let now = Utc::now();
        // Every item stamped with the time the feed was generated.
        assert_eq!(update_interval(1, &[now; 10], now), 24);
        // A back catalogue published at once, then a daily show.
        let mut dates = vec![at(500.0, now); 30];
        dates.extend((0..10).map(|d| at(2.0 + d as f64 * 24.0, now)));
        assert_eq!(update_interval(1, &dates, now), 6);
    }

    #[test]
    fn feeds_without_dates_fall_back_to_the_ceiling() {
        let now = Utc::now();
        assert_eq!(update_interval(1, &[], now), 24);
        assert_eq!(update_interval(1, &[at(3.0, now)], now), 24);
        // Future-dated items are ignored rather than read as a tiny gap.
        let future = [now + Duration::days(30), now + Duration::days(29)];
        assert_eq!(update_interval(1, &future, now), 24);
    }
}

#[cfg(test)]
mod cap_tests {
    use super::*;

    fn ep(n: i64) -> rpodder_feed::ParsedEpisode {
        rpodder_feed::ParsedEpisode {
            guid: Some(n.to_string()),
            title: format!("ep {n}"),
            description: String::new(),
            link: None,
            media_url: Some(format!("http://media.example/{n}.mp3")),
            released: DateTime::from_timestamp(n * 3600, 0),
            duration: None,
            filesize: None,
            mimetype: None,
        }
    }

    #[test]
    fn a_huge_feed_keeps_the_newest_episodes() {
        let total = MAX_EPISODES_PER_FEED as i64 + 10;
        let mut episodes: Vec<_> = (0..total).map(ep).collect();
        cap_episodes(&mut episodes, "http://feed.example");
        assert_eq!(episodes.len(), MAX_EPISODES_PER_FEED);
        assert_eq!(episodes[0].title, format!("ep {}", total - 1));
        assert!(episodes.iter().all(|e| e.title != "ep 9"));
    }
}
