use std::collections::HashMap;
use regex::Regex;
use warp::{reply, Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::StatusCode;
use warp::path::FullPath;
use url::Url;
use crate::middlewares::authentication::{validate_auth};
use crate::middlewares::dynamic_vars;
use crate::models::{Endpoints, NotFound, Unauthorized};
use crate::middlewares::rate_limit::{check_rate_limit, RateLimitTracker};
use crate::utils::add_possible_delay;

pub async fn serve_dynamic_response(
    path: FullPath,
    auth_header: Option<String>,
    endpoints: Endpoints,
    rate_limiter: RateLimitTracker,
) -> Result<impl Reply, Rejection> {
    let endpoint_data = {
        let endpoints_map = endpoints.lock().await;
        endpoints_map.get(path.as_str()).cloned()
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
                if endpoint.with_dynamic_vars.unwrap() {
                    let params = get_params_from_request(&path);
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
fn get_params_from_request(path: &FullPath) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Extract query parameters (e.g., ?key=value)
    if let Ok(url) = Url::parse(&format!("http://localhost:3001{}", path.as_str())) {
        for (key, value) in url.query_pairs() {
            params.insert(key.to_string(), value.to_string());
        }
    }

    // Regex for extracting path parameters (e.g., /api/user/{id}/item/{itemId})
    // We expect the path pattern to have dynamic parts like {id}, {itemId}
    let re = Regex::new(r"\{(\w+)}").unwrap();
    let path_segments: Vec<&str> = path.as_str().split('/').collect();

    // Iterate through the regex matches and extract path parameters
    let path_iter = path_segments.iter();
    for (i, segment) in path_iter.enumerate() {
        if let Some(caps) = re.captures(segment) {
            let param_name = &caps[1];
            if let Some(value) = path_segments.get(i + 1) {
                params.insert(param_name.to_string(), value.to_string());
            }
        }
    }
    
    params
}