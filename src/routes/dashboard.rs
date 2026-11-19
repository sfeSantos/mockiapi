use std::fmt::Write;
use rust_embed::{EmbeddedFile, RustEmbed};
use warp::http::header::{CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use warp::path::Tail;
use warp::{Filter, Rejection};

const INDEX: &str = "index.html";

/// The dashboard bundle Vite writes to `frontend/dist`.
///
/// A release build compiles the files into the binary. A debug build reads them from disk
/// on every request, so a rebuilt dashboard shows up without recompiling the server.
#[derive(RustEmbed)]
#[folder = "frontend/dist"]
pub struct Dashboard;

/// Serves the dashboard's own files, with `/` answering `index.html`.
///
/// A path that is not part of the bundle is rejected, so the request moves on to the
/// routes that come after this one.
pub fn dashboard_routes() -> impl Filter<Extract = (Response<Body>,), Error = Rejection> + Clone {
    warp::get()
        .or(warp::head())
        .unify()
        .and(warp::path::tail())
        .and(warp::header::optional::<String>(IF_NONE_MATCH.as_str()))
        .and_then(serve_asset)
}

async fn serve_asset(tail: Tail, if_none_match: Option<String>) -> Result<Response<Body>, Rejection> {
    let name = match tail.as_str() {
        "" => INDEX,
        name => name,
    };
    let asset = Dashboard::get(name).ok_or_else(warp::reject::not_found)?;
    let etag = etag(&asset);

    if if_none_match.is_some_and(|known| matches_etag(&known, &etag)) {
        return Ok(respond(StatusCode::NOT_MODIFIED, &asset, &etag, Body::empty()));
    }

    // Borrowed from the binary in a release build, so nothing is copied.
    let body = Body::from(asset.data.clone());

    Ok(respond(StatusCode::OK, &asset, &etag, body))
}

fn respond(status: StatusCode, asset: &EmbeddedFile, etag: &str, body: Body) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, asset.metadata.mimetype())
        .header(ETAG, etag)
        .body(body)
        .expect("the dashboard response headers are valid")
}

/// The file's SHA-256 as a quoted hex string: it changes exactly when the content does.
fn etag(asset: &EmbeddedFile) -> String {
    let hash = asset.metadata.sha256_hash();
    let mut etag = String::with_capacity(hash.len() * 2 + 2);
    etag.push('"');

    for byte in hash {
        let _ = write!(etag, "{byte:02x}");
    }

    etag.push('"');

    etag
}

/// Whether an `If-None-Match` header names this tag, alone or in a list.
fn matches_etag(if_none_match: &str, etag: &str) -> bool {
    if_none_match
        .split(',')
        .map(|candidate| candidate.trim().trim_start_matches("W/"))
        .any(|candidate| candidate == "*" || candidate == etag)
}
