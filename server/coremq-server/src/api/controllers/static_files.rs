use axum::{body::Body, extract::Request, http::{header, StatusCode}, response::{IntoResponse, Response}};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../client/dist"]
struct Assets;

/// Serves embedded React SPA assets.
/// Tries the exact path first; falls back to index.html for client-side routing.
pub async fn spa_handler(req: Request) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');

    // Exact file match (JS, CSS, images, etc.)
    if !path.is_empty() {
        if let Some(file) = Assets::get(path) {
            return serve_embedded(path, file);
        }
    }

    // SPA fallback — let React Router handle the path
    match Assets::get("index.html") {
        Some(file) => serve_embedded("index.html", file),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Frontend not built. Run: cd client && yarn build",
        )
            .into_response(),
    }
}

fn serve_embedded(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(file.data))
        .unwrap()
}
