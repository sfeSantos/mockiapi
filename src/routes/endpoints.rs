use std::convert::Infallible;
use warp::{reply, Filter, Rejection, Reply};
use crate::models::{Endpoints, MultipartHandler, NotFound};

pub async fn register_endpoint(form: warp::multipart::FormData, endpoints: Endpoints) -> Result<impl Reply, Rejection> {
    MultipartHandler::parse(form, endpoints).await?;

    Ok(reply::json(&"Registered successfully"))
}

pub async fn list_endpoint(endpoints: Endpoints) -> Result<impl Reply, Rejection> {
    let endpoints_map = endpoints.lock().await.clone();
    Ok(reply::json(&endpoints_map))
}

pub async fn delete_endpoint(path_to_delete: String, endpoints: Endpoints) -> Result<impl Reply, Rejection> {
    let decoded_path = urlencoding::decode(&path_to_delete)
        .map_err(|_| warp::reject::custom(NotFound))?
        .into_owned();

    let mut endpoints_map = endpoints.lock().await;
    if let Some(endpoint) = endpoints_map.remove(&decoded_path) {
        let file_path = format!("uploads/{}", endpoint.file);
        if tokio::fs::remove_file(&file_path).await.is_err() {
            log::info!("Failed to delete file: {}", file_path);
        }
        return Ok(reply::with_status("Deleted successfully", warp::http::StatusCode::OK));
    }

    Err(warp::reject::custom(NotFound))
}

pub fn with_endpoints(endpoints: Endpoints) -> impl Filter<Extract = (Endpoints,), Error = Infallible> + Clone {
    warp::any().map(move || endpoints.clone())
}