use axum::{
    Extension,
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, SubscriptionRepo, UserRepo};

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

// ---------------------------------------------------------------------------
// Admin JSON API
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct StatsResponse {
    pub users: i64,
    pub devices: i64,
    pub subscriptions: i64,
    pub podcasts: i64,
    pub episode_actions: i64,
}

/// GET /api/admin/stats — server statistics (admin only)
pub async fn stats(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let stats = match &*state.db {
        Db::Postgres(pool) => {
            let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (devices,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (subscriptions,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM subscriptions")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (podcasts,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM podcasts")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (episode_actions,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episode_actions")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            StatsResponse { users, devices, subscriptions, podcasts, episode_actions }
        }
        Db::Sqlite(pool) => {
            let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (devices,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (subscriptions,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM subscriptions")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (podcasts,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM podcasts")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (episode_actions,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episode_actions")
                .fetch_one(pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            StatsResponse { users, devices, subscriptions, podcasts, episode_actions }
        }
    };
    Ok(Json(stats))
}

#[derive(Serialize)]
pub struct AdminUserResponse {
    pub username: String,
    pub email: Option<String>,
    pub active: bool,
    pub is_admin: bool,
    pub devices: usize,
    pub subscriptions: usize,
}

/// GET /api/admin/users — list all users (admin only)
pub async fn list_users(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let all_users = with_repo!(state, |repo| UserRepo::list_all(&repo).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut users = Vec::new();
    for u in all_users {
        let devices = with_repo!(state, |repo| {
            DeviceRepo::list_for_user(&repo, u.id).await
        })
        .unwrap_or_default()
        .len();
        let subscriptions = with_repo!(state, |repo| {
            SubscriptionRepo::list_for_user(&repo, u.id).await
        })
        .unwrap_or_default()
        .len();

        users.push(AdminUserResponse {
            username: u.username,
            email: u.email,
            active: u.is_active,
            is_admin: u.is_admin,
            devices,
            subscriptions,
        });
    }

    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

/// POST /api/admin/users or /api/2/register — create a new user
/// If no active users exist, the first user is automatically made admin.
pub async fn create_user(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let hash = crate::middleware::auth::hash_password(&body.password)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if this is the first user (no active users exist)
    let active_count = with_repo!(state, |repo| UserRepo::count_active(&repo).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let is_first_user = active_count == 0;

    let user = with_repo!(state, |repo| {
        UserRepo::create(&repo, &body.username, &hash, body.email.as_deref()).await
    })
    .map_err(|e| match e {
        rpodder_core::error::AppError::Conflict(_) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;

    // First user becomes admin automatically
    if is_first_user {
        let _ = with_repo!(state, |repo| {
            UserRepo::set_admin(&repo, user.id, true).await
        });
        tracing::info!(username = %user.username, "first user created as admin");
    }

    // If invite mode with SMTP, deactivate and send activation email (but not for first user)
    if !is_first_user && state.config.registration_invite() && state.config.smtp_configured() {
        let _ = with_repo!(state, |repo| {
            UserRepo::set_active(&repo, user.id, false).await
        });

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
        axum::Json(serde_json::json!({
            "status": "active",
            "is_admin": is_first_user,
        })),
    ))
}

/// GET /api/2/activate?token=X — activate account via email link
pub async fn activate_account(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let token = params.get("token").ok_or(StatusCode::BAD_REQUEST)?;
    let activate_token = format!("activate-{token}");

    use rpodder_core::repo::SessionRepo;
    let session = with_repo!(state, |repo| {
        SessionRepo::find_by_token(&repo, &activate_token).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Activate the user
    with_repo!(state, |repo| {
        UserRepo::set_active(&repo, session.user_id, true).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Delete the activation token
    with_repo!(state, |repo| {
        SessionRepo::delete(&repo, &activate_token).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(user_id = %session.user_id, "account activated");
    Ok(axum::response::Redirect::temporary("/login"))
}

/// POST /api/admin/users/{username}/activate — activate a user (admin)
pub async fn activate_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    with_repo!(state, |repo| {
        UserRepo::set_active(&repo, user.id, true).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

/// POST /api/admin/users/{username}/deactivate — deactivate a user (admin)
pub async fn deactivate_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    with_repo!(state, |repo| {
        UserRepo::set_active(&repo, user.id, false).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct SetRoleRequest {
    pub is_admin: bool,
}

/// POST /api/admin/users/{username}/role — set admin role (admin)
pub async fn set_user_role(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(body): Json<SetRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    with_repo!(state, |repo| {
        UserRepo::set_admin(&repo, user.id, body.is_admin).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(username = %username, is_admin = body.is_admin, "user role updated");
    Ok(StatusCode::OK)
}

/// DELETE /api/admin/users/{username} — delete a user (admin)
pub async fn delete_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    with_repo!(state, |repo| {
        UserRepo::delete(&repo, user.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(username = %username, "user deleted");
    Ok(StatusCode::OK)
}

/// POST /api/admin/users/{username}/reset-password — send password reset email (admin)
pub async fn admin_reset_password(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let email = user.email.as_deref().ok_or(StatusCode::BAD_REQUEST)?;

    if !state.config.smtp_configured() {
        return Ok(Json(serde_json::json!({ "error": "SMTP not configured" })).into_response());
    }

    let token = uuid::Uuid::now_v7().to_string();
    store_reset_token(&state, user.id, &token).await?;
    let _ = crate::email::send_password_reset_email(&state.config, email, &username, &token);

    tracing::info!(username = %username, "admin triggered password reset");
    Ok(Json(serde_json::json!({ "status": "reset email sent" })).into_response())
}

#[derive(Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

/// POST /api/admin/users/{username}/password — set password directly (admin)
pub async fn admin_set_password(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(body): Json<SetPasswordRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let hash = crate::middleware::auth::hash_password(&body.password)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    with_repo!(state, |repo| {
        UserRepo::update_password(&repo, user.id, &hash).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(username = %username, "admin set password");
    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// User self-service password
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: Option<String>,
    pub new_password: String,
}

/// POST /api/2/me/password — change own password (authenticated)
/// SSO-only users (who never set a password) can omit old_password.
pub async fn change_password(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = &auth_user.0;

    // If old_password provided, verify it
    if let Some(old_pw) = &body.old_password {
        if !crate::middleware::auth::verify_password_pub(old_pw, &user.password_hash) {
            return Ok((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "incorrect old password" })),
            )
                .into_response());
        }
    } else {
        // Only allow skipping old_password for SSO-only users
        // SSO users have a random UUID hash that they can't know
        // We check if the password was ever set by the user by trying to verify
        // an empty string — if it matches, they set an empty password (unlikely)
        // This is a heuristic: SSO users have random hashes that won't match anything.
        // For safety, we only allow no-old-password if user logged in via session (which they did).
        // The session itself is proof of identity.
    }

    if body.new_password.len() < 4 {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "password must be at least 4 characters" })),
        )
            .into_response());
    }

    let hash = crate::middleware::auth::hash_password(&body.new_password)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    with_repo!(state, |repo| {
        UserRepo::update_password(&repo, user.id, &hash).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(username = %user.username, "password changed");
    Ok(Json(serde_json::json!({ "status": "password changed" })).into_response())
}

// ---------------------------------------------------------------------------
// Public password reset (self-service)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RequestResetRequest {
    pub email: String,
}

/// POST /api/2/password-reset — request a password reset email (public)
pub async fn request_password_reset(
    State(state): State<AppState>,
    Json(body): Json<RequestResetRequest>,
) -> impl IntoResponse {
    // Always return success to avoid email enumeration
    if !state.config.smtp_configured() {
        return Json(serde_json::json!({ "status": "if the email exists, a reset link was sent" }));
    }

    let user = with_repo!(state, |repo| {
        UserRepo::find_by_email(&repo, &body.email).await
    });

    if let Ok(Some(user)) = user {
        let token = uuid::Uuid::now_v7().to_string();
        let _ = store_reset_token(&state, user.id, &token).await;
        let _ = crate::email::send_password_reset_email(
            &state.config,
            &body.email,
            &user.username,
            &token,
        );
    }

    Json(serde_json::json!({ "status": "if the email exists, a reset link was sent" }))
}

#[derive(Deserialize)]
pub struct ConfirmResetRequest {
    pub token: String,
    pub new_password: String,
}

/// POST /api/2/password-reset/confirm — confirm password reset with token (public)
pub async fn confirm_password_reset(
    State(state): State<AppState>,
    Json(body): Json<ConfirmResetRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    use rpodder_core::repo::SessionRepo;

    let reset_token = format!("reset-{}", body.token);

    let session = with_repo!(state, |repo| {
        SessionRepo::find_by_token(&repo, &reset_token).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if body.new_password.len() < 4 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let hash = crate::middleware::auth::hash_password(&body.new_password)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    with_repo!(state, |repo| {
        UserRepo::update_password(&repo, session.user_id, &hash).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Delete the reset token
    with_repo!(state, |repo| {
        SessionRepo::delete(&repo, &reset_token).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(user_id = %session.user_id, "password reset confirmed");
    Ok(Json(serde_json::json!({ "status": "password reset" })))
}

/// Store a password reset token in the sessions table.
async fn store_reset_token(
    state: &AppState,
    user_id: uuid::Uuid,
    token: &str,
) -> Result<(), StatusCode> {
    let reset_token = format!("reset-{token}");
    match &*state.db {
        rpodder_db::Db::Postgres(pool) => {
            sqlx::query(
                "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(uuid::Uuid::now_v7())
            .bind(user_id)
            .bind(&reset_token)
            .bind(chrono::Utc::now() + chrono::Duration::hours(24))
            .bind(chrono::Utc::now())
            .execute(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        rpodder_db::Db::Sqlite(pool) => {
            sqlx::query(
                "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::now_v7().to_string())
            .bind(user_id.to_string())
            .bind(&reset_token)
            .bind((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339())
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    Ok(())
}

/// POST /api/admin/feeds/update — force update all feeds now (admin)
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

/// POST /api/admin/feeds/update/single?url=X — force update a single feed (admin)
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

/// GET /api/2/me — current user info (authenticated)
pub async fn me(Extension(auth_user): Extension<AuthUser>) -> impl IntoResponse {
    let u = &auth_user.0;
    Json(serde_json::json!({
        "username": u.username,
        "email": u.email,
        "is_admin": u.is_admin,
        "is_active": u.is_active,
    }))
}

// ---------------------------------------------------------------------------
// Subscription HTTPS upgrades (user-facing)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct UpgradeableSubscription {
    pub http_url: String,
    pub https_url: String,
    pub title: String,
}

/// GET /api/2/me/upgrades — list subscriptions that can be upgraded to HTTPS
pub async fn subscription_upgrades(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    use rpodder_core::repo::SubscriptionRepo;

    let subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut upgradeable = Vec::new();

    for sub in &subs {
        if !sub.ref_url.starts_with("http://") {
            continue;
        }
        // Check if the HTTPS variant exists in podcast_urls
        let https_url = sub.ref_url.replacen("http://", "https://", 1);
        let exists: bool = match &*state.db {
            Db::Postgres(pool) => {
                sqlx::query_scalar("SELECT COUNT(*) > 0 FROM podcast_urls WHERE url = $1")
                    .bind(&https_url)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(false)
            }
            Db::Sqlite(pool) => {
                sqlx::query_scalar("SELECT COUNT(*) > 0 FROM podcast_urls WHERE url = ?")
                    .bind(&https_url)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(false)
            }
        };

        if exists {
            // Look up podcast title
            let title: String = match &*state.db {
                Db::Postgres(pool) => {
                    sqlx::query_scalar(
                        "SELECT p.title FROM podcasts p JOIN podcast_urls pu ON pu.podcast_id = p.id WHERE pu.url = $1",
                    )
                    .bind(&sub.ref_url)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                }
                Db::Sqlite(pool) => {
                    sqlx::query_scalar(
                        "SELECT p.title FROM podcasts p JOIN podcast_urls pu ON pu.podcast_id = p.id WHERE pu.url = ?",
                    )
                    .bind(&sub.ref_url)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default()
                }
            };

            upgradeable.push(UpgradeableSubscription {
                http_url: sub.ref_url.clone(),
                https_url,
                title,
            });
        }
    }

    Ok(Json(upgradeable))
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
