use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use serde::Deserialize;
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::grpc::GrpcMockRequest;
use crate::utils::json_response;

const MOCK_NOT_FOUND: &str = r#"{"error":"Mock not found"}"#;

/// The part of a call needed to find its mock. The payload under `request` is never read,
/// so it is skipped instead of being parsed into a tree.
#[derive(Deserialize)]
struct GrpcCall<'a> {
    #[serde(borrow)]
    service: Cow<'a, str>,
    #[serde(borrow)]
    rpc: Cow<'a, str>,
}

/// Answers a simulated gRPC call sent as JSON. Returns `None` when the body is not a call.
pub async fn handle_grpc(body: &[u8], registry: &GrpcRegistry) -> Option<Response<Body>> {
    let call = serde_json::from_slice::<GrpcCall>(body).ok()?;

    Some(answer(&call.service, &call.rpc, registry).await)
}

/// Answers a call that was already decoded.
pub async fn grpc_handler(
    req: GrpcMockRequest,
    registry: Arc<GrpcRegistry>,
) -> Result<Response<Body>, warp::Rejection> {
    Ok(answer(&req.service, &req.method, &registry).await)
}

async fn answer(service: &str, method: &str, registry: &GrpcRegistry) -> Response<Body> {
    let Some(mock) = registry.get_mock(service, method) else {
        return json_response(StatusCode::NOT_FOUND, MOCK_NOT_FOUND);
    };

    if let Some(delay) = mock.delay_ms {
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    json_response(mock.status, mock.body.clone())
}
