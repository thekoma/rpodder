use axum::{
    Extension,
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, SubscriptionRepo};

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};

macro_rules! with_repo {
    ($state:expr, |$repo:ident| $body:expr) => {
        match &*$state.db {
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

#[derive(Serialize)]
struct UserInfo {
    username: String,
    email: String,
    active: bool,
    devices: Vec<DeviceInfo>,
    subscriptions: Vec<String>,
}

#[derive(Serialize)]
struct DeviceInfo {
    device_id: String,
    caption: String,
    device_type: String,
}

#[derive(Serialize)]
struct StatusData {
    users: Vec<UserInfo>,
    total_episode_actions: i64,
}

/// GET / — simple HTML dashboard showing DB contents
pub async fn status_page(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    // Gather data
    let data = gather_status(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>rpodder — status</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: system-ui, -apple-system, sans-serif; background: #0f1117; color: #e0e0e0; padding: 2rem; max-width: 900px; margin: 0 auto; }
  h1 { color: #7cb3f5; margin-bottom: .3rem; }
  .subtitle { color: #888; margin-bottom: 2rem; font-size: .9rem; }
  .card { background: #1a1d27; border: 1px solid #2a2d37; border-radius: 8px; padding: 1.2rem; margin-bottom: 1rem; }
  .card h2 { color: #a0c4ff; font-size: 1.1rem; margin-bottom: .8rem; }
  .badge { display: inline-block; background: #2a3a5a; color: #7cb3f5; padding: 2px 8px; border-radius: 4px; font-size: .8rem; margin-right: .3rem; }
  .badge.active { background: #1a3a2a; color: #6fcf97; }
  .badge.inactive { background: #3a1a1a; color: #cf6f6f; }
  table { width: 100%; border-collapse: collapse; margin-top: .5rem; }
  th, td { text-align: left; padding: .4rem .6rem; border-bottom: 1px solid #2a2d37; font-size: .85rem; }
  th { color: #888; font-weight: 500; }
  .stat { display: inline-block; background: #1a1d27; border: 1px solid #2a2d37; border-radius: 6px; padding: .6rem 1rem; margin-right: .5rem; margin-bottom: .5rem; }
  .stat-val { font-size: 1.5rem; font-weight: 700; color: #7cb3f5; }
  .stat-label { font-size: .75rem; color: #888; }
  .sub-url { color: #a0c4ff; word-break: break-all; font-size: .82rem; }
  .empty { color: #666; font-style: italic; }
</style>
</head>
<body>
<h1>rpodder</h1>
<p class="subtitle">gpodder-compatible podcast sync server</p>

<div style="margin-bottom: 1.5rem;">
"#,
    );

    // Stats summary
    let total_subs: usize = data.users.iter().map(|u| u.subscriptions.len()).sum();
    let total_devices: usize = data.users.iter().map(|u| u.devices.len()).sum();

    html.push_str(&format!(
        r#"<div class="stat"><div class="stat-val">{}</div><div class="stat-label">users</div></div>
<div class="stat"><div class="stat-val">{}</div><div class="stat-label">devices</div></div>
<div class="stat"><div class="stat-val">{}</div><div class="stat-label">subscriptions</div></div>
<div class="stat"><div class="stat-val">{}</div><div class="stat-label">episode actions</div></div>
</div>"#,
        data.users.len(),
        total_devices,
        total_subs,
        data.total_episode_actions,
    ));

    // Per-user cards
    for user in &data.users {
        let status_badge = if user.active {
            r#"<span class="badge active">active</span>"#
        } else {
            r#"<span class="badge inactive">inactive</span>"#
        };
        let email_display = if user.email.is_empty() {
            String::new()
        } else {
            format!(
                r#" <span style="color:#888; font-size:.85rem;">({})</span>"#,
                user.email
            )
        };

        html.push_str(&format!(
            r#"<div class="card">
<h2>{} {}{}</h2>
"#,
            user.username, status_badge, email_display
        ));

        // Devices
        if user.devices.is_empty() {
            html.push_str(r#"<p class="empty">No devices</p>"#);
        } else {
            html.push_str(r#"<table><tr><th>Device ID</th><th>Caption</th><th>Type</th></tr>"#);
            for d in &user.devices {
                html.push_str(&format!(
                    r#"<tr><td><code>{}</code></td><td>{}</td><td><span class="badge">{}</span></td></tr>"#,
                    d.device_id, d.caption, d.device_type
                ));
            }
            html.push_str("</table>");
        }

        // Subscriptions
        if user.subscriptions.is_empty() {
            html.push_str(r#"<p class="empty" style="margin-top:.6rem;">No subscriptions</p>"#);
        } else {
            html.push_str(&format!(
                r#"<p style="margin-top:.8rem; color:#888; font-size:.8rem;">Subscriptions ({})</p><ul style="list-style:none; margin-top:.3rem;">"#,
                user.subscriptions.len()
            ));
            for url in &user.subscriptions {
                html.push_str(&format!(
                    r#"<li class="sub-url" style="padding: 2px 0;">• {}</li>"#,
                    url
                ));
            }
            html.push_str("</ul>");
        }

        html.push_str("</div>\n");
    }

    html.push_str(
        r#"<p style="color:#555; font-size:.75rem; margin-top:2rem;">rpodder v0.1.0</p>
</body></html>"#,
    );

    Ok(Html(html))
}

async fn gather_status(
    state: &AppState,
) -> Result<StatusData, Box<dyn std::error::Error + Send + Sync>> {
    // Get all users — we need a simple query since UserRepo doesn't have list_all
    let users_raw: Vec<(String, String, Option<String>, bool)> = match &*state.db {
        Db::Postgres(pool) => {
            let rows: Vec<(String, String, Option<String>, bool)> = sqlx::query_as(
                "SELECT id::text, username, email, is_active FROM users ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?;
            rows
        }
        Db::Sqlite(pool) => {
            let rows: Vec<(String, String, Option<String>, bool)> = sqlx::query_as(
                "SELECT id, username, email, is_active FROM users ORDER BY created_at",
            )
            .fetch_all(pool)
            .await?;
            rows
        }
    };

    let mut users = Vec::new();
    for (id_str, username, email, active) in &users_raw {
        let user_id: uuid::Uuid = id_str.parse().unwrap_or_default();

        let devices_raw = with_repo!(state, |repo| {
            DeviceRepo::list_for_user(&repo, user_id).await
        })?;

        let subs_raw = with_repo!(state, |repo| {
            SubscriptionRepo::list_for_user(&repo, user_id).await
        })?;

        let devices: Vec<DeviceInfo> = devices_raw
            .into_iter()
            .map(|d| DeviceInfo {
                device_id: d.device_id,
                caption: d.caption,
                device_type: format!("{:?}", d.device_type).to_lowercase(),
            })
            .collect();

        let subscriptions: Vec<String> = subs_raw.into_iter().map(|s| s.ref_url).collect();

        users.push(UserInfo {
            username: username.clone(),
            email: email.clone().unwrap_or_default(),
            active: *active,
            devices,
            subscriptions,
        });
    }

    // Count episode actions
    let total_episode_actions: i64 = match &*state.db {
        Db::Postgres(pool) => {
            let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episode_actions")
                .fetch_one(pool)
                .await?;
            row.0
        }
        Db::Sqlite(pool) => {
            let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episode_actions")
                .fetch_one(pool)
                .await?;
            row.0
        }
    };

    Ok(StatusData {
        users,
        total_episode_actions,
    })
}

// ---------------------------------------------------------------------------
// Admin JSON API
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct AdminUserResponse {
    pub username: String,
    pub email: Option<String>,
    pub active: bool,
    pub devices: usize,
    pub subscriptions: usize,
}

/// GET /api/admin/users — list all users (admin only, requires auth)
pub async fn list_users(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let data = gather_status(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let users: Vec<AdminUserResponse> = data
        .users
        .into_iter()
        .map(|u| AdminUserResponse {
            username: u.username,
            email: if u.email.is_empty() {
                None
            } else {
                Some(u.email)
            },
            active: u.active,
            devices: u.devices.len(),
            subscriptions: u.subscriptions.len(),
        })
        .collect();

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

/// POST /api/admin/users or /api/2/register — create a new user
/// Respects registration mode: open (anyone), closed (admin only), invite (email confirmation)
pub async fn create_user(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    use rpodder_core::repo::UserRepo;

    let hash = crate::middleware::auth::hash_password(&body.password)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = with_repo!(state, |repo| {
        UserRepo::create(&repo, &body.username, &hash, body.email.as_deref()).await
    })
    .map_err(|e| match e {
        rpodder_core::error::AppError::Conflict(_) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;

    // If invite mode with SMTP, deactivate and send activation email
    if state.config.registration_invite() && state.config.smtp_configured() {
        // Deactivate until email confirmed
        match &*state.db {
            rpodder_db::Db::Postgres(pool) => {
                let _ = sqlx::query("UPDATE users SET is_active = false WHERE id = $1")
                    .bind(user.id)
                    .execute(pool)
                    .await;
            }
            rpodder_db::Db::Sqlite(pool) => {
                let _ = sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
                    .bind(user.id.to_string())
                    .execute(pool)
                    .await;
            }
        }

        // Generate activation token and store it
        let token = uuid::Uuid::now_v7().to_string();
        match &*state.db {
            rpodder_db::Db::Postgres(pool) => {
                let _ = sqlx::query(
                    "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(uuid::Uuid::now_v7())
                .bind(user.id)
                .bind(format!("activate-{token}"))
                .bind(chrono::Utc::now() + chrono::Duration::hours(48))
                .bind(chrono::Utc::now())
                .execute(pool)
                .await;
            }
            rpodder_db::Db::Sqlite(pool) => {
                let _ = sqlx::query(
                    "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(uuid::Uuid::now_v7().to_string())
                .bind(user.id.to_string())
                .bind(format!("activate-{token}"))
                .bind((chrono::Utc::now() + chrono::Duration::hours(48)).to_rfc3339())
                .bind(chrono::Utc::now().to_rfc3339())
                .execute(pool)
                .await;
            }
        }

        // Send activation email
        if let Some(email) = &body.email {
            let _ =
                crate::email::send_activation_email(&state.config, email, &body.username, &token);
        }

        return Ok((
            StatusCode::CREATED,
            axum::Json(serde_json::json!({
                "status": "pending_activation",
                "message": "Check your email to activate your account"
            })),
        ));
    }

    Ok((
        StatusCode::CREATED,
        axum::Json(serde_json::json!({ "status": "active" })),
    ))
}

/// GET /api/2/activate?token=X — activate account via email link
pub async fn activate_account(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let token = params.get("token").ok_or(StatusCode::BAD_REQUEST)?;
    let activate_token = format!("activate-{token}");

    // Find the session with this activation token
    use rpodder_core::repo::SessionRepo;
    let session = with_repo!(state, |repo| {
        SessionRepo::find_by_token(&repo, &activate_token).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Activate the user
    match &*state.db {
        rpodder_db::Db::Postgres(pool) => {
            sqlx::query("UPDATE users SET is_active = true WHERE id = $1")
                .bind(session.user_id)
                .execute(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        rpodder_db::Db::Sqlite(pool) => {
            sqlx::query("UPDATE users SET is_active = 1 WHERE id = ?")
                .bind(session.user_id.to_string())
                .execute(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    // Delete the activation token
    with_repo!(state, |repo| {
        SessionRepo::delete(&repo, &activate_token).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(user_id = %session.user_id, "account activated");

    // Redirect to login
    Ok(axum::response::Redirect::temporary("/login"))
}

/// POST /api/admin/users/{username}/deactivate — deactivate a user
pub async fn deactivate_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    match &*state.db {
        Db::Postgres(pool) => {
            sqlx::query("UPDATE users SET is_active = false WHERE LOWER(username) = LOWER($1)")
                .bind(&username)
                .execute(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        Db::Sqlite(pool) => {
            sqlx::query("UPDATE users SET is_active = 0 WHERE username = ? COLLATE NOCASE")
                .bind(&username)
                .execute(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    Ok(StatusCode::OK)
}

/// POST /api/admin/feeds/update — force update all feeds now
pub async fn force_feed_update(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = state.db.clone();
    tokio::spawn(async move {
        let fetcher = rpodder_feed::FeedFetcher::new();
        crate::feed_updater::run_one_cycle(&db, &fetcher).await;
    });
    Ok(Json(serde_json::json!({ "status": "feed update started" })))
}

#[derive(Deserialize)]
pub struct FeedUpdateQuery {
    pub url: String,
}

/// POST /api/admin/feeds/update/single?url=X — force update a single feed
pub async fn force_single_feed_update(
    State(state): State<AppState>,
    Query(params): Query<FeedUpdateQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = state.db.clone();
    let url = params.url.clone();
    tokio::spawn(async move {
        let fetcher = rpodder_feed::FeedFetcher::new();
        let _ = crate::feed_updater::update_podcast_feed_forced(&db, &fetcher, &url).await;
    });
    Ok(Json(
        serde_json::json!({ "status": "feed update started", "url": params.url }),
    ))
}

// ---------------------------------------------------------------------------
// Episode Actions History (user-facing, not admin)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct EpisodeHistoryItem {
    pub podcast_title: String,
    pub podcast_url: String,
    pub episode_title: String,
    pub action: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub page: Option<i64>,
}

/// GET /api/2/history/{username}.json — episode action history with enriched data
pub async fn episode_history(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<HistoryQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = username_json
        .strip_suffix(".json")
        .unwrap_or(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let page = params.page.unwrap_or(0);
    let per_page = 50i64;

    // Get episode actions with enriched data
    type HistRow = (
        String,
        String,
        String,
        String,
        String,
        Option<i32>,
        Option<i32>,
    );
    let items: Vec<EpisodeHistoryItem> = match &*state.db {
        Db::Postgres(pool) => {
            let rows: Vec<HistRow> = sqlx::query_as(
                "SELECT
                        COALESCE(p.title, ea.podcast_ref_url, '') as podcast_title,
                        COALESCE(ea.podcast_ref_url, '') as podcast_url,
                        COALESCE(e.title, ea.episode_ref_url, '') as episode_title,
                        ea.action,
                        ea.timestamp::text,
                        ea.position,
                        ea.total
                     FROM episode_actions ea
                     LEFT JOIN episodes e ON e.id = ea.episode_id
                     LEFT JOIN podcasts p ON p.id = e.podcast_id
                     WHERE ea.user_id = $1
                     ORDER BY ea.timestamp DESC
                     LIMIT $2 OFFSET $3",
            )
            .bind(auth_user.0.id)
            .bind(per_page)
            .bind(page * per_page)
            .fetch_all(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            rows.into_iter()
                .map(|(pt, pu, et, action, ts, pos, total)| EpisodeHistoryItem {
                    podcast_title: pt,
                    podcast_url: pu,
                    episode_title: et,
                    action,
                    timestamp: ts,
                    position: pos,
                    total,
                })
                .collect()
        }
        Db::Sqlite(pool) => {
            let rows: Vec<HistRow> = sqlx::query_as(
                "SELECT
                        COALESCE(p.title, ea.podcast_ref_url, '') as podcast_title,
                        COALESCE(ea.podcast_ref_url, '') as podcast_url,
                        COALESCE(e.title, ea.episode_ref_url, '') as episode_title,
                        ea.action,
                        ea.timestamp,
                        ea.position,
                        ea.total
                     FROM episode_actions ea
                     LEFT JOIN episodes e ON e.id = ea.episode_id
                     LEFT JOIN podcasts p ON p.id = e.podcast_id
                     WHERE ea.user_id = ?
                     ORDER BY ea.timestamp DESC
                     LIMIT ? OFFSET ?",
            )
            .bind(auth_user.0.id.to_string())
            .bind(per_page)
            .bind(page * per_page)
            .fetch_all(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            rows.into_iter()
                .map(|(pt, pu, et, action, ts, pos, total)| EpisodeHistoryItem {
                    podcast_title: pt,
                    podcast_url: pu,
                    episode_title: et,
                    action,
                    timestamp: ts,
                    position: pos,
                    total,
                })
                .collect()
        }
    };

    Ok(Json(items))
}
