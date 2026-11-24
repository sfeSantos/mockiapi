use std::sync::Arc;
use tokio::fs;
use warp::http::StatusCode;
use warp::multipart::FormData;
use warp::{reply, Reply};
use crate::handlers::multipart::RegistrationForm;
use crate::handlers::registration::{forget_grpc_mocks, register, remove_upload};
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{Endpoints, RegistrationError};
use crate::routing::Route;
use crate::utils::{json_response, text_response};

pub async fn register_endpoint(
    form: FormData,
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Reply {
    // The refusal is answered here: a rejection would fall through to the mock catch-all.
    match register_form(form, &endpoints, &grpc_registry).await {
        Ok(()) => reply::with_status(reply::json(&"Registered successfully"), StatusCode::OK),
        Err(error) => reply::with_status(reply::json(&error.reason()), error.status()),
    }
}

async fn register_form(
    form: FormData,
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<(), RegistrationError> {
    let form = RegistrationForm::read(form).await?;
    register(form, endpoints, grpc_registry).await?;
    endpoints.save().await;

    Ok(())
}

pub async fn list_endpoint(endpoints: Endpoints) -> impl Reply {
    json_response(StatusCode::OK, endpoints.to_json())
}

/// How the admin API names an endpoint in a URL: by the id `GET /list` gives it, or by
/// the path it was registered with, URL-encoded.
#[derive(Debug, PartialEq, Eq)]
enum EndpointKey {
    Id(u64),
    Path(String),
}

impl EndpointKey {
    /// A registered path always starts with `/`, so it can never be taken for an id.
    fn parse(segment: &str) -> Option<Self> {
        if let Ok(id) = segment.parse() {
            return Some(EndpointKey::Id(id));
        }

        urlencoding::decode(segment)
            .ok()
            .map(|path| EndpointKey::Path(path.into_owned()))
    }
}

/// Answers the response an endpoint was registered with, as the text that was uploaded.
///
/// It is read from the upload, not from memory: a GraphQL document is kept parsed, which
/// loses the formatting the dashboard shows back to the user.
pub async fn endpoint_content(key: String, endpoints: Endpoints) -> impl Reply {
    let mut routes = match EndpointKey::parse(&key) {
        Some(EndpointKey::Id(id)) => endpoints.get(id)
            .into_iter()
            .collect(),
        Some(EndpointKey::Path(path)) => endpoints.registered_on(&path),
        None => Vec::new(),
    };

    if routes.len() > 1 {
        return text_response(
            StatusCode::BAD_REQUEST,
            "this path has several endpoints: ask for one by the id GET /list gives it",
        );
    }

    let Some(route) = routes.pop() else {
        return text_response(StatusCode::NOT_FOUND, "Resource not found");
    };

    match fs::read(&route.endpoint.file).await {
        Ok(content) => text_response(StatusCode::OK, content),
        Err(_) => text_response(StatusCode::INTERNAL_SERVER_ERROR, "the response file could not be read"),
    }
}

/// Deletes one endpoint by id, or every endpoint of a path.
pub async fn delete_endpoint(
    key: String,
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Reply {
    let removed: Vec<Arc<Route>> = match EndpointKey::parse(&key) {
        Some(EndpointKey::Id(id)) => endpoints.remove(id)
            .into_iter()
            .collect(),
        Some(EndpointKey::Path(path)) => endpoints.remove_path(&path),
        None => Vec::new(),
    };

    if removed.is_empty() {
        return reply::with_status("Resource not found", StatusCode::NOT_FOUND);
    }

    for route in &removed {
        remove_upload(&route.endpoint.file).await;
    }

    forget_grpc_mocks(&removed, &endpoints, &grpc_registry);
    endpoints.save().await;

    reply::with_status("Deleted successfully", StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_is_an_id_and_anything_else_is_a_path() {
        assert_eq!(EndpointKey::parse("12"), Some(EndpointKey::Id(12)));
        assert_eq!(EndpointKey::parse("%2Fapi%2Fhello"), Some(EndpointKey::Path("/api/hello".to_string())));
        assert_eq!(EndpointKey::parse("%2F12"), Some(EndpointKey::Path("/12".to_string())));
    }
}
