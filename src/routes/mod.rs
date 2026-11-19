use std::convert::Infallible;
use std::sync::Arc;
use warp::http::header::AUTHORIZATION;
use warp::{Filter, Rejection, Reply};
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::Endpoints;
use crate::routes::dynamic_response::serve_dynamic_response;
use crate::routes::endpoints::{delete_endpoint, list_endpoint, register_endpoint};

pub mod endpoints;
pub mod dashboard;
pub mod dynamic_response;

pub use dashboard::dashboard_routes;

const MAX_UPLOAD_BYTES: u64 = 5_000_000;

/// Routes the dashboard uses to register, list and delete mock endpoints.
pub fn admin_routes(
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let register = warp::post()
        .and(warp::path!("register"))
        .and(warp::multipart::form().max_length(MAX_UPLOAD_BYTES))
        .and(with(endpoints.clone()))
        .and(with(Arc::clone(&grpc_registry)))
        .then(register_endpoint);
    let list = warp::get()
        .and(warp::path!("list"))
        .and(with(endpoints.clone()))
        .then(list_endpoint);
    let delete = warp::delete()
        .and(warp::path!("delete" / String))
        .and(with(endpoints))
        .and(with(grpc_registry))
        .then(delete_endpoint);

    register.or(list).or(delete)
}

/// Catch-all filter that answers every registered mock endpoint.
///
/// It always produces a response, including for a path nobody registered, so it has to
/// be the last route.
pub fn mock_routes(
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let raw_query = warp::query::raw()
        .or(warp::any().map(String::new))
        .unify();

    warp::method()
        .and(warp::path::full())
        .and(raw_query)
        .and(warp::header::optional::<String>(AUTHORIZATION.as_str()))
        .and(warp::body::bytes())
        .and(with(endpoints))
        .and(with(grpc_registry))
        .then(serve_dynamic_response)
}

/// Hands a clone of shared state to a handler.
fn with<T: Clone + Send>(state: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}
