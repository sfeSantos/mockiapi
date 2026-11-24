use std::sync::Arc;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use warp::http::StatusCode;
use warp::multipart::FormData;
use warp::{reply, Reply};
use crate::handlers::multipart::part_bytes;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::mocks::{self, Syntax};
use crate::models::Endpoints;

const MISSING_FILE: &str = "send the mocks file in a form field named file";

/// Registers every endpoint of an uploaded mocks file: mockiapi's own format in JSON or
/// YAML, or a Postman collection. The name of the file tells YAML from JSON.
///
/// The file is registered whole or not at all, and a refusal says which endpoint and why.
pub async fn import_mocks(
    form: FormData,
    endpoints: Endpoints,
    grpc_registry: Arc<GrpcRegistry>,
) -> impl Reply {
    let Some((syntax, content)) = uploaded_file(form).await else {
        return reply::with_status(reply::json(&MISSING_FILE), StatusCode::BAD_REQUEST);
    };

    let imported = mocks::import(&content, syntax, &endpoints, &grpc_registry).await;
    // Also after a refusal: a file is checked whole before anything is stored, but a disk
    // that fails halfway can still leave some of its endpoints registered.
    endpoints.save().await;

    match imported {
        Ok(loaded) => reply::with_status(reply::json(&loaded), StatusCode::OK),
        Err(error) => reply::with_status(reply::json(&error.to_string()), StatusCode::BAD_REQUEST),
    }
}

/// The content of the `file` field and the syntax its file name asks for.
async fn uploaded_file(form: FormData) -> Option<(Syntax, Bytes)> {
    let mut parts = form.into_stream();

    while let Some(part) = parts.next().await {
        let part = part.ok()?;

        if part.name() != "file" {
            continue;
        }

        let syntax = Syntax::of(part.filename().unwrap_or_default());
        let content = part_bytes(part).await.ok()?;

        return Some((syntax, content));
    }

    None
}
