use std::convert::Infallible;
use std::sync::Arc;
use warp::{reply, Filter, Rejection, Reply};
use crate::middlewares::grpc_registry::GrpcRegistry;
use warp::http::StatusCode;
use crate::models::{EndpointKind, Endpoints, InvalidRegistration, MultipartHandler, NotFound};

pub async fn register_endpoint(form: warp::multipart::FormData, endpoints: Endpoints, grpc_registry: Arc<GrpcRegistry>) -> Result<impl Reply, Rejection> {
    // A rejection here would fall through to the mock catch-all and come back as a 404,
    // so a bad form is answered directly.
    match MultipartHandler::parse(form, endpoints, grpc_registry).await {
        Ok(()) => Ok(reply::with_status(reply::json(&"Registered successfully"), StatusCode::OK)),
        Err(rejection) => match rejection.find::<InvalidRegistration>() {
            Some(InvalidRegistration(reason)) => {
                Ok(reply::with_status(reply::json(reason), StatusCode::BAD_REQUEST))
            }
            None => Err(rejection),
        },
    }
}

pub async fn list_endpoint(endpoints: Endpoints) -> Result<impl Reply, Rejection> {
    let store = endpoints.lock().await;

    Ok(reply::json(&*store))
}

pub async fn delete_endpoint(path_to_delete: String, endpoints: Endpoints, grpc_registry: Arc<GrpcRegistry>) -> Result<impl Reply, Rejection> {
    let decoded_path = urlencoding::decode(&path_to_delete)
        .map_err(|_| warp::reject::custom(NotFound))?
        .into_owned();

    let mut store = endpoints.lock().await;

    if let Some(route) = store.remove(&decoded_path) {
        let endpoint = &route.endpoint;
        // `file` already holds the full relative path the upload was written to.
        if tokio::fs::remove_file(&endpoint.file).await.is_err() {
            log::info!("Failed to delete file: {}", endpoint.file);
        }

        // The registry is shared by every gRPC endpoint, so its mocks go away with the last one.
        let is_last_grpc_endpoint = endpoint.kind == EndpointKind::Grpc
            && !store.iter().any(|remaining| remaining.endpoint.kind == EndpointKind::Grpc);

        if is_last_grpc_endpoint {
            grpc_registry.clear();
        }

        return Ok(reply::with_status("Deleted successfully", StatusCode::OK));
    }

    Err(warp::reject::custom(NotFound))
}

pub fn with_endpoints(endpoints: Endpoints) -> impl Filter<Extract = (Endpoints,), Error = Infallible> + Clone {
    warp::any().map(move || endpoints.clone())
}