# Design: web player

Status: design, revision 4, approved for implementation by the user after the 3-revision cap (elephant/goldfish `/eg-new-feature`), branch `feat/web-player` (stacked on `fix/episode-action-sync`).

## Why

Users can manage subscriptions in the web UI but cannot listen there. They want to play episodes in the browser and, optionally, have the browser behave like another gpodder client: resume on the phone (Kasts/AntennaPod) where the browser stopped and vice versa, and optionally share subscriptions with a device sync group. Decisions taken with the user (conversation, 2026-10-12):

- D1 Mixed content: play the enclosure directly; for `http://` enclosures on an `https://` page try the `https://` variant first, and on failure show an "open externally" fallback. No server-side audio proxy.
- D2 One web device per user, uid `rpodder-web`, shared by all browsers.
- D3 Subscribe in the web UI: if `rpodder-web` is in a sync group, subscribe goes to `rpodder-web` and the backend propagates it to the group; if it is in no group, subscribe goes to every device of the user (including `rpodder-web`).
- D4 Unsubscribe in the web UI removes the podcast from every device that has it, after a confirmation listing those devices.
- D5 The two settings (position sync, sync group) are stored server-side.
- D6 Only logged-in users can play.
- D7 On logout, local positions are cleared only when position sync is on (the server holds them); with sync off they are the only copy and are kept.

## Scope

In:

1. Media URL in the episode list (`GET /api/2/data/podcast/episodes.json`, additive fields `url`, `media`).
2. New authenticated endpoints `GET /api/2/me/in-progress` ("continue listening") and `GET /api/2/me/positions?podcast=` (latest play per episode of one podcast).
3. Persistent mini-player in the layout (survives client-side navigation); play buttons and per-episode progress on the podcast page; "Continue listening" on `/subscriptions`.
4. Position sync as gpodder `play` actions from device `rpodder-web`, behind the "Sync listening positions" setting; positions also kept in localStorage.
5. "Web player sync group" selector on `/settings`.
6. Shared subscription helpers (D3/D4) replacing every `devices[0]` call site: `routes/discover/podcast/+page.svelte:47`, `routes/discover/+page.svelte:55`, `routes/discover/trending/+page.svelte:33`, `routes/discover/tag/[tag]/+page.svelte:30`, and in `routes/subscriptions/+page.svelte` the `deviceId` selection (line 28) used by `unsubscribe(url)` (lines 41-47), `upgradeToHttps(upgrade: UpgradeableSub)` (lines 49-60) and `upgradeAll()` (lines 62-75).
7. Zero-dependency frontend unit tests for the player's pure logic (`node --test`), a mock RSS fixture with real audio, docs pages.

Out (explicit):

- A new `web` value for `devices.device_type`: SQLite cannot alter a `CHECK` constraint without rebuilding `devices` and its FKs, `Db::migrate` (`crates/rpodder-db/src/lib.rs:61`) re-runs every migration on each start, and mygpo clients only know `desktop/laptop/mobile/server/other` (the SQLite `CHECK` additionally allows `tablet`). The web device is `type: "other"`, caption `Web player`.
- The legacy device `web`: the old code fell back to a device literally named `web` for users with no devices, so some users own one holding web-UI subscriptions. It is left as an ordinary device (it receives subscriptions under D3's no-group rule like any other, and can be deleted on `/devices`); no automatic migration, documented in the user guide.
- Server-side audio proxy, downloads, offline, queue, chapters, video UI.
- Creating or renaming sync groups from settings (stays on `/devices`).
- `total` count in `episodes.json`, the Vite `/subscriptions` proxy overlap, the broader UI overhaul (planned separately with the impeccable design skill).
- `CHANGELOG.md`: per `AGENTS.md`, edited only when cutting a release.
- A full frontend test runner (Vitest/Playwright in CI) and a CI job for `node --test`; only pure logic gets `node --test` tests (run locally and in the gate), UI behaviour is verified by the scripted walkthrough below.

## Surfaces touched

Backend:

- `crates/rpodder-core/src/types.rs`: `LatestPlay`, `LatestPlaysFilter`.
- `crates/rpodder-core/src/repo.rs`: `EpisodeRepo::primary_urls`, `EpisodeActionRepo::latest_plays`.
- `crates/rpodder-db/src/sqlite.rs`, `crates/rpodder-db/src/postgres.rs`: both methods.
- `crates/rpodder-server/src/routes/directory.rs`: `EpisodeListItem` (~257) gains `url`, `media`; `podcast_episodes` (~277) calls `primary_urls`.
- `crates/rpodder-server/src/routes/episodes.rs`: handlers `in_progress` and `positions` with their query/response types.
- `crates/rpodder-server/src/main.rs`: `.route("/api/2/me/in-progress", get(routes::episodes::in_progress))` and `.route("/api/2/me/positions", get(routes::episodes::positions))` in the `authenticated` router after `/api/2/me` (~343).

Frontend (`web/`):

- `package.json`: script `"test": "node --test src/lib/playerLogic.test.ts"` (Node ≥ 23.6 strips TypeScript types natively; no dependency added).
- `src/lib/playerLogic.ts` (new, no imports): pure functions. `src/lib/playerLogic.test.ts` (new).
- `src/lib/api.ts`, `src/lib/auth.svelte.ts` (session-end hooks), `src/lib/webDevice.ts` (new), `src/lib/positions.ts` (new), `src/lib/player.svelte.ts` (new).
- `src/routes/login/+page.svelte` ("session expired" message).
- `src/lib/components/MiniPlayer.svelte`, `PlayButton.svelte`, `ContinueListening.svelte` (new).
- `src/routes/+layout.svelte`, `discover/podcast/+page.svelte`, `discover/+page.svelte`, `discover/trending/+page.svelte`, `discover/tag/[tag]/+page.svelte`, `subscriptions/+page.svelte`, `settings/+page.svelte`.

Tests, fixtures, docs:

- `crates/rpodder-server/src/tests/web_player.rs` (new) + `mod web_player;` in `crates/rpodder-server/src/tests/mod.rs`.
- `tests/docker/mock-rss.py` (`/feed/player`, `/media/*`, `ThreadingHTTPServer` with a lock around the shared `state` counters); `tests/docker/tests/09_web_player.sh` (new; `08_episode_sync.sh` already exists on this branch).
- `docs/src/api/directory.md` (episodes `url`), `docs/src/api/auth.md` (the `/api/2/me/*` page: in-progress, positions), `docs/src/api/episodes.md` (the `rpodder-web` device and how it reports), `docs/src/user-guide/web-ui.md` (player, settings, subscribe semantics, legacy `web` device).

## Interfaces

### `GET /api/2/data/podcast/episodes.json` (public, unchanged auth)

`EpisodeListItem` gains two fields, serialized only when `url` is present:

- `url: Option<String>`: the episode's `episode_urls.url` with the lowest `"order"` (normalized enclosure, or the item `<link>` when there is no enclosure, `feed_updater.rs:199-206`).
- `media: Option<bool>`: `true` iff the URL came from an enclosure, computed as `episode.mimetype.is_some() || episode.link.map(normalize_url) != Some(url)` (the parser sets `mimetype` only from the enclosure, `rpodder-feed/src/parser.rs:96-98`; a `<link>`-only item stores its link as the URL).

```rust
// EpisodeRepo
/// Primary (lowest "order") URL per given episode; episodes without a URL are absent.
fn primary_urls(&self, episode_ids: &[Uuid])
    -> impl Future<Output = Result<HashMap<Uuid, String>>> + Send;
```

SQLite: `SELECT episode_id, url FROM episode_urls WHERE episode_id IN (?,…) ORDER BY episode_id, "order"` (placeholders generated from the count, ids bound). PostgreSQL: `… WHERE episode_id = ANY($1) ORDER BY episode_id, "order"` with `&[Uuid]`. First row per episode wins; empty input → empty map, no query. ≤ 100 ids.

### Latest plays (repo)

```rust
// rpodder-core/src/types.rs
pub struct LatestPlaysFilter {
    pub podcast_id: Option<Uuid>,   // only this podcast's episodes
    pub in_progress_only: bool,     // apply the "unfinished and not reset" filter below
    pub limit: Option<i64>,
}
pub struct LatestPlay {
    pub podcast_url: String,
    pub podcast_title: Option<String>,
    pub podcast_logo_url: Option<String>,
    pub episode_url: String,
    pub episode_title: Option<String>,
    pub mimetype: Option<String>,
    pub position: i32,
    pub total: Option<i32>,           // effective total
    pub timestamp: DateTime<Utc>,
    pub device_uid: Option<String>,
}
// EpisodeActionRepo
fn latest_plays(&self, user_id: Uuid, filter: &LatestPlaysFilter)
    -> impl Future<Output = Result<Vec<LatestPlay>>> + Send;
```

Per episode: the user's latest `play` action (`timestamp DESC, created_at DESC, id DESC`) among actions with non-null `episode_ref_url`. `eff_total = COALESCE(NULLIF(total,0), NULLIF(episodes.duration,0))`. With `in_progress_only`: keep iff `position > 0`, (`eff_total IS NULL` or `position < eff_total - 30`), and no `new` action (mark-as-unplayed) for that episode with a later `timestamp`. Ordered by `timestamp DESC`. Titles equal to their URL or empty → `None` (in Rust).

SQL (same on both DBs except placeholders; PostgreSQL casts `eff_total::INT4`). The optional parts are appended by the impl:

```sql
SELECT podcast_url, podcast_title, podcast_logo_url, episode_url, episode_title, mimetype,
       pos, eff_total, ts, device_uid
FROM (
  SELECT ea.episode_id, ea.user_id,
         COALESCE(ea.podcast_ref_url,
                  (SELECT pu.url FROM podcast_urls pu
                    WHERE pu.podcast_id = e.podcast_id AND pu."order" = 0)) AS podcast_url,
         p.title AS podcast_title, p.logo_url AS podcast_logo_url,
         ea.episode_ref_url AS episode_url, e.title AS episode_title, e.mimetype AS mimetype,
         ea.position AS pos,
         COALESCE(NULLIF(ea.total, 0), NULLIF(e.duration, 0)) AS eff_total,
         ea.timestamp AS ts, d.device_id AS device_uid,
         ROW_NUMBER() OVER (PARTITION BY ea.episode_id
                            ORDER BY ea.timestamp DESC, ea.created_at DESC, ea.id DESC) AS rn
  FROM episode_actions ea
  JOIN episodes e ON e.id = ea.episode_id
  LEFT JOIN podcasts p ON p.id = e.podcast_id
  LEFT JOIN devices d ON d.id = ea.device_id
  WHERE ea.user_id = :user AND ea.action = 'play' AND ea.episode_ref_url IS NOT NULL
        [AND e.podcast_id = :podcast]                                  -- podcast_id filter
) t
WHERE rn = 1 AND podcast_url IS NOT NULL
  [AND pos > 0 AND (eff_total IS NULL OR pos < eff_total - 30)
   AND NOT EXISTS (SELECT 1 FROM episode_actions n
                    WHERE n.user_id = t.user_id AND n.episode_id = t.episode_id
                      AND n.action = 'new' AND n.timestamp > t.ts)]   -- in_progress_only
ORDER BY ts DESC
[LIMIT :limit]
```

### `GET /api/2/me/in-progress?limit=N` (authenticated, rpodder-only)

`limit`: optional integer, default 20, clamped to `1..=50` (≤ 0 → 1; a non-integer is rejected by axum's `Query` with 400). Calls `latest_plays(user, { podcast_id: None, in_progress_only: true, limit })`. Response `200`:

```json
[{
  "podcast_url": "https://example.com/feed.xml",
  "podcast_title": "Example",
  "podcast_logo_url": "https://example.com/logo.png",
  "episode_url": "https://example.com/ep1.mp3",
  "episode_title": "Episode 1",
  "mimetype": "audio/mpeg",
  "position": 1234,
  "total": 3600,
  "timestamp": "2026-10-12T10:00:00",
  "device": "phone"
}]
```

`podcast_title`, `podcast_logo_url`, `episode_title`, `mimetype`, `total`, `device` may be `null`. `timestamp` format `%Y-%m-%dT%H:%M:%S`.

### `GET /api/2/me/positions?podcast=<url>` (authenticated, rpodder-only)

`podcast` required (missing → 400). The handler normalizes it (`normalize_url`) and resolves it with `PodcastRepo::find_by_url`; unknown podcast → `200 []`. Calls `latest_plays(user, { podcast_id: Some(id), in_progress_only: false, limit: None })` (bounded by the podcast's episode count) and returns the same item shape as in-progress.

Both handlers live in `routes/episodes.rs`, take `State`, `Extension<AuthUser>` and a `Query`, and have no path parameters; the query is always filtered by `AuthUser.id`.

### Existing endpoints used (unchanged)

- `POST /api/2/episodes/{u}.json`, body `{"actions":[…]}`.
- `GET/POST /api/2/settings/{u}/account.json`, key `rpodder_web_sync_positions` (boolean, missing = `true`).
- `GET /api/2/devices/{u}.json`; `POST /api/2/devices/{u}/rpodder-web.json` `{"caption":"Web player","type":"other"}`.
- `GET/POST /api/2/sync-devices/{u}.json`; `GET /subscriptions/{u}/{device}.json`; `POST /api/2/subscriptions/{u}/{device}.json` (the server normalizes `add`/`remove` URLs and resolves removals by podcast, `routes/subscriptions.rs:335-369`, so the canonical URL is enough to unsubscribe on every device).

All new frontend helpers build paths with `encodeURIComponent` on every `{u}`, `{device}` and query value (device ids created through subscription uploads are not validated and may contain spaces, `#` or `?`).

### `lib/playerLogic.ts` (pure, no imports, erasable TypeScript only: no enums, namespaces or parameter properties, so `node --test` can strip types)

```ts
export function normalizeUrl(raw: string): string;
  // Mirror of rpodder_core::url::normalize_url: trim; if `new URL()` throws return trimmed;
  // drop an empty fragment; strip all trailing "/" from a path longer than "/"; return url.toString().
export function isSafeMediaUrl(url: string | null | undefined): boolean;   // protocol http: or https: only
export function isPlayable(item: { url?: string | null; media?: boolean | null; mimetype?: string | null }): boolean;
  // isSafeMediaUrl(url) && media !== false && !(mimetype && /^(text\/|image\/)/.test(mimetype))
export function chooseSource(pageProtocol: string, url: string): { src: string; upgraded: boolean };
export interface KnownPosition { position: number; total: number | null; timestamp: string; device: string | null }
export function pickResume(remote: KnownPosition | null | undefined, local: KnownPosition | null | undefined): KnownPosition | null;
  // greater timestamp wins (ISO strings of the same format compare lexically); either may be missing
export function resumeAt(pos: KnownPosition | null, duration: number | null): number;
  // 0 if pos null, or (pos.total ?? duration) known and pos.position >= that - 30; else pos.position
export function evict<T extends { timestamp: string }>(entries: Record<string, T>, cap: number): Record<string, T>;
export function isoNow(d?: Date): string;   // (d ?? new Date()).toISOString().slice(0, 19)
export function effectiveTotal(audioDuration: number, feedDuration: number | null | undefined): number | null;
  // floor(audioDuration) if finite and > 0, else feedDuration if > 0, else null
export interface UploadGuard { sync: boolean; remoteKnown: boolean; position: number; total: number | null; lastUploaded: number | null }
export function shouldUpload(g: UploadGuard): boolean;
  // sync && remoteKnown && total !== null && position >= 1 && position !== lastUploaded
export interface PlayActionInput { podcastUrl: string; episodeUrl: string; device: string; started: number; position: number; total: number; now: Date }
export function buildPlayAction(i: PlayActionInput): { podcast: string; episode: string; device: string; action: 'play'; started: number; position: number; total: number; timestamp: string };
  // started = min(i.started, i.position) so started <= position always holds
```

`playerLogic.test.ts` imports `./playerLogic.ts` (explicit extension, required by Node; the file is not part of the Vite bundle). Cases: normalization mirrors `crates/rpodder-core/src/url.rs` tests (trailing slashes, host case, empty fragment); `javascript:`/`data:` rejected; `isPlayable` with `media: false`, `text/html`, missing mimetype; https upgrade only on https pages; `pickResume` newest-wins with either side missing; `resumeAt` near-end restart; `evict` cap; `effectiveTotal` fallbacks; `shouldUpload` every guard; `buildPlayAction` clamps `started` and formats the timestamp.

### `lib/api.ts` additions

Existing lenient helpers stay for current callers. New strict helpers throw `ApiError { status }` (0 for network errors) on any non-2xx:

```ts
export class ApiError extends Error { status: number; constructor(status: number, message?: string) }
export async function fetchDevices(username: string): Promise<Device[]>;
export async function fetchSubscriptions(username: string, device: string): Promise<string[]>;
export async function fetchSyncStatus(username: string): Promise<SyncStatus>;
export async function fetchAccountSettings(username: string): Promise<Record<string, unknown>>;
export async function postAccountSettings(username: string, set: Record<string, unknown>): Promise<void>;
export async function postSubscriptionChanges(username: string, device: string, add: string[], remove: string[]): Promise<void>;
export async function postSyncStatus(username: string, synchronize: (string[] | { devices: string[]; name?: string })[], stopSynchronize?: string[]): Promise<void>;
export async function postDevice(username: string, deviceId: string, caption: string, type: string): Promise<void>;
export interface EpisodeActionOut { podcast: string; episode: string; device?: string; action: string; timestamp: string; started?: number; position?: number; total?: number }
export async function uploadEpisodeActions(username: string, actions: EpisodeActionOut[], opts?: { keepalive?: boolean }): Promise<void>;
export interface PlayItem { podcast_url: string; podcast_title: string | null; podcast_logo_url: string | null; episode_url: string; episode_title: string | null; mimetype: string | null; position: number; total: number | null; timestamp: string; device: string | null }
export async function getInProgress(limit: number): Promise<PlayItem[]>;
export async function getPositions(podcastUrl: string): Promise<PlayItem[]>;
```

`EpisodeListItem` gains `url?: string; media?: boolean`.

### Session end (`lib/auth.svelte.ts`)

```ts
export function onSessionEnd(cb: () => void): void;   // registered listeners, called by sessionExpired() and auth.logout()
export function sessionExpired(): void;               // listeners, auth.logout(), goto('/login?expired=1')
export function isAuthError(e: unknown): boolean;     // e instanceof ApiError && (e.status === 401 || e.status === 403)
```

Every caller of a strict helper calls `sessionExpired()` when `isAuthError(e)`. A 403 is treated like 401 here because every new call targets the logged-in user's own resources, so a 403 means the cookie belongs to a different user than `auth.username`. `player.svelte.ts` registers `onSessionEnd(() => player.reset())` at module load (player imports auth; auth does not import player, so there is no cycle). `/login` shows "Your session expired, please log in again." when `?expired=1`.

### `lib/webDevice.ts` (does not import the player)

```ts
export const WEB_DEVICE_ID = 'rpodder-web';
export const WEB_DEVICE_CAPTION = 'Web player';
export async function ensureWebDevice(username: string): Promise<void>;
  // fetchDevices; if no WEB_DEVICE_ID device, or its caption === WEB_DEVICE_ID (auto-created by an action upload,
  // routes/helpers.rs:46-66), postDevice(u, WEB_DEVICE_ID, WEB_DEVICE_CAPTION, 'other').
  // Memoized per username (Map<string, Promise<void>>), entry deleted on rejection.
export async function webGroup(username: string): Promise<SyncGroupInfo | null>;
export async function subscribe(username: string, url: string): Promise<void>;
  // ensureWebDevice; if webGroup(): postSubscriptionChanges(u, WEB_DEVICE_ID, [url], [])
  // else for each device of fetchDevices(): postSubscriptionChanges(u, d.id, [url], []) sequentially.
export async function devicesWith(username: string, urls: string[]): Promise<Map<string, Device[]>>;
  // fetchDevices, then fetchSubscriptions per device (sequential); keyed by normalizeUrl(url);
  // a device owns a URL when one of its subscriptions normalizes to the same string.
export async function unsubscribe(username: string, devices: Device[], url: string): Promise<void>;
  // postSubscriptionChanges(u, d.id, [], [normalizeUrl(url)]) per device, sequentially; throws on the first failure.
export async function replaceSubscriptions(username: string, pairs: { from: string; to: string }[]): Promise<void>;
  // owners = devicesWith(u, froms) once; per owning device one postSubscriptionChanges(add = its `to`s, remove = its `from`s).
export interface WebSettings { syncPositions: boolean; group: SyncGroupInfo | null; groups: SyncGroupInfo[] }
export async function getWebSettings(username: string): Promise<WebSettings>;
export async function setSyncPositions(username: string, on: boolean): Promise<void>;
export async function joinSyncGroup(username: string, group: SyncGroupInfo | null): Promise<void>;
  // ensureWebDevice. null → postSyncStatus(u, [], [WEB_DEVICE_ID]).
  // group → postSyncStatus(u, [{ devices: [...group.devices.filter(d => d !== WEB_DEVICE_ID), WEB_DEVICE_ID], name: group.name || undefined }]).
export function groupLabel(g: SyncGroupInfo): string;
  // `${g.name || 'Unnamed group'} — ${g.devices.filter(d => d !== WEB_DEVICE_ID).join(', ')}`
```

Backend facts relied on (`routes/sync.rs` `update_sync_status` ~155-210 and `merge_subscriptions` below it): a `synchronize` entry creates a new group with the listed devices and merges their subscriptions both ways (not undoable); the previous group keeps its remaining devices; `get_sync_status` lists only groups with more than one device; bare-array groups have `name = ""`. Group ids change on every join, so callers re-fetch `getWebSettings`.

### `lib/positions.ts`

```ts
export interface LocalEntry extends KnownPosition {   // timestamp = isoNow() of this local write; device 'rpodder-web'
  podcastUrl: string; podcastTitle: string; episodeTitle: string; imageUrl: string | null;
}
// key `rpodder_positions_${username.toLowerCase()}` → { [normalizeUrl(episodeUrl)]: LocalEntry }, evict(…, 200) on write.
// Every access in try/catch; storage errors make reads return null/[] and writes no-ops.
export function localGet(username: string, episodeUrl: string): LocalEntry | null;
export function localSet(username: string, episodeUrl: string, entry: LocalEntry): void;
export function localClear(username: string): void;
export function localInProgress(username: string, limit: number): PlayItem[];
  // position > 0 and (total null or position < total - 30), newest first.
export type PositionsResult = { positions: Map<string, KnownPosition>; remoteKnown: boolean };
export async function podcastPositions(username: string, podcastUrl: string, sync: boolean): Promise<PositionsResult>;
  // locals = entries with normalizeUrl(entry.podcastUrl) === normalizeUrl(podcastUrl).
  // sync: remote = getPositions(podcastUrl) keyed by normalizeUrl(episode_url); merged per key with pickResume(remote, local);
  //       remoteKnown = true. On error: isAuthError → sessionExpired() and rethrow; otherwise positions = locals, remoteKnown = false.
  // !sync: positions = locals, remoteKnown = false.
```

### `lib/player.svelte.ts`

```ts
export interface PlayerEpisode {
  url: string; title: string; podcastUrl: string; podcastTitle: string;
  imageUrl?: string | null; duration?: number | null;
}
export const REPORT_INTERVAL_MS = 300_000;   // overridable for tests: localStorage 'rpodder_player_report_interval_ms' (min 5000)
export const LOCAL_SAVE_MS = 5_000;
export const SEEK_SETTLE_MS = 3_000;
export const REMOTE_STALE_MS = 120_000;
class Player {
  // $state:
  episode: PlayerEpisode | null = null;
  playing = false; position = 0; duration = 0; rate = 1;
  error: 'secure-version-failed' | 'load' | null = null;
  recent: Record<string, KnownPosition> = {};   // last local saves this page load, keyed by normalizeUrl
  async play(username: string, ep: PlayerEpisode): Promise<void>;
  toggle(): void;                     // resume goes through the same remote refresh as play() when stale
  seek(seconds: number): void;
  skip(delta: number): void;
  setRate(r: number): void;
  async setSync(username: string, on: boolean): Promise<void>;
  async close(opts?: { timeoutMs?: number }): Promise<void>;   // final upload raced against timeoutMs (default 3000), then reset()
  reset(): void;                      // no upload: pause, clear src and episode, close segment, gen++, forget username
}
export const player = new Player();
```

State kept inside the player (not `$state`): `username`, `sync`, `remoteKnown`, `lastRemoteRead` (ms), `segment: { started: number } | null`, `lastUploaded: number | null`, `seekBurstUntil` (ms), `gen`.

Behaviour:

- One `HTMLAudioElement`, created on the first `play()`, appended once to `document.body`, `preload = "metadata"`, no `crossOrigin`. `src` is only ever set from `chooseSource()` of a URL that passed `isSafeMediaUrl`.
- Concurrency: `play()`, `toggle()`-resume, `setSync()`, `reset()` and `close()` increment `gen`; async work captures it and returns early after each `await` if it changed or if `auth.username !== this.username`.
- `play(username, ep)`:
  1. `!isSafeMediaUrl(ep.url)` → `error = 'load'`, return. If `username !== this.username`, `reset()` first and remember `username`. If `normalizeUrl(ep.url)` equals the loaded episode's → `toggle()`, return.
  2. If an episode is loaded: end its segment (upload).
  3. Settings: `getWebSettings` (cached 60 s per username; cache cleared by `setSync`, settings-page saves and `reset()`). `isAuthError` → `sessionExpired()`, return. Other error → `sync = false`.
  4. Positions: `podcastPositions(username, ep.podcastUrl, sync)` → `positions`, `remoteKnown`; `lastRemoteRead = now` when `remoteKnown`. Auth errors rethrown → return. Start at `resumeAt(positions.get(normalizeUrl(ep.url)) ?? null, ep.duration ?? null)`.
  5. `chooseSource(location.protocol, ep.url)`. On `loadedmetadata` set `currentTime`, then `audio.play()` (a rejected promise from autoplay policy leaves it paused without error). On `error`: `upgraded ? 'secure-version-failed' : 'load'`.
- `toggle()` while paused: if `sync && (now - lastRemoteRead > REMOTE_STALE_MS || !remoteKnown)`, re-read `getPositions` for the episode's podcast first; if the remote position for this episode is newer than the player's last local save (`pickResume`), seek there; then play. If the read fails, keep `remoteKnown = false` (no uploads) and play from the current position.
- `setSync(username, on)`: `sync = on`; when turning on during a loaded episode, re-read remote as in `toggle()`; uploads only start once `remoteKnown` is true.
- `navigator.mediaSession` (when present): metadata, `play`, `pause`, `seekbackward` (15), `seekforward` (30), `seekto`.
- Multi-tab: `BroadcastChannel('rpodder-player')` (when present); on each `playing` event post `{ type: 'playing', tab: TAB_ID }`; another tab receiving it pauses (ending its segment).
- `$state` mirrors from `play`, `pause`, `timeupdate`, `durationchange`, `ended`.

Position reporting (one gpodder `play` action per upload, fresh timestamp each time; a fresh timestamp matches how AntennaPod stamps plays at pause time and keeps newest-timestamp-wins correct for clients that only apply newer remote actions):

- Segment: opened on `playing` when none is open and no seek burst is active, `started = floor(currentTime)`.
- Upload = `buildPlayAction({ podcastUrl, episodeUrl, device: 'rpodder-web', started: segment.started, position: floor(currentTime), total, now })` with `total = effectiveTotal(audio.duration, ep.duration)`, sent only if `shouldUpload({ sync, remoteKnown, position, total, lastUploaded })`. Without a known total nothing is uploaded (AntennaPod expects `total`); local saves still happen.
- Triggers: `pause` and `ended` (position = duration) end the segment; switching episodes and `close()` end it; `pagehide` ends it; `visibilitychange` → `hidden` uploads without ending it; every `REPORT_INTERVAL_MS` while playing.
- Seek/skip: on the first `seek()`/`skip()` of a burst while a segment is open, upload the segment as it stood before the seek (`position` = pre-seek time) and close it; set `seekBurstUntil = now + SEEK_SETTLE_MS` (extended by every further seek); when the burst ends and the element is playing, open a new segment at the settled position. Seeks while paused upload nothing (no open segment); the local save records the new position.
- `ensureWebDevice` before the first upload of a page load. Uploads use `uploadEpisodeActions(…, { keepalive: true })`. `isAuthError` → `sessionExpired()`. Other failures → `console.warn` (the next upload carries a newer position).
- Local: every `LOCAL_SAVE_MS` while playing and at each trigger, `localSet` and `recent[normalizeUrl(url)] = …` (always, regardless of `sync`).
- Growth: ≤ 12 uploads per hour of continuous listening, plus one per pause, seek burst or switch; each is a new `episode_actions` row and `/history` entry. Accepted trade-off for correct newest-wins semantics.

### Components

`PlayButton.svelte` props `{ episode: PlayerEpisode; playable: boolean; progress?: KnownPosition | null }`. Renders nothing if `!auth.loggedIn`. Shown position = live `player.position`/`player.duration` for the loaded episode, else `pickResume(progress, player.recent[normalizeUrl(episode.url)])`. Pause icon when loaded and playing. Click → `player.play(auth.username, episode)`. Disabled with `title="No audio file"` when `!playable`. With a total > 0: 2px bar and "N min left".

`MiniPlayer.svelte` (no props) renders when `auth.loggedIn && player.episode`: `fixed inset-x-0 bottom-0 z-40 bg-surface border-t border-border`; logo, title and podcast title (link `/discover/podcast?url=…`), -15 s, play/pause, +30 s, range (`0..duration`, seek on `change`), elapsed / remaining, speed (1 → 1.25 → 1.5 → 2 → 1), close (`player.close()`). Errors: `secure-version-failed` → "Couldn't load a secure (https) version of this episode; it may only be available over plain HTTP."; `load` → "Can't play this episode."; both followed by an "Open episode" link (`target="_blank" rel="noopener noreferrer"`) rendered only when `isSafeMediaUrl(episode.url)`. Below `md`: two rows.

`ContinueListening.svelte` (no props): `$effect` + `browser` guard, once per mount, when logged in: `sync` from `getWebSettings` (auth error → `sessionExpired()`; other error → false); items = `sync ? getInProgress(12)` (auth error → `sessionExpired()`; other error → `localInProgress`) `: localInProgress(username, 12)`. Renders nothing when empty. Otherwise heading "Continue listening" and a 1/2/3-column grid of cards: logo or placeholder, episode title (fallback: decoded last path segment of `episode_url`), podcast title (fallback: host of `podcast_url`), "N min left", and a `PlayButton` with `episode = { url: item.episode_url, title, podcastUrl: item.podcast_url, podcastTitle, imageUrl: item.podcast_logo_url, duration: item.total }`, `playable = isPlayable({ url: item.episode_url, mimetype: item.mimetype })`, `progress = item`. Placed at the top of `/subscriptions`.

Mini-player sketch (desktop):

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ [logo] Episode title · Podcast   ⟲15  ▶/❚❚  30⟳   12:03 ━━━━●──── -41:10  1.5×  ✕ │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### Page changes

- `+layout.svelte`: `<MiniPlayer />` after the footer; `pb-24` on the outer `min-h-screen` wrapper when `player.episode` is set. `handleLogout`: `sync` = cached web settings (if unknown, fetch with a 2 s timeout; failure → treated as unknown); `await player.close({ timeoutMs: 3000 })`; if `sync === true`, `localClear(auth.username)` (D7); then `apiLogout`, `auth.logout()` (which also fires session-end listeners), `goto('/login')`.
- `login/+page.svelte`: "Your session expired, please log in again." when `?expired=1`.
- `discover/podcast/+page.svelte`: when logged in, after episodes load: `sync` from `getWebSettings`, `positions = (await podcastPositions(u, podcastUrl, sync)).positions`. Rows get `PlayButton` with `episode = { url: ep.url, title: ep.title, podcastUrl, podcastTitle: data.podcast.title, imageUrl: data.podcast.logo_url, duration: ep.duration }`, `playable = isPlayable(ep)`, `progress = positions.get(normalizeUrl(ep.url))`; `podcastUrl` = the page's `?url=` value. Subscription state: `(await getAllSubscriptions(u)).some(s => normalizeUrl(s) === normalizeUrl(podcastUrl))`. Subscribe → `subscribe(u, podcastUrl)`, then re-read. Unsubscribe → `owners = (await devicesWith(u, [podcastUrl])).get(normalizeUrl(podcastUrl)) ?? []`; empty → "Not subscribed on any device." and re-read; else `confirm("Unsubscribe from <title> on: <captions joined by ', '>?")` → `unsubscribe(u, owners, podcastUrl)`, then re-read. Errors → inline message next to the button; auth errors → `sessionExpired()`.
- `discover/+page.svelte`, `discover/trending/+page.svelte`, `discover/tag/[tag]/+page.svelte`: subscribe via `subscribe()`; on error the existing failure UI; auth errors → `sessionExpired()`.
- `subscriptions/+page.svelte`: remove the `deviceId` selection; `<ContinueListening />` at the top; row unsubscribe uses the same confirm flow; `upgradeToHttps(upgrade)` → `replaceSubscriptions(u, [{ from: upgrade.http_url, to: upgrade.https_url }])`; `upgradeAll()` → one `replaceSubscriptions` with all pairs; after any change re-read `getAllSubscriptions`.
- `settings/+page.svelte`, card "Web player":

```
Web player
[x] Sync listening positions with my other devices
    When off, positions stay in this browser only.
Sync group: [ None ▾ | Home — phone, tablet | Unnamed group — laptop, desktop ]  [Save]
    The web player shares subscriptions with the devices in this group.
    (when the selection differs from the current group and is not None:)
    The web player's current subscriptions will be added to every device in "<label>", and theirs to the web player. This can't be undone.
    (when the current group has exactly two devices and the selection differs from it:)
    "<current label>" will be dissolved: <other device> will no longer sync with anything.
```

  Checkbox → `setSyncPositions(u, on)` then `player.setSync(u, on)`. Select + Save → `joinSyncGroup`. Then `getWebSettings` re-fetched. On error the control reverts and "Couldn't save, try again." is shown; auth errors → `sessionExpired()`.

## UX flow

1. Logged-in user opens `/discover/podcast?url=X`; rows show play buttons and progress.
2. Play: mini-player appears, seeks to the newest known position (remote vs local), plays.
3. Client-side navigation: playback continues.
4. Uploads on pause, seek, tab hide, every 5 min, episode switch, close (only when sync is on, the remote position was read and the duration is known); local save every 5 s.
5. Phone's next `GET /api/2/episodes?since=…` receives the newest action and resumes there.
6. `/subscriptions` "Continue listening" resumes from cards.
7. `/settings`: toggle sync, pick a group (warnings shown).
8. Subscribe (D3) / unsubscribe with confirm (D4) / HTTPS upgrades on every owning device.
9. Logout: final upload (max 3 s); local positions cleared if sync is on (D7); session closed.
10. Session expired or a different user logged in: the player stops without uploading, redirect to `/login?expired=1`.

## Auth & ownership

- `/api/2/me/in-progress` and `/api/2/me/positions`: `authenticated` group (`require_auth_layer`, session or Basic; OAuth users hold a session); filter by `AuthUser.id`; no path or id parameters. A podcast URL in `positions` only selects among the caller's own actions.
- Writes reuse endpoints that check `{username}` against `AuthUser` case-insensitively.
- `episodes.json` stays public.

## gpodder compatibility

- `episodes.json`: additive `url` and `media`.
- `rpodder-web` is an ordinary `other` device sending standard `play` actions with `started ≤ position`, a known `total` (uploads are skipped when it is unknown), and a fresh ISO timestamp; other clients see `device: "rpodder-web"` and a `Web player` device.
- No change to existing parsing, status codes or timestamp formats.

## Data & migrations

No migration, no new table. Two read-only repo methods (`primary_urls`, `latest_plays`) in both DBs.

## Privacy

- In-progress and positions return only the caller's own actions.
- `episodes.json` returns media URLs only for a feed URL the requester supplies; anyone holding a private feed URL can already read the feed and its enclosures from its host. Private feeds stay out of public listings.
- The player requests `GET /api/2/me/positions?podcast=<url>`; for private feeds the token appears in that request URI (and in reverse-proxy access logs). Same exposure as the existing `/api/2/episodes/{u}.json?podcast=` filter; noted.
- Private feeds are skipped by the background updater (`feed_updater.rs:77-78`), so their episode lists may be stale.
- localStorage positions (URLs, titles) are cleared on logout when sync is on; with sync off they are kept as the only copy (D7), keyed by the lower-cased username.
- Episode URLs from feeds are untrusted: only `http:`/`https:` URLs are used as audio sources or links (`isSafeMediaUrl`).

## Build variants & footprint

- Backend changes are not behind `web-ui`; `--no-default-features` builds them.
- In-progress: one query per `/subscriptions` view over the caller's `play` rows (`idx_epactions_user_episode`). Positions: one query per podcast page view and per play/stale resume, bounded by that podcast's episodes.
- `primary_urls`: one extra query (≤ 100 ids) per episodes page.
- Player: settings cached 60 s; ≤ 12 uploads/hour while playing plus one per pause/seek burst/switch, each a new `episode_actions` row (and one `/history` entry) — accepted trade-off for newest-wins semantics.
- `replaceSubscriptions`: one subscription GET per device plus one POST per owning device, regardless of the number of URLs.
- No new Rust or npm dependencies.

## Failure modes

| Failure | Result |
|---|---|
| Enclosure 404 / unreachable / unsupported | "Can't play this episode." + "Open episode". |
| `http://` enclosure on `https://`, https variant fails | "Couldn't load a secure (https) version…" + "Open episode". |
| Non-`http(s)` URL (e.g. `javascript:`) | Not playable; never used as `src` or `href`. |
| `<link>` page (`media: false`), `text/*` or `image/*` mimetype, or missing `url` | Play disabled ("No audio file"). |
| Autoplay blocked | Paused at the resume position. |
| Settings fetch fails at `play()` | Sync treated as off for this playback: local positions, no uploads. |
| Position lookup fails at `play()` or at a stale resume | Resume from local/current; no uploads until a later remote read succeeds. |
| Paused tab resumed after the phone played on | Remote re-read (stale > 2 min) and seek to the newer remote position before playing. |
| Duration unknown (no metadata, no feed duration) | Local saves only; no uploads. |
| User turns sync off during playback | `player.setSync(false)`: no further uploads. |
| User turns sync on during playback | Remote re-read first; uploads start only after it succeeds. |
| Upload fails (offline/5xx) | `console.warn`; local save continues; next upload carries a newer position; on next play the newer local position wins over the older remote one. |
| Session expired or cookie of another user (401/403 in these flows) | Player reset without upload; redirect to `/login?expired=1` with a message. |
| Another user logs in on the same tab | `play()` sees a different username and resets the player first. |
| Logout on a stalled network | Final upload abandoned after 3 s; logout proceeds. |
| `in-progress` fails | Local list; empty → section hidden. |
| Subscribe / unsubscribe / group change fails | Inline error; state re-read from the server; `ensureWebDevice` memo cleared. |
| Unsubscribe when no device has the URL | "Not subscribed on any device."; state re-read. |
| Two tabs play | Newest plays; the other pauses and reports. |
| Rapid clicks on different episodes | Generation token: only the last `play()` takes effect. |
| localStorage unavailable | Player works; no local resume / local list. |
| Logged-out visitor | No play buttons, no mini-player. |

## Verification criteria

Rust (`crates/rpodder-server/src/tests/web_player.rs`, in-memory SQLite, real `api_router`; podcasts created through repos with non-empty `title` and `description` so `podcast_episodes` does not fetch; extra `episode_urls` rows inserted with raw `sqlx::query`):

- `episodes_list_includes_primary_media_url`: ep A with mimetype `audio/mpeg`, URLs order 0 `…/a.mp3`, order 1 `…/a-alt.mp3`; ep B without mimetype whose `link` equals its only URL `…/b.html` → A `url …/a.mp3, media true`; B `url …/b.html, media false`.
- `in_progress_returns_latest_unfinished_play_per_episode` (timestamps relative to now): ep1 play 100/3600 at -2h (phone), 200/3600 at -1h (tablet), plus a `download` at -30m → included, position 200, device "tablet"; ep2 play 3590/3600 → excluded; ep3 play 0 → excluded; ep4 only `download` → excluded; ep5 play 100, no total, episode duration 120 → excluded; ep6 play 50, no total, no duration → included, `total: null`; ep7 play 500/3600 at -3h then `new` at -2h → excluded; order ep1 before ep6 (ep6's play is older).
- `in_progress_titles_null_when_equal_to_url` and `mimetype` passthrough.
- `in_progress_is_scoped_and_authenticated`: bob's actions absent from alice's list; no cookie → 401.
- `in_progress_limit_is_clamped`: 60 in-progress episodes → default 20, `limit=0` → 1, `limit=-5` → 1, `limit=500` → 50.
- `positions_returns_latest_play_per_episode_of_podcast`: plays on two podcasts; `?podcast=<p1 with trailing slash>` (normalized) returns only p1's episodes, each its latest play including finished ones; unknown podcast → `[]`; missing `podcast` → 400.

Frontend unit tests: `cd web && node --test src/lib/playerLogic.test.ts` (cases listed under `playerLogic.ts`).

Docker E2E `tests/docker/tests/09_web_player.sh` (SQLite and PostgreSQL). Like `07_feed_update.sh`, it `skip`s when the mock server is unreachable. It runs as the shared test user, so in-progress assertions filter with jq on `podcast_url == "${MOCK_FEED_HOST}/feed/player"`:

- Subscribe to `${MOCK_FEED_HOST}/feed/player`, trigger `POST /api/admin/feeds/update/single?url=…`, `wait_until` episodes appear.
- `episodes.json`: `Player ep1` and `Player ep2` have `url`, `media: true` and an audio `mimetype`; `Player ep3` has a `url` and `media: false`.
- Upload plays from device `e2e-phone`, using feed episode URLs where they exist and explicit extra URLs under the same podcast otherwise (`MOCK_PUBLIC=http://host.docker.internal:8888`): ep1 play 100/180 at `2026-01-01T10:00:00` then 120/180 at `2026-01-01T11:00:00` → included with position 120 and title "Player ep1"; `${MOCK_PUBLIC}/media/case-finished.mp3` play 175/180 → excluded; `${MOCK_PUBLIC}/media/case-zero.mp3` play 0/180 → excluded; `${MOCK_PUBLIC}/media/case-download.mp3` download only → excluded; `${MOCK_PUBLIC}/media/case-reset.mp3` play 60/180 at `2026-01-01T10:00:00` then `new` at `2026-01-01T12:00:00` → excluded. The filtered list equals exactly `[ep1]`.
- `/api/2/me/positions?podcast=${MOCK_FEED_HOST}/feed/player` returns ep1 (120), case-finished (175) and case-zero (0), case-reset (60), not case-download.
- `limit=1` returns exactly one item. `/api/2/devices` lists `e2e-phone`.
- Not exercised on PostgreSQL: `primary_urls` ordering with several URLs per episode (feeds only produce one URL per episode; covered on SQLite).

Mock fixture (`tests/docker/mock-rss.py`): `ThreadingHTTPServer`, with a `threading.Lock` around every read/write of the shared `state` dict (fetch counters asserted by `07`). `/feed/player` (title "Player Feed"): `Player ep1` enclosure `${PUBLIC}/media/tone.wav` (`audio/wav`, `<itunes:duration>180</itunes:duration>`); `Player ep2` enclosure `${PUBLIC}/media/missing.mp3` (`audio/mpeg`); `Player ep3` no enclosure, `<link>${PUBLIC}/page/ep3</link>` (no extension, the case the server-side `media` flag exists for). `${PUBLIC}` = env `MOCK_PUBLIC_URL`, default `http://host.docker.internal:8888`. `/media/tone.wav`: 180 s, 8 kHz, mono, 8-bit PCM WAV of a 440 Hz tone, generated at startup with `wave`, served with `Accept-Ranges: bytes` and `206 Partial Content` + `Content-Range` for `Range: bytes=a-b|a-`. `/media/missing.mp3` → 404.

Frontend build: `cd web && bun run build` (via the `oven/bun:1` image; bun is not installed on the host).

Browser walkthrough (Playwright via the `webapp-testing` skill; the Claude in Chrome MCP is not available here), against the embedded build:

- Setup: host `MOCK_PUBLIC_URL=http://localhost:8888 python3 tests/docker/mock-rss.py 8888`; `web/dist` built; backend in `rust:bookworm` with `-p 3006:3006`, `RPODDER_PORT=3006`, `RPODDER_DATABASE_URL=sqlite:///tmp/walk.db`, `cargo run -p rpodder-server -- serve`; `rpodder user create alice pass --admin`; devices `phone`, `tablet` (grouped "Home" via `POST /api/2/sync-devices/alice.json` `{"synchronize":[{"devices":["phone","tablet"],"name":"Home"}]}`) and `laptop` (ungrouped) via `POST /api/2/devices/alice/{id}.json`. Backend feed URL `http://host.docker.internal:8888/feed/player`, fetched via the admin single-feed update. Browser at `http://localhost:3006`; before step 2 set `localStorage.rpodder_player_report_interval_ms = 5000`. Between pages, navigate with in-app links.
- Steps (expected):
  1. Login, open the podcast: ep1 and ep2 enabled, ep3 disabled.
  2. Play ep1: mini-player visible; `audio.currentTime` grows over 3 s.
  3. In-app navigate to `/devices` and back: the same audio element is still playing.
  4. Keep playing ~12 s: at least one periodic POST to `/api/2/episodes/alice.json` with `device: "rpodder-web"`; pause at ~15 s: another POST with position ≈ 15 and `started ≤ position`; `/api/2/me/in-progress` lists ep1; `/api/2/devices/alice.json` lists `rpodder-web` with caption "Web player".
  5. Play, then seek to 60 via the range input: one POST with position ≈ the pre-seek time; after 4 s of playing and a pause: a POST with position ≈ 64 and `started` ≈ 60.
  6. Reload, play ep1: starts at ≥ 63 s. Pause.
  7. With ep1 paused, upload via API a `play` for ep1 position 120 from `phone` with timestamp = now, then reload the podcast page: progress shows 120 and play starts at ≈ 120.
  8. Play ep2: "Can't play this episode." and an "Open episode" link.
  9. While ep1 plays, stub `document.visibilityState` to `hidden` and dispatch `visibilitychange`: a POST is observed.
  10. Settings: sync off; play ep1, pause: no POST; "Continue listening" on `/subscriptions` shows ep1 from local.
  11. Subscribe semantics: `rpodder-web` in no group → subscribe from the podcast page → `phone`, `tablet`, `laptop`, `rpodder-web` all list the feed; unsubscribe → confirm lists the four captions → none list it. Settings: join "Home" (warning shown) → `/api/2/sync-devices` group contains phone, tablet, rpodder-web; subscribe → phone, tablet, rpodder-web list it, laptop does not.
  12. Settings: sync on again. Two tabs: play in A, then in B → A pauses.
  13. Logout while playing: a final POST precedes the logout request; `localStorage.rpodder_positions_alice` is gone (sync on).
  14. Login, play ep1, clear the session cookie through the Playwright context, press play on ep1 again after 2 s pause → redirected to `/login?expired=1`, audio paused.
- Not covered by automation (stated): the real mixed-content path (dev runs on http; covered by `chooseSource` unit tests), partial unsubscribe failure, localStorage-disabled mode, the >2 min stale-resume timer (logic covered by `pickResume` tests).

## Out-of-scope follow-ups

Open findings accepted at the review cap (2026-10-12), to fix next:

- Pause + resume while the first-play settings read is still pending (slow server, > 3 s) leaves `#sync` false for that episode; add a `#syncKnown` flag so resume re-reads settings.
- Turning sync off while a slow first-play lookup is in flight can be overwritten by the late (old) settings value; guard the assignment with a `setSync` epoch counter.
- The settings checkbox stays disabled until `player.setSync`'s remote read finishes; don't await it from the settings page.
- A second tab still playing for the same user can recreate `rpodder_positions_<user>` after logout cleared it; skip local writes unless `rpodder_user` still matches, and clear after `auth.logout()`.

Pre-existing bugs found while building this:

- `EpisodeRepo::get_or_create_for_url` is not atomic: a concurrent on-demand fetch (`episodes.json`) and an admin/feed-updater fetch can leave orphan episode rows titled with their URL and without an `episode_urls` row; they show up in episode lists.
- The auth middleware prefers the session cookie over Basic credentials, so `/api/2/auth/{user}/login.json` for a different user fails while another user's cookie is present (the login page says "Invalid username or password").

- Server-side proxy for http-only enclosures.
- `total` count in `episodes.json`.
- Optional per-browser device; adopting the legacy `web` device automatically.
- Queue / playlists, chapters, offline download; mark played/unplayed.
- Orphaned or shrunken sync-group rows left by `create_group`.
- Refresh of private feeds' episode lists.
- Frontend test runner in CI (Vitest/Playwright).
