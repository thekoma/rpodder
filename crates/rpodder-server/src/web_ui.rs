//! Embedded web UI served from the Svelte build output.
//!
//! Only compiled when the `web-ui` feature is enabled.
//! Can be disabled at runtime with RPODDER_SERVE_UI=false.

use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist/"]
struct WebAssets;

/// Handler that serves embedded static files.
/// Falls through to index.html for SPA client-side routing.
pub async fn serve_ui(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches("/ui/").trim_start_matches("/ui");
    let path = if path.is_empty() { "index.html" } else { path };

    // Try exact file match first
    if let Some(file) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, cache_control(path))
            .body(Body::from(file.data.to_vec()))
            .unwrap();
    }

    // SPA fallback: serve index.html for client-side routes
    if let Some(file) = WebAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(file.data.to_vec()))
            .unwrap();
    }

    StatusCode::NOT_FOUND.into_response()
}

fn cache_control(path: &str) -> &'static str {
    if path.contains("immutable") || path.ends_with(".js") || path.ends_with(".css") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}
