use axum::response::{Html, IntoResponse};

/// Serve the built-in admin dashboard (single-page app).
///
/// The HTML is embedded at compile time via `include_str!` so the binary
/// remains self-contained — no external files or asset server needed.
pub async fn dashboard() -> impl IntoResponse {
    Html(include_str!("../dashboard.html"))
}
