use std::sync::Arc;
use bytes::Bytes;
use warp::http::{Method, Response};
use warp::hyper::Body;
use warp::path::FullPath;
use crate::handlers::{graphql, grpc, rest};
use crate::middlewares::authentication::validate_auth;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{Endpoints, MockBody, MockError};
use crate::utils::add_possible_delay;

/// Answers any request with the mock registered for its path, or with the reason it
/// could not be served.
pub async fn serve_dynamic_response(
    method: Method,
    path: FullPath,
    query: String,
    auth_header: Option<String>,
    body: Bytes,
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> Response<Body> {
    respond(method, path.as_str(), &query, auth_header.as_deref(), &body, &endpoints, &grpc_registry)
        .await
        .unwrap_or_else(MockError::into_response)
}

async fn respond(
    method: Method,
    path: &str,
    query: &str,
    auth_header: Option<&str>,
    body: &[u8],
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<Response<Body>, MockError> {
    let route = endpoints.find(path, query, &method)?;
    let endpoint = &route.endpoint;

    if let Some(expected) = &endpoint.authentication
        && !validate_auth(expected, auth_header)
    {
        return Err(MockError::Unauthorized);
    }

    // The limit belongs to the registered route, whatever values fill its placeholders.
    if let Some(limiter) = &route.limiter
        && !limiter.try_acquire(&method)
    {
        return Err(MockError::RateLimited);
    }

    // The protocol is known since registration, so the body is never sniffed.
    match &endpoint.body {
        MockBody::Raw(content) => {
            add_possible_delay(endpoint).await;

            Ok(rest::respond(&route, content, path, query, body))
        }
        MockBody::GraphQl(mock) => {
            add_possible_delay(endpoint).await;

            graphql::respond(endpoint.status(), mock, body)
        }
        // A gRPC mock carries its own delay, which the gRPC handler applies.
        MockBody::Grpc { .. } => grpc::respond(body, grpc_registry).await,
    }
}
