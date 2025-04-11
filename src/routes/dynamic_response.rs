use std::collections::HashMap;
use warp::{reply, Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::StatusCode;
use warp::path::FullPath;
use warp::hyper::body::Bytes;
use crate::handlers::params::{get_body_from_request, get_params_from_request};
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