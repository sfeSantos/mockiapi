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
use crate::models::{Endpoints, NotFound, Unauthorized};
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
    
    let endpoint_data = {
        let endpoints_map = endpoints.lock().await;
        endpoints_map.get(&full_url).cloned()
    };

    if let Some(endpoint) = endpoint_data {
        if let Some(auth) = &endpoint.authentication {
            if !validate_auth(Some(auth.clone()), auth_header) {
                return Err(warp::reject::custom(Unauthorized));
            }
        }

        check_rate_limit(path.as_str().to_string(), "GET", endpoint.rate_limit.as_ref(), rate_limiter.clone()).await?;

        if endpoint.delay.is_some() {
            add_possible_delay(&endpoint).await;
        }

        match tokio::fs::read_to_string(&endpoint.file).await {
            Ok(mut data) => {
                // Apply dynamic variable replacement if the flag is true
                if endpoint.with_dynamic_vars.unwrap_or(false) {
                    let params: HashMap<String, String>;
                    
                    if let Some(body) = body {
                        params = get_body_from_request(body);
                    } else {
                        params = get_params_from_request(full_url.as_str());
                    }
                    
                    data = dynamic_vars::replace_variables(&data, &params);
                }

                let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
                    .unwrap_or(StatusCode::NOT_FOUND);
                let response = reply::with_status(data, status_code);
                let response = reply::with_header(response, CONTENT_TYPE, "application/json");

                Ok(response)
            }
            Err(_) => Err(warp::reject::custom(NotFound)),
        }
    } else {
        Err(warp::reject::custom(NotFound))
    }
}

/// A helper function to extract parameters (example: from query or path)
fn get_params_from_request(path: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    let full_url = format!("http://localhost:3001{}", path);
    let url = Url::parse(&full_url).expect("Failed to parse URL");

    for (key, value) in url.query_pairs() {
        params.insert(key.to_string(), value.to_string());
    }

    // Regex to match versioning patterns (e.g., "v1", "v2", "v10", etc.)
    let version_regex = Regex::new(r"^v\d+$").unwrap();

    // Extract path segments, filtering out "api" and version segments
    let segments: Vec<&str> = url
        .path()
        .split('/')
        .filter(|s| !s.is_empty() && *s != "api" && !version_regex.is_match(s))
        .collect();

    let mut iter = segments.iter();
    while let (Some(key), Some(value)) = (iter.next(), iter.next()) {
        params.insert(key.to_string(), value.to_string());
    }

    params
}

fn get_body_from_request(body: Bytes) -> HashMap<String, String> {
    let mut params = HashMap::new();
    
    if let Ok(json) = serde_json::from_slice::<Value>(&body) {
        if let Some(obj) = json.as_object() {
            for (key, value) in obj {
                if let Some(val_str) = value.as_str() {
                    params.insert(key.clone(), val_str.to_string());
                } else {
                    params.insert(key.clone(), value.to_string());
                }
            }
        }
    }
    params
}