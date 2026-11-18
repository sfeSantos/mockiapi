use std::sync::Arc;
use std::time::Duration;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::time::Instant;
use warp::http::StatusCode;
use warp::test::request;
use warp::{Filter, Rejection, Reply};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::middlewares::rate_limit::new_rate_limit;
use mockiapi::models::grpc::GrpcMockResponse;
use mockiapi::models::{AuthData, Endpoint, EndpointKind, RateLimit};
use mockiapi::routes::mock_routes;
use mockiapi::routing::EndpointStore;

const BASIC_USER_PASS: &str = "Basic dXNlcjpwYXNz"; // user:pass

fn endpoint(kind: EndpointKind, method: &str, file: &str) -> Endpoint {
    Endpoint {
        kind,
        method: vec![method.to_string()],
        file: file.to_string(),
        status_code: Some(200),
        rate_limit: None,
        authentication: None,
        delay: None,
        with_dynamic_vars: None,
    }
}

fn credentials(json: &str) -> AuthData {
    serde_json::from_str(json).expect("credentials are not valid JSON")
}

fn rest_endpoint(file: &str) -> Endpoint {
    endpoint(EndpointKind::Rest, "GET", file)
}

/// Builds the same filter chain the server uses, so the tests see what a client sees.
fn routes_with_registry(
    registered: Vec<(&'static str, Endpoint)>,
    registry: Arc<GrpcRegistry>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let mut store = EndpointStore::default();

    for (path, endpoint) in registered {
        store.insert(path.to_string(), endpoint);
    }

    mock_routes(Arc::new(Mutex::new(store)), new_rate_limit(), registry)
}

fn routes(
    registered: Vec<(&'static str, Endpoint)>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    routes_with_registry(registered, Arc::new(GrpcRegistry::new()))
}

fn json_body(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("Response is not valid JSON")
}

#[tokio::test]
async fn test_non_existent_endpoint() {
    let filter = routes(vec![]);

    let res = request().method("GET").path("/nonexistent").reply(&filter).await;

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_valid_request_without_auth() {
    let filter = routes(vec![("/public", rest_endpoint("uploads/file.json"))]);

    let res = request().method("GET").path("/public").reply(&filter).await;

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res.body())["first_name"], "John");
}

#[tokio::test]
async fn test_rest_request_with_body_returns_the_mock() {
    let filter = routes(vec![(
        "/public",
        endpoint(EndpointKind::Rest, "POST", "uploads/file.json"),
    )]);

    let res = request()
        .method("POST")
        .path("/public")
        .json(&json!({ "anything": "at all" }))
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res.body())["first_name"], "John");
}

#[tokio::test]
async fn test_method_not_allowed() {
    let filter = routes(vec![("/public", rest_endpoint("uploads/file.json"))]);

    let res = request().method("DELETE").path("/public").reply(&filter).await;

    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_valid_request_with_basic_auth() {
    let mut protected = rest_endpoint("uploads/file.json");
    protected.authentication = Some(credentials("{\"username\": \"user\", \"password\": \"pass\"}"));
    let filter = routes(vec![("/protected", protected)]);

    let res = request()
        .method("GET")
        .path("/protected")
        .header("Authorization", BASIC_USER_PASS)
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_authentication() {
    let mut protected = rest_endpoint("uploads/file.json");
    protected.authentication = Some(credentials("{\"username\": \"user1\", \"password\": \"password1\"}"));
    let filter = routes(vec![("/protected", protected)]);

    let res = request()
        .method("GET")
        .path("/protected")
        .header("Authorization", "Basic dXNlcjpwd2Q=")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_missing_authentication() {
    let mut protected = rest_endpoint("uploads/file.json");
    protected.authentication = Some(credentials("{\"username\": \"user\", \"password\": \"pass\"}"));
    let filter = routes(vec![("/protected", protected)]);

    let res = request().method("GET").path("/protected").reply(&filter).await;

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_valid_request_with_bearer_auth() {
    let mut secure = rest_endpoint("uploads/file.json");
    secure.authentication = Some(credentials("{\"tokenData\": \"SOME_LONG_TOKEN\"}"));
    let filter = routes(vec![("/secure", secure)]);

    let res = request()
        .method("GET")
        .path("/secure")
        .header("Authorization", "Bearer SOME_LONG_TOKEN")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_rate_limit_exceeded() {
    let mut limited = rest_endpoint("uploads/file.json");
    limited.rate_limit = Some(RateLimit { requests: 1, window_ms: 60_000 });
    let filter = routes(vec![("/rate-limited", limited)]);

    let first = request().method("GET").path("/rate-limited").reply(&filter).await;
    let second = request().method("GET").path("/rate-limited").reply(&filter).await;

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_rate_limit_is_counted_per_method() {
    let mut limited = rest_endpoint("uploads/file.json");
    limited.method = vec!["GET".to_string(), "POST".to_string()];
    limited.rate_limit = Some(RateLimit { requests: 1, window_ms: 60_000 });
    let filter = routes(vec![("/rate-limited", limited)]);

    let get = request().method("GET").path("/rate-limited").reply(&filter).await;
    let post = request().method("POST").path("/rate-limited").reply(&filter).await;
    let second_post = request().method("POST").path("/rate-limited").reply(&filter).await;

    assert_eq!(get.status(), StatusCode::OK);
    assert_eq!(post.status(), StatusCode::OK);
    assert_eq!(second_post.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_request_with_delay() {
    let mut delayed = rest_endpoint("uploads/file.json");
    delayed.delay = Some(200);
    let filter = routes(vec![("/delayed", delayed)]);

    let start_time = Instant::now();
    let res = request().method("GET").path("/delayed").reply(&filter).await;
    let elapsed = start_time.elapsed();

    assert_eq!(res.status(), StatusCode::OK);
    assert!(elapsed >= Duration::from_millis(200));
}

#[tokio::test]
async fn test_custom_status_code() {
    let mut created = rest_endpoint("uploads/file.json");
    created.status_code = Some(201);
    let filter = routes(vec![("/created", created)]);

    let res = request().method("GET").path("/created").reply(&filter).await;

    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_extract_params_from_request() {
    let mut dynamic = rest_endpoint("uploads/dynamic_vars.json");
    dynamic.with_dynamic_vars = Some(true);
    let filter = routes(vec![("/api/user/{user}/item/{item}?name={name}", dynamic)]);

    let res = request()
        .method("GET")
        .path("/api/user/123/item/456?id=789&name=John")
        .reply(&filter)
        .await;
    let body = json_body(res.body());

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body["user"], "123");
    assert_eq!(body["item"], "456");
    assert_eq!(body["id"], "789");
    assert_eq!(body["name"], "John");
}

#[tokio::test]
async fn test_templated_path_answers_any_value() {
    let filter = routes(vec![("/api/user/{id}", rest_endpoint("uploads/file.json"))]);

    let first = request().method("GET").path("/api/user/1").reply(&filter).await;
    let second = request().method("GET").path("/api/user/2?verbose=true").reply(&filter).await;
    let too_deep = request().method("GET").path("/api/user/2/orders").reply(&filter).await;

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(too_deep.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_query_parameters_match_in_any_order() {
    let filter = routes(vec![("/search?kind=book&lang=en&page=1", rest_endpoint("uploads/file.json"))]);

    let same_order = request().method("GET").path("/search?kind=book&lang=en&page=1").reply(&filter).await;
    let shuffled = request().method("GET").path("/search?page=1&kind=book&lang=en").reply(&filter).await;
    let other_value = request().method("GET").path("/search?page=1&kind=film&lang=en").reply(&filter).await;

    assert_eq!(same_order.status(), StatusCode::OK);
    assert_eq!(shuffled.status(), StatusCode::OK);
    assert_eq!(other_value.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_rate_limit_is_shared_by_every_value_of_a_templated_path() {
    let mut limited = rest_endpoint("uploads/file.json");
    limited.rate_limit = Some(RateLimit { requests: 1, window_ms: 60_000 });
    let filter = routes(vec![("/api/user/{id}", limited)]);

    let first = request().method("GET").path("/api/user/1").reply(&filter).await;
    let second = request().method("GET").path("/api/user/2").reply(&filter).await;

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_extract_params_from_request_body() {
    let mut dynamic = endpoint(EndpointKind::Rest, "POST", "uploads/dynamic_vars.json");
    dynamic.with_dynamic_vars = Some(true);
    let filter = routes(vec![("/api/user", dynamic)]);

    let res = request()
        .method("POST")
        .path("/api/user")
        .json(&json!({ "id": "789", "name": "John", "user": "123", "item": "456" }))
        .reply(&filter)
        .await;
    let body = json_body(res.body());

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body["user"], "123");
    assert_eq!(body["item"], "456");
    assert_eq!(body["id"], "789");
    assert_eq!(body["name"], "John");
}

#[tokio::test]
async fn test_graphql_endpoint_answers_the_operation() {
    let filter = routes(vec![(
        "/api/graphql",
        endpoint(EndpointKind::GraphQl, "POST", "uploads/graphql.json"),
    )]);

    let res = request()
        .method("POST")
        .path("/api/graphql")
        .json(&json!({ "query": "query getUser { id name }" }))
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json_body(res.body()), json!({ "data": { "id": "123", "name": "John Doe" } }));
}

#[tokio::test]
async fn test_graphql_endpoint_rejects_a_body_that_is_not_graphql() {
    let filter = routes(vec![(
        "/api/graphql",
        endpoint(EndpointKind::GraphQl, "POST", "uploads/graphql.json"),
    )]);

    let res = request()
        .method("POST")
        .path("/api/graphql")
        .body("not graphql")
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_graphql_endpoint_rejects_a_query_that_does_not_parse() {
    let filter = routes(vec![(
        "/api/graphql",
        endpoint(EndpointKind::GraphQl, "POST", "uploads/graphql.json"),
    )]);

    let res = request()
        .method("POST")
        .path("/api/graphql")
        .json(&json!({ "query": "query getUser { id " }))
        .reply(&filter)
        .await;

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_grpc_endpoint_applies_the_delay_once() {
    let registry = Arc::new(GrpcRegistry::new());
    registry
        .register_mock(
            "com.example.BookService",
            "GetBook",
            GrpcMockResponse { output: json!({}), delay_ms: Some(300), status: Some(200) },
        );
    let mut grpc = endpoint(EndpointKind::Grpc, "POST", "uploads/file.json");
    grpc.delay = Some(300);
    let filter = routes_with_registry(vec![("/grpc", grpc)], registry);

    let start_time = Instant::now();
    let res = request()
        .method("POST")
        .path("/grpc")
        .json(&json!({ "service": "com.example.BookService", "rpc": "GetBook", "request": {} }))
        .reply(&filter)
        .await;
    let elapsed = start_time.elapsed();

    assert_eq!(res.status(), StatusCode::OK);
    assert!(elapsed >= Duration::from_millis(300));
    assert!(elapsed < Duration::from_millis(550), "delay applied twice: {elapsed:?}");
}

#[tokio::test]
async fn test_grpc_endpoint_rejects_a_body_that_is_not_a_call() {
    let filter = routes(vec![("/grpc", endpoint(EndpointKind::Grpc, "POST", "uploads/file.json"))]);

    let res = request().method("POST").path("/grpc").body("not a call").reply(&filter).await;

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_grpc_endpoint_answers_the_registered_mock() {
    let registry = Arc::new(GrpcRegistry::new());
    registry
        .register_mock(
            "com.example.BookService",
            "GetBook",
            GrpcMockResponse { output: json!({ "id": "b123" }), delay_ms: None, status: Some(200) },
        );
    let filter = routes_with_registry(
        vec![("/grpc", endpoint(EndpointKind::Grpc, "POST", "uploads/file.json"))],
        registry,
    );

    let found = request()
        .method("POST")
        .path("/grpc")
        .json(&json!({ "service": "com.example.BookService", "rpc": "GetBook", "request": {} }))
        .reply(&filter)
        .await;
    let missing = request()
        .method("POST")
        .path("/grpc")
        .json(&json!({ "service": "com.example.BookService", "rpc": "Nope", "request": {} }))
        .reply(&filter)
        .await;

    assert_eq!(found.status(), StatusCode::OK);
    assert_eq!(json_body(found.body()), json!({ "id": "b123" }));
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
