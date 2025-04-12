use std::collections::HashMap;
use warp::{Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, Response, StatusCode};
use warp::path::FullPath;
use warp::hyper::body::Bytes;
use crate::handlers::graphql::process_graphql;
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

    // Read file content
    let json_file_content = match tokio::fs::read_to_string(&endpoint.file).await {
        Ok(content) => content,
        Err(_) => return Err(warp::reject::custom(NotFound)),
    };

    // Handle GraphQL simulation if query is detected
    if let Some(ref body_bytes) = body {
        if let Ok(body_str) = std::str::from_utf8(body_bytes) {
            if body_str.contains("\"query\"") {
                if let Ok(Some(gql_data)) = process_graphql(body_str, &json_file_content) {
                    let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
                        .unwrap_or(StatusCode::OK);
                    let response: Response<String> = Response::builder()
                        .status(status_code)
                        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                        .body(gql_data.into())
                        .unwrap();
                    return Ok(response);
                }
            }
        }
    }

    // Default path if not a GraphQL request
    let response_body = maybe_replace_variables(json_file_content, &endpoint, &full_url, body);
    let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
        .unwrap_or(StatusCode::NOT_FOUND);

    let response = Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(response_body.into())
        .unwrap();


    Ok(response)
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