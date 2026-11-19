use std::fmt::Write;
use rust_embed::{EmbeddedFile, RustEmbed};
use warp::http::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE, ETAG, IF_NONE_MATCH, VARY,
};
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use warp::path::Tail;
use warp::{Filter, Rejection};

const INDEX: &str = "index.html";
const GZIP: &str = "gzip";
/// What the frontend build appends to a file name for its gzip copy.
const GZIP_SUFFIX: &str = ".gz";

/// The dashboard bundle Vite writes to `frontend/dist`.
///
/// A release build compiles the files into the binary. A debug build reads them from disk
/// on every request, so a rebuilt dashboard shows up without recompiling the server.
#[derive(RustEmbed)]
#[folder = "frontend/dist"]
pub struct Dashboard;

/// Serves the dashboard's own files, with `/` answering `index.html`.
///
/// A client that accepts gzip gets the copy the frontend build compressed ahead of time,
/// so nothing is compressed here.
///
/// A path that is not part of the bundle is rejected, so the request moves on to the
/// routes that come after this one.
pub fn dashboard_routes() -> impl Filter<Extract = (Response<Body>,), Error = Rejection> + Clone {
    warp::get()
        .or(warp::head())
        .unify()
        .and(warp::path::tail())
        .and(warp::header::optional::<String>(IF_NONE_MATCH.as_str()))
        .and(warp::header::optional::<String>(ACCEPT_ENCODING.as_str()))
        .and_then(serve_asset)
}

async fn serve_asset(
    tail: Tail,
    if_none_match: Option<String>,
    accept_encoding: Option<String>,
) -> Result<Response<Body>, Rejection> {
    let name = match tail.as_str() {
        "" => INDEX,
        name => name,
    };
    let asset = Dashboard::get(name).ok_or_else(warp::reject::not_found)?;
    // A bundle built before the frontend wrote gzip copies has none, and is served as it is.
    let compressed = accept_encoding
        .filter(|accepted| accepts_gzip(accepted))
        .and_then(|_| Dashboard::get(&format!("{name}{GZIP_SUFFIX}")));
    let sent = compressed.as_ref().unwrap_or(&asset);
    let reply = Reply {
        content_type: asset.metadata.mimetype(),
        // Each variant has its own tag, so a cached copy is never revalidated as the other.
        etag: etag(sent),
        gzipped: compressed.is_some(),
    };

    if if_none_match.is_some_and(|known| matches_etag(&known, &reply.etag)) {
        return Ok(reply.with(StatusCode::NOT_MODIFIED, Body::empty()));
    }

    // Borrowed from the binary in a release build, so nothing is copied.
    let body = Body::from(sent.data.clone());

    Ok(reply.with(StatusCode::OK, body))
}

/// The headers every answer for a dashboard file carries, whatever its status.
struct Reply<'asset> {
    content_type: &'asset str,
    etag: String,
    gzipped: bool,
}

impl Reply<'_> {
    fn with(&self, status: StatusCode, body: Body) -> Response<Body> {
        let mut response = Response::builder()
            .status(status)
            .header(CONTENT_TYPE, self.content_type)
            .header(ETAG, &self.etag)
            .header(VARY, ACCEPT_ENCODING);

        if self.gzipped {
            response = response.header(CONTENT_ENCODING, GZIP);
        }

        response
            .body(body)
            .expect("the dashboard response headers are valid")
    }
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

/// Whether an `Accept-Encoding` header lists gzip without refusing it with `q=0`.
fn accepts_gzip(accept_encoding: &str) -> bool {
    accept_encoding
        .split(',')
        .any(|entry| {
            let mut parts = entry.split(';');
            let coding = parts.next().unwrap_or_default().trim();

            coding.eq_ignore_ascii_case(GZIP) && !parts.any(is_zero_weight)
        })
}

/// Whether a parameter of an `Accept-Encoding` entry is a weight of zero, such as `q=0.0`.
fn is_zero_weight(parameter: &str) -> bool {
    let parameter = parameter.trim();

    parameter
        .strip_prefix("q=")
        .or_else(|| parameter.strip_prefix("Q="))
        .map(str::trim)
        .is_some_and(|weight| weight.starts_with('0') && weight.chars().all(|digit| matches!(digit, '0' | '.')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_gzip_alone_or_in_a_list() {
        assert!(accepts_gzip("gzip"));
        assert!(accepts_gzip("deflate, gzip, br"));
        assert!(accepts_gzip("br;q=1.0, gzip;q=0.8"));
    }

    #[test]
    fn accepts_gzip_in_any_case() {
        assert!(accepts_gzip("GZIP"));
        assert!(accepts_gzip("deflate, Gzip ; Q=0.5"));
    }

    #[test]
    fn refuses_gzip_with_a_zero_weight() {
        assert!(!accepts_gzip("gzip;q=0"));
        assert!(!accepts_gzip("gzip; q=0.000, br"));
        assert!(!accepts_gzip("gzip;Q=0"));
    }

    #[test]
    fn refuses_a_header_without_gzip() {
        assert!(!accepts_gzip(""));
        assert!(!accepts_gzip("br, deflate"));
        assert!(!accepts_gzip("identity"));
        assert!(!accepts_gzip("x-gzip"));
    }
}
