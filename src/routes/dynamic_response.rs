use std::collections::HashMap;
use std::sync::Arc;
use warp::{Rejection, Reply};
use warp::http::header::CONTENT_TYPE;
use warp::http::{HeaderValue, Method, Response, StatusCode};
use warp::path::FullPath;
use warp::hyper::body::Bytes;
use crate::handlers::graphql::{handle_graphql};
use crate::handlers::grpc::{handle_grpc};
use crate::handlers::params::get_body_from_request;
use crate::middlewares::authentication::{validate_auth};
use crate::middlewares::dynamic_vars;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{EndpointKind, Endpoints, GrpcMockNotFound, InvalidGraphQLRequest, MethodNotAllowed, NotFound, Unauthorized};
use crate::middlewares::rate_limit::{check_rate_limit, RateLimitTracker};
use crate::routing::Route;
use crate::utils::add_possible_delay;

pub async fn serve_dynamic_response(
    method: Method,
    path: FullPath,
    query: String,
    auth_header: Option<String>,
    endpoints: Endpoints,
    rate_limiter: RateLimitTracker,
    body: Option<Bytes>,
    grpc_registry: Arc<GrpcRegistry>,
) -> Result<impl Reply, Rejection> {
    let route = endpoints.lock()
        .await
        .find(path.as_str(), &query)
        .ok_or_else(|| warp::reject::custom(NotFound))?;
    let endpoint = &route.endpoint;

    if !endpoint.method.iter().any(|allowed| allowed.eq_ignore_ascii_case(method.as_str())) {
        return Err(warp::reject::custom(MethodNotAllowed));
    }

    if let Some(auth) = &endpoint.authentication {
        if !validate_auth(Some(auth.clone()), auth_header) {
            return Err(warp::reject::custom(Unauthorized));
        }
    }

    // The limit belongs to the registered route, whatever values fill its placeholders.
    check_rate_limit(route.path.clone(), method.as_str(), endpoint.rate_limit.as_ref(), rate_limiter.clone()).await?;

    // A gRPC mock carries its own delay, which the gRPC handler applies.
    if endpoint.kind != EndpointKind::Grpc {
        add_possible_delay(endpoint).await;
    }

    // Read file content
    let json_file_content = match tokio::fs::read_to_string(&endpoint.file).await {
        Ok(content) => content,
        Err(_) => return Err(warp::reject::custom(NotFound)),
    };

    // The protocol is known since registration, so the body is never sniffed.
    let response = match endpoint.kind {
        EndpointKind::GraphQl => handle_graphql(body_as_str(&body), endpoint, &json_file_content)
            .ok_or_else(|| warp::reject::custom(InvalidGraphQLRequest))?,
        EndpointKind::Grpc => handle_grpc(body_as_str(&body), grpc_registry)
            .await
            .ok_or_else(|| warp::reject::custom(GrpcMockNotFound))?,
        EndpointKind::Rest => rest_response(json_file_content, &route, path.as_str(), &query, body),
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
    route: &Route,
    path: &str,
    query: &str,
    body: Option<Bytes>,
) -> Response<String> {
    let endpoint = &route.endpoint;
    let response_body = if endpoint.with_dynamic_vars.unwrap_or(false) {
        dynamic_vars::replace_variables(&json_file_content, &request_variables(route, path, query, body))
    } else {
        json_file_content
    };
    let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
        .unwrap_or(StatusCode::NOT_FOUND);

    Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(response_body)
        .unwrap()
}

/// Gathers the values a response template can refer to: path placeholders, query
/// parameters and the fields of a JSON body. A later source overrides an earlier one.
fn request_variables(
    route: &Route,
    path: &str,
    query: &str,
    body: Option<Bytes>,
) -> HashMap<String, String> {
    let mut variables = HashMap::new();
    route.pattern.capture(path, query, &mut variables);

    if let Some(body) = body {
        get_body_from_request(&body, &mut variables);
    }

    variables
}
