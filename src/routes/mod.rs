use std::collections::HashMap;
use std::sync::Arc;
use warp::http::header::AUTHORIZATION;
use warp::{Filter, Rejection, Reply};
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::middlewares::rate_limit::RateLimitTracker;
use crate::models::Endpoints;
use crate::routes::dynamic_response::serve_dynamic_response;
use crate::routes::endpoints::with_endpoints;
use crate::utils::{handle_rejection, with_rate_limiter};

pub mod endpoints;
pub mod dynamic_response;

/// Catch-all filter that answers every registered mock endpoint.
pub fn mock_routes(
    endpoints: Endpoints,
    rate_limiter: RateLimitTracker,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let optional_query = warp::query::<HashMap<String, String>>()
        .map(Some)
        .or(warp::any().map(|| None))
        .unify();
    let optional_body = warp::body::bytes()
        .map(Some)
        .or(warp::any().map(|| None))
        .unify();

    warp::method()
        .and(warp::path::full())
        .and(optional_query)
        .and(warp::header::optional::<String>(AUTHORIZATION.as_str()))
        .and(with_endpoints(endpoints))
        .and(with_rate_limiter(rate_limiter))
        .and(optional_body)
        .and(warp::any().map(move || Arc::clone(&grpc_registry)))
        .and_then(serve_dynamic_response)
        .recover(handle_rejection)
}
