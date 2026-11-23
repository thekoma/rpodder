# Design: Nextcloud Login Flow v2 and the gpoddersync API

Status: implemented, design revision 3 (elephant/goldfish `/eg-new-feature`; revisions 1 and 2 each went through a critic and a readiness pass, revision 3 was approved by the user without a third round), branch `feat/nextcloud-login-flow`.

## Why

AntennaPod has two sync providers: gpodder.net and "Nextcloud gPodder Sync". Podcini, an AntennaPod fork, only has the Nextcloud one (its gpodder.net provider is commented out). The Nextcloud provider logs in through Nextcloud's [Login Flow v2](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/LoginFlow/index.html): the app opens a login page in the browser, the user signs in and grants access there, and the app receives an app password. Whatever login the page offers, SSO included, works without the app knowing about it.

rpodder speaks only the gpodder.net API, so:

- Podcini cannot sync with rpodder at all.
- AntennaPod users on an SSO instance must set a separate rpodder password and type it into the app, because the gpodder.net provider only does HTTP Basic with username and password.
- People moving off Nextcloud's gpoddersync app have to switch provider in every app.

Decisions (conversation, 2026-11-23):

- D1 Implement the four gpoddersync endpoints AntennaPod and Podcini call, on top of the existing subscription and episode-action logic. No new sync semantics.
- D2 Add app passwords: per-client credentials, accepted wherever HTTP Basic is accepted except credential and admin management, listed and revocable in Settings, and creatable by hand (so gpodder.net clients of SSO users get a credential that isn't the account password).
- D3 App passwords survive the owner changing their own password, as in Nextcloud; the change still logs out every other session, those created from app passwords included, and clients log in again with their app password. Recovery revokes them: completing a password reset (`confirm_password_reset`) and an admin setting the password (`admin_set_password`) delete all of the user's app passwords, their sessions and their approved login flows. Deactivation blocks them while it lasts. Each can be revoked in Settings.
- D4 gpoddersync has no devices, so every gpoddersync client of a user shares one rpodder device, uid `gpoddersync`, caption `Nextcloud sync`. It can join a sync group like any device.
- D5 An app password can't create more credentials: callers authenticated by an app password, or by a session created from one, are refused on app-password management, login-flow lookup and answers, password change and `/api/admin/*`. Creating an app password by hand requires the current password, or an SSO session, exactly like `change_password`.

## Scope

In:

1. `POST /index.php/login/v2` and `POST /index.php/login/v2/poll` (public).
2. Browser page `/login/flow#<login token>` where a signed-in user grants or denies access.
3. App passwords: table, repo trait, HTTP Basic support, auth-method tracking, `/api/2/me/app-passwords` endpoints, Settings section, revocation on recovery.
4. `GET /index.php/apps/gpoddersync/subscriptions`, `POST …/subscription_change/create`, `GET …/episode_action`, `POST …/episode_action/create` (authenticated).
5. Returning to the flow page after a login (password, SSO, or after the session expired), without the token ever entering a URL the server sees.
6. Unit, router, repo and Docker E2E tests; API, client, admin and architecture docs.

Out:

- Any other Nextcloud API (OCS capabilities, WebDAV, `status.php`). AntennaPod and Podcini call only the six routes above.
- Login Flow v1 and the `nc://` URL scheme.
- Grant-time scoping of app passwords beyond D5.
- Showing the requester's IP on the grant page: rpodder has no trusted-proxy setting, so behind a reverse proxy it would show the proxy's address.
- A general "return to the page you were on" after session expiry; only the flow page uses the return target.
- `CHANGELOG.md`: per `AGENTS.md`, edited when cutting a release.

## Surfaces touched

- `migrations/postgresql/006_app_passwords.{up,down}.sql`, `migrations/sqlite/006_app_passwords.{up,down}.sql`, and the `MIGRATIONS` list in `crates/rpodder-db/src/lib.rs`.
- `crates/rpodder-core/src/types.rs` (`AppPassword`, `LoginFlow`), `crates/rpodder-core/src/repo.rs` (`AppPasswordRepo`).
- `crates/rpodder-db/src/postgres.rs`, `crates/rpodder-db/src/sqlite.rs` (impl + SQLite repo tests).
- `crates/rpodder-server/src/middleware/auth.rs`: `AuthMethod`, `AuthVia`, app passwords in Basic, app-session validation, `hash_token`, `APP_SESSION_PREFIX`, admin layer refuses app methods.
- `crates/rpodder-server/src/routes/auth.rs` (`login` mints `app-` sessions for app methods).
- `crates/rpodder-server/src/routes/admin.rs`: `change_password` refuses app methods; `admin_set_password` and `confirm_password_reset` call `delete_all_for_user`.
- `crates/rpodder-server/src/routes/directory.rs:226` (`authenticate` return type).
- New `crates/rpodder-server/src/routes/nextcloud.rs` (login flow + gpoddersync adapters) and `crates/rpodder-server/src/routes/app_passwords.rs` (`/api/2/me/…`), registered in `routes/mod.rs` and `main.rs`.
- `crates/rpodder-server/Cargo.toml`: `sha2 = "0.11"` (already in `Cargo.lock`; same RustCrypto line as `sha1 = "0.11"`).
- Web: `web/src/lib/api.ts`, new `web/src/lib/navigation.ts` + `navigation.test.ts`, `web/src/routes/login/+page.svelte`, new `web/src/routes/login/flow/+page.svelte`, `web/src/routes/settings/+page.svelte`, `web/src/routes/+page.svelte`, `web/src/routes/+layout.svelte` (`authPage`), `web/vite.config.ts` and `web/nginx.conf` (proxy `/index.php`), `web/package.json` (`test` script).
- Tests: `crates/rpodder-server/src/tests/nextcloud_login.rs` (+ `tests/mod.rs`), `tests/docker/tests/12_nextcloud.sh`.
- Docs: `docs/src/api/auth.md`, new `docs/src/api/nextcloud.md`, `docs/src/user-guide/clients.md`, `docs/src/user-guide/web-ui.md`, `docs/src/admin-guide/passwords.md`, `docs/src/architecture/database.md`, `docs/src/architecture/crates.md`, `docs/src/api/overview.md`, `CLAUDE.md` (trait count), `mkdocs.yml` nav.

## Interfaces

### Schema (`006_app_passwords`)

SQLite:

```sql
CREATE TABLE IF NOT EXISTS app_passwords (
    id           TEXT PRIMARY KEY NOT NULL,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    created_at   TEXT NOT NULL,
    last_used_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_app_passwords_user ON app_passwords (user_id);

CREATE TABLE IF NOT EXISTS login_flows (
    id               TEXT PRIMARY KEY NOT NULL,
    poll_token_hash  TEXT NOT NULL UNIQUE,
    login_token_hash TEXT NOT NULL UNIQUE,
    client_name      TEXT NOT NULL,
    user_id          TEXT REFERENCES users(id) ON DELETE CASCADE,
    redeemed_at      TEXT,
    expires_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_login_flows_expires ON login_flows (expires_at);
CREATE INDEX IF NOT EXISTS idx_login_flows_user ON login_flows (user_id);
```

PostgreSQL: same tables with `UUID` ids, `VARCHAR(128)` for `name`/`client_name`, `VARCHAR(64)` for the hashes (not `CHAR`, which sqlx's `text` binds would compare through a cast that skips the index), `TIMESTAMPTZ` for times, `CREATE … IF NOT EXISTS` throughout. Down migrations drop both tables (`DROP TABLE IF EXISTS`). SQLite timestamps are RFC 3339 text written by `to_rfc3339()`, as in the existing tables, so text comparison orders them.

Tokens (app password, poll token, login token) are 64 random `[0-9a-z]` characters from `routes::auth::generate_token`. Only their SHA-256 (lowercase hex) is stored: they are random, so a fast hash is enough, and Basic Auth finds an app password by index instead of trying argon2 against every row.

### Types (`rpodder-core/src/types.rs`)

```rust
pub struct AppPassword { pub id: Uuid, pub user_id: Uuid, pub name: String, pub token_hash: String,
                         pub created_at: DateTime<Utc>, pub last_used_at: Option<DateTime<Utc>> }
pub struct LoginFlow { pub id: Uuid, pub poll_token_hash: String, pub login_token_hash: String,
                       pub client_name: String, pub user_id: Option<Uuid>,
                       pub redeemed_at: Option<DateTime<Utc>>,
                       pub expires_at: DateTime<Utc>, pub created_at: DateTime<Utc> }
```

### `AppPasswordRepo` (`rpodder-core/src/repo.rs`, both impls)

```rust
fn create(&self, p: &AppPassword) -> Result<()>;
fn list_for_user(&self, user_id: Uuid) -> Result<Vec<AppPassword>>;            // newest first
fn find_by_token_hash(&self, hash: &str) -> Result<Option<AppPassword>>;
/// Whether app password `id` exists and belongs to `user_id`.
fn exists_for_user(&self, id: Uuid, user_id: Uuid) -> Result<bool>;
/// Set last_used_at = now unless it is already later than now - 1h.
fn touch(&self, id: Uuid, now: DateTime<Utc>) -> Result<()>;
/// In one transaction: delete the user's password `id` and the sessions whose token starts
/// with `app-{id}-`. False (nothing changed) if it isn't theirs or doesn't exist.
fn delete(&self, user_id: Uuid, id: Uuid) -> Result<bool>;
/// In one transaction: delete all of the user's app passwords, the sessions whose token
/// starts with `app-`, and every login flow with this user_id. Returns the number of app
/// passwords removed.
fn delete_all_for_user(&self, user_id: Uuid) -> Result<u64>;

fn create_flow(&self, f: &LoginFlow) -> Result<()>;
fn delete_expired_flows(&self, now: DateTime<Utc>) -> Result<u64>;
/// Delete the oldest unapproved unexpired flows so that at most `keep` unexpired flows remain.
/// Returns how many were evicted.
fn evict_flows_over(&self, keep: i64, now: DateTime<Utc>) -> Result<u64>;
/// The unexpired flow with this login token, in any state.
fn find_flow_by_login_hash(&self, hash: &str, now: DateTime<Utc>) -> Result<Option<LoginFlow>>;
/// Set user_id on the unexpired flow if it is unapproved. Returns the flow's user_id after
/// the call (Some(user_id) on success, Some(other) when someone else approved it), or None
/// when there is no such unexpired flow.
fn approve_flow(&self, login_hash: &str, user_id: Uuid, now: DateTime<Utc>) -> Result<Option<Uuid>>;
/// Delete the unexpired, unredeemed flow if it is unapproved or approved by `user_id`.
/// False otherwise.
fn deny_flow(&self, login_hash: &str, user_id: Uuid, now: DateTime<Utc>) -> Result<bool>;
/// In one transaction: set redeemed_at on the approved, unredeemed, unexpired flow with this
/// poll token, then insert an app password (`password_id`, `token_hash`, name = the flow's
/// client_name, created_at = now) for the flow's user, only while that user is active.
/// When either step changes no row, roll back and return None.
/// Returns the user id the password was created for.
fn redeem_flow(&self, poll_hash: &str, password_id: Uuid, token_hash: &str,
               now: DateTime<Utc>) -> Result<Option<Uuid>>;
```

Implementation notes for both impls:

- `redeem_flow`, `delete`, `delete_all_for_user` use `self.pool.begin()` and run every statement on the transaction. On SQLite the pool has one connection, so touching `self.pool` inside the transaction would deadlock.
- `redeem_flow`: `UPDATE login_flows SET redeemed_at = $now WHERE poll_token_hash = $h AND user_id IS NOT NULL AND redeemed_at IS NULL AND expires_at > $now RETURNING user_id, client_name`, then `INSERT INTO app_passwords (id, user_id, name, token_hash, created_at, last_used_at) SELECT $id, id, $name, $hash, $now, NULL FROM users WHERE id = $user AND is_active` (SQLite `is_active = 1`); roll back unless it inserted one row. The `redeemed_at IS NULL` condition makes a poll token yield at most one app password under concurrent polls, and `delete_all_for_user` removing the flows makes a flow approved before a recovery unredeemable after it.
- `approve_flow`: `UPDATE … SET user_id = $u WHERE login_token_hash = $h AND user_id IS NULL AND expires_at > $now`, then select `user_id` of the unexpired flow.
- `evict_flows_over`: count the unexpired flows; when `count - keep <= 0` return 0 without a DELETE; otherwise `DELETE FROM login_flows WHERE id IN (SELECT id FROM login_flows WHERE user_id IS NULL AND expires_at > $now ORDER BY created_at LIMIT $n)` with `$n = count - keep` computed in Rust.
- Session deletion by prefix compares the start of the token, as `SessionRepo::delete_for_user` does: `left(token, length($p)) = $p` on PostgreSQL, `substr(token, 1, length(?)) = ?` on SQLite.

### Auth method (`middleware/auth.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthMethod { Password, AppPassword(Uuid), Session, SsoSession, AppSession(Uuid) }
impl AuthMethod {
    pub fn is_app(self) -> bool;                // AppPassword(_) | AppSession(_)
    pub fn app_password_id(self) -> Option<Uuid>;
}
#[derive(Clone)]
pub struct AuthUser(pub User);                  // unchanged, so existing handlers are untouched
#[derive(Clone, Copy)]
pub struct AuthVia(pub AuthMethod);             // inserted next to AuthUser by require_auth
pub const APP_SESSION_PREFIX: &str = "app-";
pub fn hash_token(token: &str) -> String;       // SHA-256, lowercase hex
```

`authenticate` returns `Option<(User, AuthMethod)>`; its other caller (`routes/directory.rs:226`) only checks `.is_some()` and is unchanged apart from the type.

`resolve_session(token)` as today, then:

- `sso-…` → `SsoSession`.
- `app-{uuid}-…` → the uuid must parse and `exists_for_user(uuid, session.user_id)` must be true, else the session is rejected (`None`). This closes the race where a `login.json` verified with an app password inserts its session after the password was revoked. → `AppSession(uuid)`.
- Anything else → `Session`.

`resolve_basic_auth(username, secret)`: find the user (as today, `None` if missing or inactive), then:

1. If `secret` is 64 `[0-9a-z]` characters, look it up with `find_by_token_hash(hash_token(secret))`. If it belongs to this user: `touch` it (errors logged at `debug`, never failing the request) and return `AppPassword(id)`.
2. Otherwise argon2 against `password_hash`, as today, returning `Password`.

A real password of that exact shape that isn't an app password still falls through to step 2.

`require_admin` additionally refuses (`403`) when `AuthVia` is an app method.

`POST /api/2/auth/{username}/login.json` takes `Extension<AuthVia>`. For any app method it mints `app-{id}-{generate_token()}` (at most 105 characters; `sessions.token` is `VARCHAR(128)` on PostgreSQL), so an app session can never be upgraded to a plain one. `create_login(session, Some(password_hash))` keeps its meaning.

`change_password` refuses app methods with `403 {"error": "not allowed with an app password"}` before its current-password check. Its existing `revoke_sessions(Logins { keep })` also removes `app-` sessions; app passwords stay (D3).

`admin_set_password` (for another user; an admin setting their own password keeps their app passwords, as with `change_password`) and `confirm_password_reset` call `delete_all_for_user` right after their last `revoke_sessions` call. `admin_reset_password` only sends the reset mail and changes nothing; revocation happens when the reset completes.

### Base URL

`fn public_base(config: &AppConfig, headers: &HeaderMap) -> Option<String>` in `routes/nextcloud.rs`:

- `config.base_url` without trailing slash, when set.
- Otherwise `{proto}://{host}`: `host` from `X-Forwarded-Host`, else `Host`; `proto` from `X-Forwarded-Proto` (`http` or `https` only), else `http`, with a `tracing::warn!` ("RPODDER_BASE_URL is unset and the request has no X-Forwarded-Proto; login URLs use http") at flow start. `None` if there is no usable host. Values are only used to build URLs handed back to the same client, never fetched.

`web/nginx.conf` sets `proxy_set_header Host $http_host; X-Forwarded-Proto $scheme; X-Forwarded-Host $http_host;` on the new `/index.php/` and `/auth/` locations. `clients.md` tells self-hosters behind HTTPS to set `RPODDER_BASE_URL`.

### Public endpoints (`routes/nextcloud.rs`, outside `require_auth_layer`)

Always routed, with or without the `web-ui` feature: the browser page is a frontend contract (`/login/flow` on the same origin), served by the embedded UI or by the separate frontend behind `web/nginx.conf`.

`POST /index.php/login/v2` (body ignored):

1. `delete_expired_flows(now)`.
2. `evict_flows_over(9999, now)`. If it evicted any, `tracing::warn!` with the count ("login flow cap reached, oldest pending flows dropped"). A flood can push out other pending flows but never blocks new ones.
3. `client_name` = `User-Agent`, trimmed to 100 characters, or `Unknown client`.
4. Store a flow with `expires_at = now + 10 min` (AntennaPod polls for 5).
5. `public_base` is `None`: `500`. Otherwise `200`:

```json
{"poll": {"token": "<poll token>", "endpoint": "<base>/index.php/login/v2/poll"},
 "login": "<base>/login/flow#<login token>"}
```

The login token travels in the fragment, which browsers send neither to the server nor in `Referer`, so it never reaches request logs (`TraceLayer` logs every URI at the default `tower_http=debug`).

`POST /index.php/login/v2/poll`, `application/x-www-form-urlencoded` `token=<poll token>` (via `axum::Form`; a missing or empty token is `404`):

- Approved and redeemable: `redeem_flow`, load the user by id, `200 {"server": "<base>", "loginName": "<username>", "appPassword": "<64 chars>"}`.
- Anything else (pending, denied, redeemed, expired, unknown, user deactivated or recovered): `404` with empty body. AntennaPod retries every second for 5 minutes on any non-200.

Neither handler logs a token, a form body or `client_name`.

### Authenticated rpodder endpoints (`routes/app_passwords.rs`, inside `require_auth_layer`)

All bodies use the `axum::Json` extractor, which requires `Content-Type: application/json`; with `CorsLayer::permissive()` (no credentials) and `SameSite=Lax` cookies a cross-site page cannot send them with the user's cookie. Every route answers `403 {"error": "not allowed with an app password"}` to app methods.

| Route | Body | Response |
|---|---|---|
| `GET /api/2/me/app-passwords` | | `200 [{"id","name","created_at","last_used_at"}]` (RFC 3339; `last_used_at` null when never used), newest first |
| `POST /api/2/me/app-passwords` | `{"name": "…", "current_password": "…"}` | `201 {"id","name","created_at","password"}`, the password shown once. `name` 1–100 chars after trim, else `400`. `current_password` required and checked with argon2 unless `SsoSession`, else `403 {"error": "current password required"}` / `403 {"error": "wrong password"}`, mirroring `change_password` |
| `DELETE /api/2/me/app-passwords/{id}` | | `204`; `404` when not the caller's or unknown; `400` for a malformed id |
| `POST /api/2/me/login-flows/lookup` | `{"token": "<login token>"}` | `200 {"client_name", "created_at", "approved": bool, "username"}`, `approved` true only when the caller approved it, `username` the caller's; `404` when unknown, expired, or approved by someone else |
| `POST /api/2/me/login-flows/answer` | `{"token": "…", "approve": bool}` | `204`. Approve: success or already approved by the caller → `204`; approved by someone else → `409`; unknown or expired → `404`. Deny: `deny_flow` true → `204`, else `404` |

### gpoddersync endpoints (`routes/nextcloud.rs`, inside `require_auth_layer`)

Device: `DeviceRepo::find_by_uid(user, "gpoddersync")`; on `None`, `DeviceRepo::upsert` a `Device { device_id: "gpoddersync", caption: "Nextcloud sync", device_type: Other, sync_group_id: None, … }`. An existing device is never upserted, so a renamed caption stays. Done once per request before calling the handler.

Each route is an adapter that calls the existing handler directly with constructed extractors:

| gpoddersync route | Calls | Response |
|---|---|---|
| `GET /index.php/apps/gpoddersync/subscriptions?since=N` | `subscriptions::download_subscription_changes(State, Path((username, "gpoddersync".into())), Extension(auth_user), Query(SinceQuery { since: Some(max(0, N - 300)) }))` | `{"add","remove","update_urls":[],"timestamp"}`; gpoddersync's own shape is `{"add","remove","timestamp"}`, both apps ignore the extra field |
| `POST …/subscription_change/create` `{"add":[…],"remove":[…]}` | `subscriptions::upload_subscription_changes(…, Json(body))` | `{"timestamp","update_urls"}`; the apps only check for `200` |
| `GET …/episode_action?since=N` | `episodes::download_episode_actions(State, Path(format!("{username}.json")), Extension(auth_user), Query(EpisodeQuery { since: Some(max(0, N - 300)), podcast: None, device: None, aggregated: false }))` | `{"actions":[…],"timestamp"}` |
| `POST …/episode_action/create` bare `[…]` (or `{"actions": […]}`) | parse as JSON (`400` if that fails), lowercase each `action` (the Nextcloud app stores them uppercase), set `"device": "gpoddersync"` on each object that has no `device`, re-serialize, call `episodes::upload_episode_actions(…, body)` | `{"timestamp","update_urls"}` |

A missing `since` is passed as `None`. `since` is lowered by 300 seconds because AntennaPod's Nextcloud provider stores the phone's clock, not the server's `timestamp`, as its next `since` (`NextcloudSyncService.java`). rpodder filters on server receive time with 60 seconds of slack, so a phone running fast would miss other devices' changes. Re-sent subscription changes are idempotent, and re-sent episode actions are resolved by their timestamps in the app, as with the existing slack.

Subscriptions are per device in rpodder: the gpoddersync device starts empty and gets other devices' subscriptions only through a sync group. The app uploads its own list on the first sync. Episode actions are per user and reach every client.

### Web UI

`lib/navigation.ts` (new, pure, `node --test`-able like `playerLogic.ts`):

```ts
/** `value` if it is a same-origin path, else null. Both `value` and its percent-decoding
 *  (decodeURIComponent; a decoding error gives null) must start with "/" but not "//",
 *  and contain no "\\" and no control characters; new URL(value, origin) must keep `origin`. */
export function safeNext(value: string | null, origin: string): string | null;
/** Store a return target for after login: sessionStorage `rpodder_next` = {next, at: Date.now()}. */
export function rememberNext(next: string): void;
/** Read and remove the return target; null if missing, older than 15 minutes, or not safeNext. */
export function takeNext(origin: string): string | null;
```

`rememberNext`/`takeNext` take the storage as an optional parameter so the tests can pass a fake.

The return target never travels in a URL: `/login`, the layout and `sessionExpired` are unchanged in their URLs.

`/login`: after a password login, and when already signed in on load, `goto(takeNext(location.origin) ?? '/subscriptions')`. The SSO button needs no change: the target is already in sessionStorage.

`/` (where `sso_redirect_html` lands): when signed in, `goto(takeNext(location.origin) ?? '/subscriptions', { replaceState: true })`.

`lib/api.ts`:

```ts
export interface AppPassword { id: string; name: string; created_at: string; last_used_at: string | null; }
export async function getAppPasswords(): Promise<AppPassword[] | 'forbidden'>;   // 403 → 'forbidden', other errors throw ApiError
export async function createAppPassword(name: string, currentPassword?: string):
  Promise<{ ok: true; password: string } | { ok: false; status: number; error: string }>;
export async function deleteAppPassword(id: string): Promise<boolean>;
export type FlowInfo = { client_name: string; created_at: string; approved: boolean; username: string };
export async function lookupLoginFlow(token: string): Promise<FlowInfo | 'gone' | 'unauthenticated' | 'forbidden'>;
export async function answerLoginFlow(token: string, approve: boolean):
  Promise<'ok' | 'gone' | 'taken' | 'unauthenticated' | 'forbidden' | 'error'>;
```

`401` → `'unauthenticated'`, `403` → `'forbidden'`, `404` → `'gone'`, `409` → `'taken'`.

`/login/flow` (token from `location.hash`, without `#`):

- On load, with a token: `rememberNext('/login/flow#' + token)` before anything else. Whoever redirects to `/login` next (this page, or the layout's own expiry check racing it), the login page returns here.
- No token: "This link is incomplete. Start the login again in your app."
- Not signed in: `goto('/login')`.
- `'unauthenticated'`: `sessionExpired(auth.username)` (to `/login?expired=1`).
- `'forbidden'`: "You're signed in with an app password. Sign out and sign in with your account password or SSO to grant access." (with a sign-out button).
- `'gone'`: "This link has expired or was already used. Start the login again in your app."
- Any answer other than `'unauthenticated'` calls `takeNext` to drop the stored target, so it can't send a later login back here.
- `approved`: "Access granted. You can go back to the app."
- Pending:
  - "An app wants to sync with your rpodder account **{username from lookup}**."
  - "It calls itself **{client_name}** (reported by the app), requested {relative time}."
  - Warning: "Only continue if you started signing in to an app just now. If you didn't, someone may be trying to get access to your account. Deny it."
  - Buttons **Grant access** / **Deny**. Results: `ok` → "Access granted. You can go back to the app." or "Access denied."; `taken` → "This request was already answered."; `gone` → expired text; `unauthenticated` / `forbidden` → as above; `error` → "Something went wrong. Try again."
- `AuthFrame` like `/login`; loading in `$effect` with a `browser` guard. `/login/flow` joins the layout's `authPage` list so it renders without the app shell.

`/settings`, new section **App passwords** next to the password section:

- `getAppPasswords()` returns `'forbidden'` → the section shows "Sign in with your password to manage app passwords." It is checked before the page's existing `isAuthError` handling, which would otherwise treat the 403 as an expired session.
- List: name, created, last used ("never"), **Revoke** with `confirm()`.
- Form: name and current password ("Not needed if you signed in with SSO"), **Create**. Server errors (`current password required`, `wrong password`) show under the form.
- The new password is shown once in a read-only field with a copy button: "Shown once. Use it instead of your password in podcast apps."

Proxies: `web/vite.config.ts` and `web/nginx.conf` forward `/index.php` (and nginx also `/auth`, missing today) to the backend.

## UX flow

1. In AntennaPod: Settings → Synchronization → Nextcloud → server `https://pod.example.com`.
2. App: `POST /index.php/login/v2` → opens `login` in the browser, then polls every second.
3. Browser: `/login/flow#…` stores the return target → not signed in → `/login` → password or SSO → back on `/login/flow` → reads the warning → **Grant access**.
4. App's next poll gets `200` with the app password and stores `server`, `loginName`, `appPassword`.
5. App syncs through `/index.php/apps/gpoddersync/*` with Basic `loginName:appPassword`.
6. Settings → App passwords lists the app's User-Agent with "last used". Revoke → the next sync gets `401`, which AntennaPod reports as a sync error ("Response code: 401"); the user logs in again from the sync settings.

## Auth & ownership

- `/index.php/login/v2` and `/poll` are public. The poll token is the only credential for redeeming and is never shown in the browser. The login token can't be polled with.
- Phishing: an attacker can start a flow and send a victim the link. The grant page names the account (from the server), labels the client name as self-reported, shows when the request was made, warns as Nextcloud does, and the link dies after 10 minutes. The grant still binds to whoever approves, so the defence is the user reading the page, as in Nextcloud.
- Lookup and answer need a non-app authentication. Deny can't remove a flow another user approved.
- `/api/2/me/app-passwords/{id}` deletes only the caller's rows.
- gpoddersync routes have no `{username}` segment and act on the authenticated user.
- D5: app passwords and their sessions can sync and use every gpodder and other `/api/2/me` route, but can't create credentials, look up or answer flows, change the password or use `/api/admin/*`. `login.json` keeps an app session an app session.
- Revocation is immediate for both Basic (the row is gone) and sessions (deleted in the same transaction, and `resolve_session` re-checks the app password for every `app-` session).
- Brute force: a poll guesses a 64-character random token; Basic with a wrong app password costs one indexed lookup plus the existing argon2 check.

## gpodder compatibility

No existing route changes shape. Basic Auth gains the app-password branch; sessions gain the `app-` prefix, which `is_one_time_token` doesn't match. gpoddersync clients see the shapes above, which AntennaPod parses with the same `ResponseMapper` as its gpodder.net provider.

## Data & migrations

One migration pair, `006_app_passwords`, idempotent (`IF NOT EXISTS`), added to `MIGRATIONS`. No backfill. UUID v7 ids. Expired flows are deleted on each flow start; no background task.

## Privacy

Nothing reaches the directory. Tokens never appear in URLs the server sees (fragment for the browser link, sessionStorage for the return target, JSON bodies for the API) and are never logged. `client_name` is shown only to the approving user, escaped by Svelte.

## Build variants & footprint

`--no-default-features`: everything works; the browser page comes from whatever frontend serves `/login/flow` on the same origin. One small dependency (`sha2`, already in `Cargo.lock`). On a Raspberry Pi: one indexed lookup per Basic request and per `app-` session request, at most one `last_used_at` write per password per hour, at most 10,000 flow rows.

## Failure modes

| Situation | Result |
|---|---|
| Flow not approved within 10 min | Poll keeps `404`; AntennaPod gives up after 5 min and shows its error |
| User denies | Flow deleted, poll `404` until the app gives up |
| Grant page reloaded after the app received its password | "Access granted" (the redeemed flow stays until it expires) |
| Link opened after expiry | Expired message |
| Cookie expired while on the flow page | `/login?expired=1`, then back to the flow page |
| Browser signed in with an app password | Flow page and Settings explain that the account password or SSO is needed |
| Flow flood | Oldest pending flows dropped, warning logged, new flows still work |
| Account deactivated before redeem | Poll `404`, nothing created |
| App password revoked (also racing a `login.json`) | Its sessions deleted or rejected; `401` on the next request |
| Own password changed | Other sessions, `app-` ones included, logged out; app passwords keep working over Basic |
| Password reset completed or set by an admin | All app passwords, their sessions and the user's login flows deleted |
| `touch` fails (busy SQLite) | Logged at debug, request authenticated |
| No `RPODDER_BASE_URL`, no `X-Forwarded-Proto` | `http` URLs, warning logged |
| No `Host` and no `RPODDER_BASE_URL` | Flow start `500` |

## Verification criteria

- SQLite repo tests (`sqlite.rs` tests module): create/list/find/delete app passwords; delete refuses another user's id and removes `app-{id}-` sessions only; `exists_for_user`; `delete_all_for_user` removes passwords, `app-` sessions and flows, and a flow approved before it can't be redeemed after; `touch` writes at most once per hour; approve returns the right owner; deny refuses a flow approved by someone else; redeem twice yields one password; redeem of an expired, unapproved or deactivated-user flow yields none and leaves `redeemed_at` unset; `evict_flows_over` keeps the newest and never touches approved flows.
- Router tests (`src/tests/nextcloud_login.rs`, real `api_router` on in-memory SQLite; flows and `sso-` sessions inserted through `TestApp::repo()`):
  - full flow: start → `login` contains `#` and no token query → poll `404` → lookup + approve via session → poll `200` with `server`, `loginName`, 64-char password → poll again `404` → lookup still `approved`;
  - `public_base` from `Host`/`X-Forwarded-*` when `base_url` is empty, and from `base_url` when set;
  - the app password authenticates the gpoddersync routes and a gpodder.net route; wrong `401`; revoked `401`; deactivated user `401`;
  - `login.json` with an app password, then revoke → the cookie gets `401`; `login.json` with an app cookie mints another `app-` session (the flow answer still `403`);
  - an `app-` session whose app password was deleted directly in the database (simulating the race) gets `401`;
  - app methods get `403` on app-password list/create/delete, flow lookup/answer, `change_password`, and an admin route for an admin user;
  - manual create needs the current password for a password session (`403` without, `403` wrong, `201` right) and not for an `sso-` session;
  - `change_password` logs out the `app-` session but Basic with the app password still works;
  - `confirm_password_reset` and `admin_set_password` remove the user's app passwords, `app-` sessions and approved flows (poll then `404`);
  - subscription and episode-action round trip through the gpoddersync routes; uploaded actions carry device `gpoddersync`; a gpodder.net device of the same user sees them; `since` slack returns a change made 2 minutes before `since`;
  - deny, expired (flow inserted with past `expires_at`), approve by a second user `409`, lookup by the second user `404`, lookup returns the caller's `username`, flood eviction keeps the newest flows.
- `web/src/lib/navigation.test.ts`: `safeNext` accepts `/settings?x=1`, `/login/flow#abc`; rejects `//evil.com`, `/\evil.com`, `/\t/evil.com`, `/%5Cevil.com`, `/%2F%2Fevil.com`, `https://evil.com`, `javascript:alert(1)`, `%zz`, empty, null. `takeNext` returns a fresh target once, drops a 16-minute-old one, rejects an unsafe one. Added to the `test` script.
- Docker E2E `tests/docker/tests/12_nextcloud.sh`, run for `sqlite` and `postgres`: start, lookup, approve with `$AUTH_HEADER` (password Basic), poll, poll again `404`; gpoddersync subscription and episode upload/download with the app password; Basic with the app password on `/api/2/devices/$USER.json`; `login.json` with the app password, revoke, then the cookie `401`; app-password create (with current password), list, revoke, then Basic `401`; a second flow denied, then poll `404`; double approve by the same user `204`; `admin_set_password` on a second user removes that user's app password (Basic `401`).
- Gate: `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`, `cargo test`, `cargo build --no-default-features`, `bun run build`, `bun run test`.

## Out-of-scope follow-ups

- Revoking app passwords from the admin panel for another user.
- Showing gpoddersync clients by name on `/devices` (they share one device).
- A trusted-proxy setting, after which the grant page could show the requester's IP.
