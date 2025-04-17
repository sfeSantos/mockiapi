use std::sync::Arc;
use serde_json::json;
use warp::http::StatusCode;
use mockiapi::handlers::grpc::grpc_handler;
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::grpc::{GrpcMockRequest, GrpcMockResponse};

#[tokio::test]
async fn test_grpc_handler_success() {
    let registry = Arc::new(GrpcRegistry::new());

    registry.register_mock(
        "UserService",
        "GetUser",
        GrpcMockResponse {
            output: json!({"id": 1, "name": "Alice"}),
            delay_ms: None,
            status: Some(200),
        }
    ).await;

    let req = GrpcMockRequest {
        service: "UserService".into(),
        method: "GetUser".into(),
        input: json!({"user_id": 1}),
    };

    let boxed_reply = grpc_handler(req, registry.clone()).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);
    
    assert_eq!(res.status(), StatusCode::OK);

    let body = warp::hyper::body::to_bytes(res.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json, json!({"id": 1, "name": "Alice"}));
}

#[tokio::test]
async fn test_grpc_handler_not_found() {
    let registry = Arc::new(GrpcRegistry::new());

    let req = GrpcMockRequest {
        service: "UserService".into(),
        method: "NonExistingMethod".into(),
        input: json!({}),
    };

    let boxed_reply = grpc_handler(req, registry.clone()).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);

    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let body = warp::hyper::body::to_bytes(res.into_body()).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "Mock not found");
}

#[tokio::test]
async fn test_grpc_handler_with_delay() {
    let registry = Arc::new(GrpcRegistry::new());

    registry.register_mock(
        "UserService",
        "DelayedResponse",
        GrpcMockResponse {
            output: json!({"status": "ok"}),
            delay_ms: Some(200),
            status: Some(200),
        }
    ).await;

    let req = GrpcMockRequest {
        service: "UserService".into(),
        method: "DelayedResponse".into(),
        input: json!({}),
    };

    let start = std::time::Instant::now();
    let boxed_reply = grpc_handler(req, registry.clone()).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);
    let duration = start.elapsed();

    assert!(duration.as_millis() >= 200);
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_grpc_handler_custom_status_code() {
    let registry = Arc::new(GrpcRegistry::new());

    registry.register_mock(
        "UserService",
        "ConflictCase",
        GrpcMockResponse {
            output: json!({"error": "Conflict"}),
            delay_ms: None,
            status: Some(409),
        }
    ).await;

    let req = GrpcMockRequest {
        service: "UserService".into(),
        method: "ConflictCase".into(),
        input: json!({}),
    };

    let boxed_reply = grpc_handler(req, registry.clone()).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);

    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_empty_service_and_method() {
    let registry = Arc::new(GrpcRegistry::new());

    let req = GrpcMockRequest {
        service: "".into(),
        method: "".into(),
        input: json!({}),
    };

    let boxed_reply = grpc_handler(req, registry).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_null_input_payload() {
    let registry = Arc::new(GrpcRegistry::new());

    registry.register_mock("TestService", "NullInput", GrpcMockResponse {
        output: json!({"ok": true}),
        delay_ms: None,
        status: Some(200),
    }).await;

    let req = GrpcMockRequest {
        service: "TestService".into(),
        method: "NullInput".into(),
        input: serde_json::Value::Null,
    };

    let boxed_reply = grpc_handler(req, registry).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_input_with_extra_fields() {
    let registry = Arc::new(GrpcRegistry::new());

    registry.register_mock("TestService", "ExtraFields", GrpcMockResponse {
        output: json!({"message": "Received"}),
        delay_ms: None,
        status: Some(200),
    }).await;

    let req = GrpcMockRequest {
        service: "TestService".into(),
        method: "ExtraFields".into(),
        input: json!({"field1": "value", "extra": 123}),
    };

    let boxed_reply = grpc_handler(req, registry).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_large_input_payload() {
    let registry = Arc::new(GrpcRegistry::new());

    registry.register_mock("TestService", "LargePayload", GrpcMockResponse {
        output: json!({"status": "ok"}),
        delay_ms: None,
        status: Some(200),
    }).await;

    let big_string = "x".repeat(100_000);
    let req = GrpcMockRequest {
        service: "TestService".into(),
        method: "LargePayload".into(),
        input: json!({ "blob": big_string }),
    };

    let boxed_reply = grpc_handler(req, registry).await.unwrap();
    let res = warp::reply::Reply::into_response(boxed_reply);
    assert_eq!(res.status(), StatusCode::OK);
}