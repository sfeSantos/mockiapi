use std::sync::Arc;
use tokio::fs;
use crate::handlers::multipart::RegistrationForm;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{Endpoints, RegistrationError};
use crate::routing::Route;

const UNKNOWN_ID: RegistrationError = RegistrationError::Invalid("no endpoint has this id");
const FILE_REQUIRED: RegistrationError = RegistrationError::Invalid("a JSON response file is required");
const AMBIGUOUS_RESPONSE: RegistrationError = RegistrationError::Invalid(
    "this path has several endpoints for these methods: send a JSON response file, or the id of the one to change",
);

/// Stores what a form asks for and returns the id of the registration.
///
/// The new registration takes the place of the one the form names by id, and of every
/// registration of its path that shares a method with it. Their uploads go away with
/// them, except the one the new registration keeps.
pub async fn register(
    form: RegistrationForm,
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<u64, RegistrationError> {
    let target = form.target()?;
    let previous = previous_registration(&form, target, endpoints)?;
    let registration = form.validate(previous.as_deref().map(|route| &route.endpoint))?;

    if let Some(upload) = &registration.upload {
        fs::write(&registration.endpoint.file, upload)
            .await
            .map_err(|_| RegistrationError::Storage)?;
    }

    if let Some(grpc) = registration.grpc {
        grpc_registry.register_mock(&grpc.service, &grpc.rpc, grpc.response);
    }

    let kept_file = registration.endpoint.file.clone();
    let inserted = endpoints.insert(registration.path, registration.endpoint, target);

    for replaced in &inserted.replaced {
        if replaced.endpoint.file != kept_file {
            remove_upload(&replaced.endpoint.file).await;
        }
    }

    forget_grpc_mocks(&inserted.replaced, endpoints, grpc_registry);

    Ok(inserted.id)
}

/// The registration whose response a form without a file keeps.
///
/// It is the one named by id, or the only one the form replaces. A form that brings a
/// file needs none, unless it names an id, which then has to exist.
fn previous_registration(
    form: &RegistrationForm,
    target: Option<u64>,
    endpoints: &Endpoints,
) -> Result<Option<Arc<Route>>, RegistrationError> {
    if let Some(target) = target {
        return endpoints.get(target)
            .map(Some)
            .ok_or(UNKNOWN_ID);
    }

    if form.has_file() {
        return Ok(None);
    }

    let methods = form.requested_methods()?;
    let mut replaced = endpoints.overlapping(form.path.as_deref().unwrap_or_default(), &methods);

    match replaced.len() {
        0 => Err(FILE_REQUIRED),
        1 => Ok(replaced.pop()),
        _ => Err(AMBIGUOUS_RESPONSE),
    }
}

/// Takes out of the registry the mocks of gRPC endpoints that are no longer registered.
///
/// The registry is shared and answers by service and RPC, whatever path was called, so a
/// mock left behind keeps answering. One that an endpoint still registered answers for
/// stays: that covers an endpoint registered again for the same service and RPC.
pub fn forget_grpc_mocks(removed: &[Arc<Route>], endpoints: &Endpoints, grpc_registry: &GrpcRegistry) {
    for route in removed {
        let Some((service, rpc)) = route.endpoint.grpc_target() else {
            continue;
        };

        if !endpoints.answers_grpc(service, rpc) {
            grpc_registry.remove_mock(service, rpc);
        }
    }
}

/// `file` holds the full relative path the upload was written to.
pub async fn remove_upload(file: &str) {
    if fs::remove_file(file).await.is_err() {
        log::info!("Failed to delete file: {}", file);
    }
}
