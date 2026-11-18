use std::convert::Infallible;
use std::sync::Arc;
use warp::http::header::AUTHORIZATION;
use warp::{Filter, Rejection, Reply};
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::Endpoints;
use crate::routes::dynamic_response::serve_dynamic_response;
use crate::routes::endpoints::{delete_endpoint, list_endpoint, register_endpoint, with_endpoints};
use crate::utils::handle_rejection;

pub mod endpoints;
pub mod dynamic_response;

const MAX_UPLOAD_BYTES: u64 = 5_000_000;

/// Routes the dashboard uses to register, list and delete mock endpoints.
pub fn admin_routes(
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let register = warp::post()
        .and(warp::path!("register"))
        .and(warp::multipart::form().max_length(MAX_UPLOAD_BYTES))
        .and(with_endpoints(endpoints.clone()))
        .and(with_grpc_registry(Arc::clone(&grpc_registry)))
        .then(register_endpoint);
    let list = warp::get()
        .and(warp::path!("list"))
        .and(with_endpoints(endpoints.clone()))
        .then(list_endpoint);
    let delete = warp::delete()
        .and(warp::path!("delete" / String))
        .and(with_endpoints(endpoints))
        .and(with_grpc_registry(grpc_registry))
        .then(delete_endpoint);

    register.or(list).or(delete)
}

/// Catch-all filter that answers every registered mock endpoint.
pub fn mock_routes(
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let raw_query = warp::query::raw()
        .or(warp::any().map(String::new))
        .unify();
    let optional_body = warp::body::bytes()
        .map(Some)
        .or(warp::any().map(|| None))
        .unify();

    warp::method()
        .and(warp::path::full())
        .and(raw_query)
        .and(warp::header::optional::<String>(AUTHORIZATION.as_str()))
        .and(with_endpoints(endpoints))
        .and(optional_body)
        .and(with_grpc_registry(grpc_registry))
        .and_then(serve_dynamic_response)
        .recover(handle_rejection)
}

fn with_grpc_registry(
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (Arc<GrpcRegistry>,), Error = Infallible> + Clone {
    warp::any().map(move || Arc::clone(&grpc_registry))
}
