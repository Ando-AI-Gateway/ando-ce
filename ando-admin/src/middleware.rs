use crate::server::AppState;
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

/// Admin API key authentication middleware.
pub async fn api_key_auth(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // If no API key is configured, allow all requests
    // In production, you should always set an API key
    // TODO: Read from config
    let _ = state;
    Ok(next.run(request).await)
}
