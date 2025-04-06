use std::collections::HashMap;
use regex::Regex;
use serde_json::Value;
use warp::{reply, Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::StatusCode;
use warp::path::FullPath;
use warp::hyper::body::Bytes;
use url::Url;
use crate::middlewares::authentication::{validate_auth};
use crate::middlewares::dynamic_vars;
use crate::models::{Endpoint, Endpoints, NotFound, Unauthorized};
use crate::middlewares::rate_limit::{check_rate_limit, RateLimitTracker};
use crate::utils::{add_possible_delay, reconstruct_full_url};

pub async fn serve_dynamic_response(
    path: FullPath,
    query_params: Option<HashMap<String, String>>,
    auth_header: Option<String>,
    endpoints: Endpoints,
    rate_limiter: RateLimitTracker,
    body: Option<Bytes>,
) -> Result<impl Reply, Rejection> {
    let full_url = reconstruct_full_url(path.as_str(), &query_params);

    let endpoint = {
        let endpoints_map = endpoints.lock().await;
        endpoints_map.get(&full_url).cloned()
    }.ok_or_else(|| warp::reject::custom(NotFound))?;

    if let Some(auth) = &endpoint.authentication {
        if !validate_auth(Some(auth.clone()), auth_header) {
            return Err(warp::reject::custom(Unauthorized));
        }
    }

    check_rate_limit(path.as_str().to_string(), "GET", endpoint.rate_limit.as_ref(), rate_limiter.clone()).await?;

    if let Some(_) = endpoint.delay {
        add_possible_delay(&endpoint).await;
    }

    let response_body = match tokio::fs::read_to_string(&endpoint.file).await {
        Ok(data) => maybe_replace_variables(data, &endpoint, &full_url, body),
        Err(_) => return Err(warp::reject::custom(NotFound)),
    };

    let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
        .unwrap_or(StatusCode::NOT_FOUND);

    let response = reply::with_status(response_body, status_code);
    Ok(reply::with_header(response, CONTENT_TYPE, "application/json"))
}

/// A helper function to extract parameters (example: from query or path)
fn get_params_from_request(path: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Build full URL from incoming path
    let base_url = format!("http://localhost:3001{}", path);
    let url = match Url::parse(&base_url) {
        Ok(url) => url,
        Err(_) => return params, // Return empty if parsing fails
    };

    // Extract query parameters (e.g. ?key=value)
    for (key, value) in url.query_pairs() {
        params.insert(key.to_string(), value.to_string());
    }

    // Extract path parameters, ignoring 'api' and version segments (e.g., 'v1')
    let version_regex = Regex::new(r"^v\d+$").unwrap();
    let path_segments: Vec<&str> = url
        .path_segments()
        .unwrap()
        .filter(|segment| !segment.is_empty() && *segment != "api" && !version_regex.is_match(segment))
        .collect();

    // Assume key/value pairs in the remaining path segments
    let mut segment_iter = path_segments.iter();
    while let (Some(key), Some(value)) = (segment_iter.next(), segment_iter.next()) {
        params.insert((*key).to_string(), (*value).to_string());
    }

    params
}

fn get_body_from_request(body: Bytes) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Try to parse the body as JSON
    let Ok(json) = serde_json::from_slice::<Value>(&body) else {
        return params;
    };

    // Extract key-value pairs from a JSON object
    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            let value_str = value.as_str()
                .map(String::from)
                .unwrap_or_else(|| value.to_string());

            params.insert(key.to_owned(), value_str);
        }
    }

    params
}

fn maybe_replace_variables(
    mut data: String,
    endpoint: &Endpoint,
    full_url: &str,
    body: Option<Bytes>,
) -> String {
    if endpoint.with_dynamic_vars.unwrap_or(false) {
        let params = match body {
            Some(body) => get_body_from_request(body),
            None => get_params_from_request(full_url),
        };
        data = dynamic_vars::replace_variables(&data, &params);
    }
    data
}