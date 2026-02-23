use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

/// Embeds the pre-built Next.js static export at compile time.
/// Build the dashboard first: `cd dashboard && npm run build`
#[derive(Embed)]
#[folder = "../dashboard/out/"]
#[prefix = ""]
struct DashboardAssets;

/// Guess a MIME type from file extension.
fn mime_from_ext(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Serve an embedded file by path, or 404.
fn serve_embedded(path: &str) -> Response {
    match DashboardAssets::get(path) {
        Some(file) => {
            let mime = mime_from_ext(path);
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, if path.contains("/_next/") {
                        "public, max-age=31536000, immutable"
                    } else {
                        "no-cache"
                    }),
                ],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

/// `GET /dashboard` — serve the overview page.
pub async fn dashboard_index() -> impl IntoResponse {
    serve_embedded("index.html")
}

/// `GET /dashboard/{*path}` — serve dashboard sub-pages and assets.
///
/// Routing logic:
/// - Static assets (`_next/...`, files with extensions) → serve directly
/// - Page routes (`routes/`, `upstreams/`, etc.) → serve `{page}/index.html`
/// - Trailing-slash root (`/dashboard/`) → serve `index.html`
pub async fn dashboard_assets(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');

    // Empty path (trailing slash on /dashboard/) → index
    if path.is_empty() {
        return serve_embedded("index.html");
    }

    // Static assets — serve file directly
    if path.starts_with("_next/") || path.contains('.') {
        return serve_embedded(path);
    }

    // Page route — try {path}/index.html (trailingSlash: true in next.config)
    let trimmed = path.trim_end_matches('/');
    let page_path = format!("{trimmed}/index.html");
    serve_embedded(&page_path)
}
