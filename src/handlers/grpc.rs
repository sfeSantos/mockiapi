use std::sync::Arc;
use warp::http::{HeaderValue, Response, StatusCode};
use warp::{reply, Reply};
use warp::http::header::CONTENT_TYPE;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::grpc::GrpcMockRequest;

async fn process_grpc_mock(
    req: &GrpcMockRequest,
    registry: &GrpcRegistry,
) -> Option<(u16, String)> {
    let mock = registry.get_mock(&req.service, &req.method).await?;

    if let Some(delay) = mock.delay_ms {
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }

    let status = mock.status.unwrap_or(200);
    let body = serde_json::to_string(&mock.output).ok()?;

    Some((status, body))
}
pub async fn handle_grpc(
    body_str: &str,
    registry: Arc<GrpcRegistry>,
) -> Option<Response<String>> {
    let req = serde_json::from_str::<GrpcMockRequest>(body_str).ok()?;
    let (status, body) = process_grpc_mock(&req, &registry).await?;

    let response = Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(body.into())
        .ok()?;

    Some(response)
}

pub async fn grpc_handler(
    req: GrpcMockRequest,
    registry: Arc<GrpcRegistry>,
) -> Result<Box<dyn Reply>, warp::Rejection> {
    match process_grpc_mock(&req, &registry).await {
        Some((status, body)) => {
            let json_value: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let json = reply::json(&json_value);
            Ok(Box::new(reply::with_status(json, StatusCode::from_u16(status).unwrap())))
        }
        None => {
            let not_found = reply::with_status(
                reply::json(&serde_json::json!({ "error": "Mock not found" })),
                StatusCode::NOT_FOUND,
            );
            Ok(Box::new(not_found))
        }
    }
}