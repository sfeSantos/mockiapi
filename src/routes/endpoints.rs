use std::sync::Arc;
use tokio::fs;
use warp::http::StatusCode;
use warp::multipart::FormData;
use warp::{reply, Reply};
use crate::handlers::multipart::RegistrationForm;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{EndpointKind, Endpoints, RegistrationError};
use crate::utils::json_response;

pub async fn register_endpoint(
    form: FormData,
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Reply {
    // The refusal is answered here: a rejection would fall through to the mock catch-all.
    match register(form, &endpoints, &grpc_registry).await {
        Ok(()) => reply::with_status(reply::json(&"Registered successfully"), StatusCode::OK),
        Err(error) => reply::with_status(reply::json(&error.reason()), error.status()),
    }
}

async fn register(
    form: FormData,
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<(), RegistrationError> {
    let registration = RegistrationForm::read(form).await?.validate()?;

    fs::write(&registration.endpoint.file, &registration.upload)
        .await
        .map_err(|_| RegistrationError::Storage)?;

    if let Some(grpc) = registration.grpc {
        grpc_registry.register_mock(&grpc.service, &grpc.rpc, grpc.response);
    }

    let replaced = endpoints.insert(registration.path, registration.endpoint);

    // Registering a path again supersedes its previous upload.
    if let Some(replaced) = replaced {
        remove_upload(&replaced.endpoint.file).await;
    }

    Ok(())
}

pub async fn list_endpoint(endpoints: Endpoints) -> impl Reply {
    json_response(StatusCode::OK, endpoints.to_json())
}

pub async fn delete_endpoint(
    path_to_delete: String,
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Reply {
    let removed = urlencoding::decode(&path_to_delete)
        .ok()
        .and_then(|decoded_path| endpoints.remove(&decoded_path));
    let Some(route) = removed else {
        return reply::with_status("Resource not found", StatusCode::NOT_FOUND);
    };

    remove_upload(&route.endpoint.file).await;

    // The registry is shared by every gRPC endpoint, so its mocks go away with the last one.
    if route.endpoint.kind() == EndpointKind::Grpc && !endpoints.has_kind(EndpointKind::Grpc) {
        grpc_registry.clear();
    }

    reply::with_status("Deleted successfully", StatusCode::OK)
}

/// `file` holds the full relative path the upload was written to.
async fn remove_upload(file: &str) {
    if fs::remove_file(file).await.is_err() {
        log::info!("Failed to delete file: {}", file);
    }
}
