use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Serialize;

use rpodder_core::repo::{DeviceRepo, SubscriptionRepo, UserRepo};

use crate::state::AppState;
use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo, Db};

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
pub async fn status_page(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    // Gather data
    let data = gather_status(&state).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut html = String::from(r#"<!DOCTYPE html>
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
"#);

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
            format!(r#" <span style="color:#888; font-size:.85rem;">({})</span>"#, user.email)
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
                html.push_str(&format!(r#"<li class="sub-url" style="padding: 2px 0;">• {}</li>"#, url));
            }
            html.push_str("</ul>");
        }

        html.push_str("</div>\n");
    }

    html.push_str(r#"<p style="color:#555; font-size:.75rem; margin-top:2rem;">rpodder v0.1.0</p>
</body></html>"#);

    Ok(Html(html))
}

async fn gather_status(state: &AppState) -> Result<StatusData, Box<dyn std::error::Error + Send + Sync>> {
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
