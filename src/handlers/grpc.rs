use std::sync::Arc;
use warp::http::StatusCode;
use warp::{reply, Reply};
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::grpc::GrpcMockRequest;

pub async fn grpc_handler(req: GrpcMockRequest,
                          registry: Arc<GrpcRegistry>) -> Result<Box<dyn Reply>, warp::Rejection> {
    match registry.get_mock(&req.service, &req.method).await {
        Some(mock) => {
            if let Some(delay) = mock.delay_ms {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            let status = mock.status.unwrap_or(200);
            let json = reply::json(&mock.output);
            Ok(Box::new(reply::with_status(json, StatusCode::from_u16(status).unwrap())))
        }
        None => {
            let not_found = reply::with_status(
                reply::json(&serde_json::json!({
                    "error": "Mock not found"
                })),
                StatusCode::NOT_FOUND,
            );
            Ok(Box::new(not_found))
        }
    }
}