//! Nextcloud compatibility for AntennaPod's and Podcini's "Nextcloud gPodder
//! Sync" provider. Login Flow v2 (`/index.php/login/v2*`) issues an app
//! password, and the gpoddersync routes map onto the gpodder API.

use axum::{
    Extension, Form, Json,
    extract::{Query, State, rejection::FormRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use rpodder_core::repo::{AppPasswordRepo, DeviceRepo, UserRepo};
use rpodder_core::types::{Device, DeviceType, LoginFlow};

use super::{episodes, subscriptions};
use crate::config::AppConfig;
use crate::middleware::auth::{AuthUser, hash_token};
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

/// Device all gpoddersync clients of a user share. gpoddersync has no devices.
pub const DEVICE_ID: &str = "gpoddersync";
const DEVICE_CAPTION: &str = "Nextcloud sync";
/// AntennaPod polls for 5 minutes.
const FLOW_LIFETIME_MINUTES: i64 = 10;
const MAX_PENDING_FLOWS: i64 = 9999;
const MAX_CLIENT_NAME_LEN: usize = 100;
/// AntennaPod's Nextcloud provider stores the phone's clock as its next
/// `since`, not the server's `timestamp`. Changes are re-sent from this far
/// back so a phone running fast doesn't miss any.
const SINCE_SLACK_SECS: i64 = 300;

/// The URL clients reach this server at. `RPODDER_BASE_URL` when set,
/// otherwise derived from the request.
pub(crate) fn public_base(config: &AppConfig, headers: &HeaderMap) -> Option<String> {
    if !config.base_url.is_empty() {
        return Some(config.base_url.trim_end_matches('/').to_string());
    }
    let header_str = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').next().unwrap_or(v).trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let host = header_str("x-forwarded-host").or_else(|| header_str(header::HOST.as_str()))?;
    if host.contains(['/', '\\', '@', ' ']) {
        return None;
    }
    let proto = match header_str("x-forwarded-proto").as_deref() {
        Some("https") => "https",
        _ => "http",
    };
    Some(format!("{proto}://{host}"))
}

// ---------------------------------------------------------------------------
// POST /index.php/login/v2
// ---------------------------------------------------------------------------

pub async fn start_login_flow(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let base = public_base(&state.config, &headers).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if state.config.base_url.is_empty() && !headers.contains_key("x-forwarded-proto") {
        tracing::warn!(
            "RPODDER_BASE_URL is unset and the request has no X-Forwarded-Proto, login URLs use http"
        );
    }
    let now = Utc::now();

    with_repo!(state, |repo| {
        AppPasswordRepo::delete_expired_flows(&repo, now).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let evicted = with_repo!(state, |repo| {
        AppPasswordRepo::evict_flows_over(&repo, MAX_PENDING_FLOWS, now).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if evicted > 0 {
        tracing::warn!(
            evicted,
            "login flow cap reached, oldest pending flows dropped"
        );
    }

    let client_name = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|ua| {
            ua.trim()
                .chars()
                .take(MAX_CLIENT_NAME_LEN)
                .collect::<String>()
        })
        .filter(|ua| !ua.is_empty())
        .unwrap_or_else(|| "Unknown client".to_string());

    let poll_token = super::auth::generate_token();
    let login_token = super::auth::generate_token();
    let flow = LoginFlow {
        id: Uuid::now_v7(),
        poll_token_hash: hash_token(&poll_token),
        login_token_hash: hash_token(&login_token),
        client_name,
        user_id: None,
        redeemed_at: None,
        expires_at: now + Duration::minutes(FLOW_LIFETIME_MINUTES),
        created_at: now,
    };
    with_repo!(state, |repo| AppPasswordRepo::create_flow(&repo, &flow)
        .await)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // The login token goes in the fragment, which browsers don't send to the
    // server, so it stays out of request logs.
    Ok(Json(serde_json::json!({
        "poll": {
            "token": poll_token,
            "endpoint": format!("{base}/index.php/login/v2/poll"),
        },
        "login": format!("{base}/login/flow#{login_token}"),
    }))
    .into_response())
}

// ---------------------------------------------------------------------------
// POST /index.php/login/v2/poll
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PollForm {
    pub token: Option<String>,
}

pub async fn poll_login_flow(
    State(state): State<AppState>,
    headers: HeaderMap,
    form: Result<Form<PollForm>, FormRejection>,
) -> Result<Response, StatusCode> {
    let token = form
        .ok()
        .and_then(|Form(f)| f.token)
        .filter(|t| !t.is_empty())
        .ok_or(StatusCode::NOT_FOUND)?;
    let base = public_base(&state.config, &headers).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let password = super::auth::generate_token();
    let user_id = with_repo!(state, |repo| {
        AppPasswordRepo::redeem_flow(
            &repo,
            &hash_token(&token),
            Uuid::now_v7(),
            &hash_token(&password),
            Utc::now(),
        )
        .await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let user = with_repo!(state, |repo| UserRepo::find_by_id(&repo, user_id).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "server": base,
        "loginName": user.username,
        "appPassword": password,
    }))
    .into_response())
}

// ---------------------------------------------------------------------------
// gpoddersync: /index.php/apps/gpoddersync/*
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SinceParam {
    pub since: Option<i64>,
}

fn with_slack(since: Option<i64>) -> Option<i64> {
    since.map(|s| (s - SINCE_SLACK_SECS).max(0))
}

/// Creates the shared gpoddersync device with its caption on first use. An
/// existing one is left alone, so a renamed caption stays.
async fn ensure_device(state: &AppState, user_id: Uuid) -> Result<(), StatusCode> {
    let existing = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, user_id, DEVICE_ID).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if existing.is_some() {
        return Ok(());
    }
    let now = Utc::now();
    let device = Device {
        id: Uuid::now_v7(),
        user_id,
        device_id: DEVICE_ID.to_string(),
        caption: DEVICE_CAPTION.to_string(),
        device_type: DeviceType::Other,
        sync_group_id: None,
        created_at: now,
        updated_at: now,
    };
    with_repo!(state, |repo| DeviceRepo::upsert(&repo, &device).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

/// GET /index.php/apps/gpoddersync/subscriptions?since=N
pub async fn subscriptions(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<SinceParam>,
) -> Result<Response, StatusCode> {
    ensure_device(&state, auth_user.0.id).await?;
    let username = auth_user.0.username.clone();
    subscriptions::download_subscription_changes(
        State(state),
        axum::extract::Path((username, DEVICE_ID.to_string())),
        Extension(auth_user),
        Query(subscriptions::SinceQuery {
            since: with_slack(params.since),
        }),
    )
    .await
    .map(IntoResponse::into_response)
}

/// POST /index.php/apps/gpoddersync/subscription_change/create
pub async fn subscription_change(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<subscriptions::DeltaUploadRequest>,
) -> Result<Response, StatusCode> {
    ensure_device(&state, auth_user.0.id).await?;
    let username = auth_user.0.username.clone();
    subscriptions::upload_subscription_changes(
        State(state),
        axum::extract::Path((username, DEVICE_ID.to_string())),
        Extension(auth_user),
        Json(body),
    )
    .await
    .map(IntoResponse::into_response)
}

/// GET /index.php/apps/gpoddersync/episode_action?since=N
pub async fn episode_actions(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<SinceParam>,
) -> Result<Response, StatusCode> {
    let username = format!("{}.json", auth_user.0.username);
    episodes::download_episode_actions(
        State(state),
        axum::extract::Path(username),
        Extension(auth_user),
        Query(episodes::EpisodeQuery {
            since: with_slack(params.since),
            podcast: None,
            device: None,
            aggregated: false,
        }),
    )
    .await
    .map(IntoResponse::into_response)
}

/// POST /index.php/apps/gpoddersync/episode_action/create
///
/// Actions without a `device` are recorded for the gpoddersync device. Action
/// names are accepted in any case, as the Nextcloud app stores them uppercase.
pub async fn episode_action_create(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    raw_body: String,
) -> Result<Response, StatusCode> {
    let parsed: serde_json::Value =
        serde_json::from_str(&raw_body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut actions = match parsed {
        serde_json::Value::Array(actions) => actions,
        serde_json::Value::Object(mut obj) => match obj.remove("actions") {
            Some(serde_json::Value::Array(actions)) => actions,
            _ => return Err(StatusCode::BAD_REQUEST),
        },
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    for action in &mut actions {
        let Some(obj) = action.as_object_mut() else {
            continue;
        };
        if let Some(name) = obj.get("action").and_then(|a| a.as_str()) {
            let lower = name.to_ascii_lowercase();
            obj.insert("action".into(), lower.into());
        }
        if !obj.get("device").is_some_and(|d| d.is_string()) {
            obj.insert("device".into(), DEVICE_ID.into());
        }
    }
    ensure_device(&state, auth_user.0.id).await?;
    let username = format!("{}.json", auth_user.0.username);
    let body = serde_json::Value::Array(actions).to_string();
    episodes::upload_episode_actions(
        State(state),
        axum::extract::Path(username),
        Extension(auth_user),
        body,
    )
    .await
    .map(IntoResponse::into_response)
}
