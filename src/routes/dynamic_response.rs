use std::collections::HashMap;
use std::sync::Arc;
use warp::{Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, Method, Response, StatusCode};
use warp::path::FullPath;
use warp::hyper::body::Bytes;
use crate::handlers::graphql::{handle_graphql};
use crate::handlers::grpc::{handle_grpc};
use crate::handlers::params::{get_body_from_request, get_params_from_request};
use crate::middlewares::authentication::{validate_auth};
use crate::middlewares::dynamic_vars;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{Endpoint, EndpointKind, Endpoints, GrpcMockNotFound, InvalidGraphQLRequest, MethodNotAllowed, NotFound, Unauthorized};
use crate::middlewares::rate_limit::{check_rate_limit, RateLimitTracker};
use crate::utils::{add_possible_delay, reconstruct_full_url};

pub async fn serve_dynamic_response(
    method: Method,
    path: FullPath,
    query_params: Option<HashMap<String, String>>,
    auth_header: Option<String>,
    endpoints: Endpoints,
    rate_limiter: RateLimitTracker,
    body: Option<Bytes>,
    grpc_registry: Arc<GrpcRegistry>,
) -> Result<impl Reply, Rejection> {
    let full_url = reconstruct_full_url(path.as_str(), &query_params);

    let endpoint = {
        let endpoints_map = endpoints.lock().await;
        endpoints_map.get(&full_url).cloned()
    }.ok_or_else(|| warp::reject::custom(NotFound))?;

    if !endpoint.method.iter().any(|m| m.eq_ignore_ascii_case(method.as_str())) {
        return Err(warp::reject::custom(MethodNotAllowed));
    }

    if let Some(auth) = &endpoint.authentication {
        if !validate_auth(Some(auth.clone()), auth_header) {
            return Err(warp::reject::custom(Unauthorized));
        }
    }

    check_rate_limit(path.as_str().to_string(), method.as_str(), endpoint.rate_limit.as_ref(), rate_limiter.clone()).await?;

    if let Some(_) = endpoint.delay {
        add_possible_delay(&endpoint).await;
    }

    // Read file content
    let json_file_content = match tokio::fs::read_to_string(&endpoint.file).await {
        Ok(content) => content,
        Err(_) => return Err(warp::reject::custom(NotFound)),
    };

    // The protocol is known since registration, so the body is never sniffed.
    let response = match endpoint.kind {
        EndpointKind::GraphQl => handle_graphql(body_as_str(&body), &endpoint, &json_file_content)
            .ok_or_else(|| warp::reject::custom(InvalidGraphQLRequest))?,
        EndpointKind::Grpc => handle_grpc(body_as_str(&body), grpc_registry)
            .await
            .ok_or_else(|| warp::reject::custom(GrpcMockNotFound))?,
        EndpointKind::Rest => rest_response(json_file_content, &endpoint, &full_url, body),
    };

    Ok(response)
}

fn body_as_str(body: &Option<Bytes>) -> &str {
    body.as_deref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or_default()
}

fn rest_response(
    json_file_content: String,
    endpoint: &Endpoint,
    full_url: &str,
    body: Option<Bytes>,
) -> Response<String> {
    let response_body = maybe_replace_variables(json_file_content, endpoint, full_url, body);
    let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
        .unwrap_or(StatusCode::NOT_FOUND);

    Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(response_body)
        .unwrap()
}

fn maybe_replace_variables(
    mut data: String,
    endpoint: &Endpoint,
    full_url: &str,
    body: Option<Bytes>,
) -> String {
    if endpoint.with_dynamic_vars.unwrap_or(false) {
        // A request without payload still carries an empty body.
        let params = match body.filter(|bytes| !bytes.is_empty()) {
            Some(body) => get_body_from_request(body),
            None => get_params_from_request(full_url),
        };
        data = dynamic_vars::replace_variables(&data, &params);
    }
    data
}

