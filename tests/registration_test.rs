use std::path::Path;
use std::sync::Arc;
use warp::http::StatusCode;
use warp::test::request;
use warp::{Filter, Rejection, Reply};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::Endpoints;
use mockiapi::routes::{admin_routes, mock_routes};
use mockiapi::routing::Route;
use serde_json::Value;

const BOUNDARY: &str = "mockiapi-test-boundary";

fn new_endpoints() -> Endpoints {
    Endpoints::default()
}

/// The registration of a path that holds a single one.
fn only(endpoints: &Endpoints, path: &str) -> Arc<Route> {
    let mut registered = endpoints.registered_on(path);

    assert_eq!(registered.len(), 1, "{path} should hold exactly one endpoint");

    registered.remove(0)
}

fn admin(endpoints: Endpoints) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    admin_routes(endpoints, Arc::new(GrpcRegistry::new()))
}

async fn delete(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    path: &str,
) -> StatusCode {
    request()
        .method("DELETE")
        .path(&format!("/delete/{}", urlencoding::encode(path)))
        .reply(filter)
        .await
        .status()
}

/// Encodes the fields the dashboard form sends, with `file` going out as the uploaded JSON.
fn multipart_body(fields: &[(&str, &str)]) -> String {
    let mut body = String::new();

    for (name, value) in fields {
        let disposition = match *name {
            "file" => "name=\"file\"; filename=\"mock.json\"\r\nContent-Type: application/json".to_string(),
            _ => format!("name=\"{name}\""),
        };
        body.push_str(&format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; {disposition}\r\n\r\n{value}\r\n"
        ));
    }

    body.push_str(&format!("--{BOUNDARY}--\r\n"));

    body
}

async fn register(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    fields: &[(&str, &str)],
) -> StatusCode {
    request()
        .method("POST")
        .path("/register")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(multipart_body(fields))
        .reply(filter)
        .await
        .status()
}

#[tokio::test]
async fn test_rate_limit_without_window_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("path", "/api/limited"), ("methods", "GET"), ("rate_limit", "10"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.is_empty());
}

#[tokio::test]
async fn test_rate_limit_that_is_not_a_number_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("path", "/api/limited"), ("methods", "GET"), ("rate_limit", "ten/1000"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.is_empty());
}

#[tokio::test]
async fn test_grpc_service_without_rpc_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("path", "/grpc"), ("methods", "POST"), ("grpcService", "com.example.BookService"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.is_empty());
}

#[tokio::test]
async fn test_delete_removes_the_endpoint_and_its_uploaded_file() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let registered = register(
        &filter,
        &[("path", "/api/temporary"), ("methods", "GET"), ("file", "{\"ok\":true}")],
    )
    .await;
    let uploaded_file = only(&endpoints, "/api/temporary").endpoint.file.clone();
    let existed_before_delete = Path::new(&uploaded_file).exists();
    let deleted = delete(&filter, "/api/temporary").await;

    assert_eq!(registered, StatusCode::OK);
    assert!(existed_before_delete);
    assert_eq!(deleted, StatusCode::OK);
    assert!(endpoints.is_empty());
    assert!(!Path::new(&uploaded_file).exists());
}

#[tokio::test]
async fn test_delete_removes_the_grpc_mocks_of_the_endpoint() {
    let registry = Arc::new(GrpcRegistry::new());
    let filter = admin_routes(new_endpoints(), Arc::clone(&registry));

    register(
        &filter,
        &[
            ("path", "/grpc"),
            ("methods", "POST"),
            ("grpcService", "com.example.BookService"),
            ("grpcRPC", "GetBook"),
            ("file", "{\"id\":\"b123\"}"),
        ],
    )
    .await;
    let mock_before_delete = registry.get_mock("com.example.BookService", "GetBook");
    let deleted = delete(&filter, "/grpc").await;

    assert!(mock_before_delete.is_some());
    assert_eq!(deleted, StatusCode::OK);
    assert!(registry.get_mock("com.example.BookService", "GetBook").is_none());
}

/// The admin routes and the mocks they register, as the binary serves them together.
fn admin_and_mocks(endpoints: Endpoints) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let registry = Arc::new(GrpcRegistry::new());

    admin_routes(endpoints.clone(), Arc::clone(&registry)).or(mock_routes(endpoints, registry))
}

async fn register_grpc(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    path: &str,
    service: &str,
    rpc: &str,
) -> StatusCode {
    let response = format!("{{\"from\":\"{path}\"}}");

    register(
        filter,
        &[("path", path), ("methods", "POST"), ("grpcService", service), ("grpcRPC", rpc), ("file", &response)],
    )
    .await
}

/// Calls a gRPC mock through the path of an endpoint, and gives the status and the body.
async fn call_grpc(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    path: &str,
    service: &str,
    rpc: &str,
) -> (StatusCode, String) {
    let response = request()
        .method("POST")
        .path(path)
        .body(format!("{{\"service\":\"{service}\",\"rpc\":\"{rpc}\",\"request\":{{}}}}"))
        .reply(filter)
        .await;

    (response.status(), String::from_utf8_lossy(response.body()).into_owned())
}

const MOCK_NOT_FOUND: &str = "{\"error\":\"Mock not found\"}";

#[tokio::test]
async fn test_deleted_grpc_endpoint_stops_answering_while_another_one_is_registered() {
    let filter = admin_and_mocks(new_endpoints());

    register_grpc(&filter, "/grpc/books", "Books", "GetBook").await;
    register_grpc(&filter, "/grpc/users", "Users", "GetUser").await;
    let deleted = delete(&filter, "/grpc/books").await;

    assert_eq!(deleted, StatusCode::OK);
    assert_eq!(
        call_grpc(&filter, "/grpc/users", "Books", "GetBook").await,
        (StatusCode::NOT_FOUND, MOCK_NOT_FOUND.to_string())
    );
    assert_eq!(
        call_grpc(&filter, "/grpc/users", "Users", "GetUser").await,
        (StatusCode::OK, "{\"from\":\"/grpc/users\"}".to_string())
    );

    delete(&filter, "/grpc/users").await;
}

#[tokio::test]
async fn test_grpc_endpoint_registered_again_for_another_rpc_stops_answering_the_previous_one() {
    let endpoints = new_endpoints();
    let filter = admin_and_mocks(endpoints.clone());

    register_grpc(&filter, "/grpc/books", "Books", "GetBook").await;
    register_grpc(&filter, "/grpc/users", "Users", "GetUser").await;
    register_grpc(&filter, "/grpc/books", "Books", "ListBooks").await;

    assert_eq!(endpoints.len(), 2);
    assert_eq!(call_grpc(&filter, "/grpc/books", "Books", "GetBook").await.0, StatusCode::NOT_FOUND);
    assert_eq!(call_grpc(&filter, "/grpc/books", "Books", "ListBooks").await.0, StatusCode::OK);
    assert_eq!(call_grpc(&filter, "/grpc/books", "Users", "GetUser").await.0, StatusCode::OK);

    delete(&filter, "/grpc/books").await;
    delete(&filter, "/grpc/users").await;
}

#[tokio::test]
async fn test_grpc_endpoint_registered_again_for_the_same_rpc_answers_the_new_response() {
    let filter = admin_and_mocks(new_endpoints());

    register_grpc(&filter, "/grpc/books", "Books", "GetBook").await;
    let registered_again = register(
        &filter,
        &[
            ("path", "/grpc/books"),
            ("methods", "POST"),
            ("grpcService", "Books"),
            ("grpcRPC", "GetBook"),
            ("file", "{\"edition\":2}"),
        ],
    )
    .await;

    assert_eq!(registered_again, StatusCode::OK);
    assert_eq!(
        call_grpc(&filter, "/grpc/books", "Books", "GetBook").await,
        (StatusCode::OK, "{\"edition\":2}".to_string())
    );

    delete(&filter, "/grpc/books").await;
}

#[tokio::test]
async fn test_mock_shared_by_two_grpc_endpoints_goes_away_with_the_last_of_them() {
    let filter = admin_and_mocks(new_endpoints());

    register_grpc(&filter, "/grpc/first", "Books", "GetBook").await;
    register_grpc(&filter, "/grpc/second", "Books", "GetBook").await;
    delete(&filter, "/grpc/first").await;
    let after_the_first = call_grpc(&filter, "/grpc/second", "Books", "GetBook").await.0;
    delete(&filter, "/grpc/second").await;
    let after_the_last = call_grpc(&filter, "/grpc/second", "Books", "GetBook").await.0;

    assert_eq!(after_the_first, StatusCode::OK);
    assert_eq!(after_the_last, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_registering_a_path_again_removes_the_previous_upload() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());
    let fields = [("path", "/api/replaced"), ("methods", "GET"), ("file", "{}")];

    register(&filter, &fields).await;
    let first_file = only(&endpoints, "/api/replaced").endpoint.file.clone();
    register(&filter, &fields).await;
    let second_file = only(&endpoints, "/api/replaced").endpoint.file.clone();
    delete(&filter, "/api/replaced").await;

    assert_ne!(first_file, second_file);
    assert!(!Path::new(&first_file).exists());
    assert!(!Path::new(&second_file).exists());
}

#[tokio::test]
async fn test_registering_a_path_again_without_a_file_keeps_the_upload() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    register(&filter, &[("path", "/api/kept"), ("methods", "GET"), ("file", "{\"ok\":true}")]).await;
    let first_file = only(&endpoints, "/api/kept").endpoint.file.clone();
    let updated = register(
        &filter,
        &[("path", "/api/kept"), ("methods", "GET,POST"), ("status_code", "503")],
    )
    .await;
    let route = only(&endpoints, "/api/kept");
    let kept_on_disk = std::fs::read_to_string(&first_file).ok();
    delete(&filter, "/api/kept").await;

    assert_eq!(updated, StatusCode::OK);
    assert_eq!(route.endpoint.file, first_file);
    assert_eq!(route.endpoint.method, ["GET", "POST"]);
    assert_eq!(route.endpoint.status_code, Some(503));
    assert_eq!(kept_on_disk.as_deref(), Some("{\"ok\":true}"));
    assert!(!Path::new(&first_file).exists());
}

#[tokio::test]
async fn test_registering_a_new_path_without_a_file_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(&filter, &[("path", "/api/missing-file"), ("methods", "GET")]).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.is_empty());
}

async fn content(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
    path: &str,
) -> (StatusCode, String) {
    let response = request()
        .path(&format!("/content/{}", urlencoding::encode(path)))
        .reply(filter)
        .await;

    (response.status(), String::from_utf8_lossy(response.body()).into_owned())
}

#[tokio::test]
async fn test_content_answers_the_response_as_it_was_uploaded() {
    let filter = admin(new_endpoints());
    let template = "{\n  \"id\": {{id}}\n}";

    register(
        &filter,
        &[("path", "/usr/{id}"), ("methods", "GET"), ("with_dynamic_vars", "true"), ("file", template)],
    )
    .await;
    let (status, body) = content(&filter, "/usr/{id}").await;
    delete(&filter, "/usr/{id}").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, template);
}

#[tokio::test]
async fn test_content_follows_a_new_upload_and_survives_an_update_without_one() {
    let filter = admin(new_endpoints());

    register(&filter, &[("path", "/api/content"), ("methods", "GET"), ("file", "{\"version\":1}")]).await;
    register(&filter, &[("path", "/api/content"), ("methods", "GET"), ("file", "{\"version\":2}")]).await;
    let (_, replaced) = content(&filter, "/api/content").await;
    register(&filter, &[("path", "/api/content"), ("methods", "GET"), ("status_code", "503")]).await;
    let (_, kept) = content(&filter, "/api/content").await;
    delete(&filter, "/api/content").await;

    assert_eq!(replaced, "{\"version\":2}");
    assert_eq!(kept, "{\"version\":2}");
}

#[tokio::test]
async fn test_content_of_an_unknown_path_is_not_found() {
    let filter = admin(new_endpoints());

    let (status, _) = content(&filter, "/api/nobody").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn list(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
) -> Value {
    let response = request().path("/list").reply(filter).await;

    serde_json::from_slice(response.body()).expect("/list did not answer JSON")
}

/// Registers the list and the creation of a resource on one path and returns their ids.
async fn register_users(
    filter: &(impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static),
) -> (String, String) {
    register(filter, &[("path", "/users"), ("methods", "GET"), ("file", "[]")]).await;
    register(
        filter,
        &[("path", "/users"), ("methods", "POST"), ("status_code", "201"), ("file", "{\"id\":1}")],
    )
    .await;
    let listed = list(filter).await;

    (listed[0]["id"].to_string(), listed[1]["id"].to_string())
}

#[tokio::test]
async fn test_one_path_holds_a_registration_for_each_method() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    register_users(&filter).await;
    let listed = list(&filter).await;
    delete(&filter, "/users").await;

    assert_eq!(listed.as_array().map(Vec::len), Some(2));
    assert_eq!(listed[0]["path"], "/users");
    assert_eq!(listed[0]["method"][0], "GET");
    assert_eq!(listed[0]["status_code"], 200);
    assert_eq!(listed[1]["path"], "/users");
    assert_eq!(listed[1]["method"][0], "POST");
    assert_eq!(listed[1]["status_code"], 201);
    assert_ne!(listed[0]["id"], listed[1]["id"]);
}

#[tokio::test]
async fn test_deleting_one_registration_leaves_the_other_of_its_path_with_its_upload() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let (list_id, create_id) = register_users(&filter).await;
    let deleted = delete(&filter, &list_id).await;
    let left = only(&endpoints, "/users");
    let (content_status, kept_content) = content(&filter, &create_id).await;
    delete(&filter, &create_id).await;

    assert_eq!(deleted, StatusCode::OK);
    assert_eq!(left.endpoint.method, ["POST"]);
    assert_eq!(content_status, StatusCode::OK);
    assert_eq!(kept_content, "{\"id\":1}");
    assert!(endpoints.is_empty());
}

#[tokio::test]
async fn test_deleting_by_path_removes_every_registration_of_it() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    register_users(&filter).await;
    let files: Vec<String> = endpoints.registered_on("/users")
        .iter()
        .map(|route| route.endpoint.file.clone())
        .collect();
    let deleted = delete(&filter, "/users").await;

    assert_eq!(deleted, StatusCode::OK);
    assert!(endpoints.is_empty());
    assert!(files.iter().all(|file| !Path::new(file).exists()));
}

#[tokio::test]
async fn test_content_by_path_is_refused_when_the_path_has_several_endpoints() {
    let filter = admin(new_endpoints());

    let (list_id, _) = register_users(&filter).await;
    let (by_path, reason) = content(&filter, "/users").await;
    let (by_id, body) = content(&filter, &list_id).await;
    delete(&filter, "/users").await;

    assert_eq!(by_path, StatusCode::BAD_REQUEST);
    assert!(reason.contains("id"));
    assert_eq!(by_id, StatusCode::OK);
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn test_registering_a_method_again_replaces_only_that_registration() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let (list_id, create_id) = register_users(&filter).await;
    register(&filter, &[("path", "/users"), ("methods", "GET"), ("file", "[{\"id\":7}]")]).await;
    let listed = list(&filter).await;
    let (_, replaced_content) = content(&filter, &list_id).await;
    delete(&filter, "/users").await;

    assert_eq!(listed.as_array().map(Vec::len), Some(2));
    assert_eq!(listed[0]["id"].to_string(), list_id);
    assert_eq!(listed[1]["id"].to_string(), create_id);
    assert_eq!(replaced_content, "[{\"id\":7}]");
}

#[tokio::test]
async fn test_id_changes_the_methods_of_a_registration_and_keeps_its_response() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let (list_id, _) = register_users(&filter).await;
    let updated = register(&filter, &[("id", &list_id), ("path", "/users"), ("methods", "GET,PATCH")]).await;
    let listed = list(&filter).await;
    let (_, kept_content) = content(&filter, &list_id).await;
    delete(&filter, "/users").await;

    assert_eq!(updated, StatusCode::OK);
    assert_eq!(listed.as_array().map(Vec::len), Some(2));
    assert_eq!(listed[0]["id"].to_string(), list_id);
    assert_eq!(listed[0]["method"], serde_json::json!(["GET", "PATCH"]));
    assert_eq!(kept_content, "[]");
}

#[tokio::test]
async fn test_unknown_id_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    let status = register(
        &filter,
        &[("id", "999"), ("path", "/api/unknown-id"), ("methods", "GET"), ("file", "{}")],
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(endpoints.is_empty());
}

#[tokio::test]
async fn test_update_without_a_file_that_replaces_two_registrations_is_rejected() {
    let endpoints = new_endpoints();
    let filter = admin(endpoints.clone());

    register_users(&filter).await;
    let status = register(&filter, &[("path", "/users"), ("methods", "GET,POST")]).await;
    let still_registered = endpoints.len();
    delete(&filter, "/users").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(still_registered, 2);
}

#[tokio::test]
async fn test_group_is_listed_with_the_endpoint() {
    let filter = admin(new_endpoints());

    register(
        &filter,
        &[("path", "/api/grouped"), ("methods", "GET"), ("group", "users"), ("file", "{}")],
    )
    .await;
    register(&filter, &[("path", "/api/ungrouped"), ("methods", "GET"), ("file", "{}")]).await;
    let listed = list(&filter).await;
    delete(&filter, "/api/grouped").await;
    delete(&filter, "/api/ungrouped").await;

    assert_eq!(listed[0]["group"], "users");
    assert_eq!(listed[1]["group"], Value::Null);
}
