use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use serde_json::{json, Value};
use warp::http::StatusCode;
use warp::test::request;
use warp::{Filter, Rejection, Reply};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::mocks::{self, Loaded, MocksError, Syntax};
use mockiapi::models::Endpoints;
use mockiapi::routes::{admin_routes, mock_routes};

const FIXTURES: &str = "tests/fixtures/mocks";
const BOUNDARY: &str = "mockiapi-test-boundary";

/// A server with nothing registered, built from the same filters the binary serves.
struct Server {
    endpoints: Endpoints,
    registry: Arc<GrpcRegistry>,
}

impl Server {
    fn new() -> Self {
        Self { endpoints: Endpoints::default(), registry: Arc::new(GrpcRegistry::new()) }
    }

    async fn load(&self, fixture: &str) -> Result<Loaded, MocksError> {
        mocks::load_file(&Path::new(FIXTURES).join(fixture), &self.endpoints, &self.registry).await
    }

    fn routes(&self) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static {
        admin_routes(self.endpoints.clone(), Arc::clone(&self.registry))
            .or(mock_routes(self.endpoints.clone(), Arc::clone(&self.registry)))
    }

    async fn call(&self, method: &str, path: &str) -> (StatusCode, String) {
        let response = request()
            .method(method)
            .path(path)
            .reply(&self.routes())
            .await;

        (response.status(), String::from_utf8_lossy(response.body()).into_owned())
    }

    async fn post_json(&self, path: &str, body: Value) -> (StatusCode, Value) {
        let response = request()
            .method("POST")
            .path(path)
            .json(&body)
            .reply(&self.routes())
            .await;

        (response.status(), serde_json::from_slice(response.body()).unwrap_or(Value::Null))
    }

    async fn list(&self) -> Vec<Value> {
        let (_, body) = self.call("GET", "/list").await;

        serde_json::from_str(&body).expect("/list did not answer a JSON list")
    }

    async fn import(&self, file_name: &str, content: &str) -> (StatusCode, Value) {
        let body = format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\r\n{content}\r\n--{BOUNDARY}--\r\n"
        );
        let response = request()
            .method("POST")
            .path("/import")
            .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
            .body(body)
            .reply(&self.routes())
            .await;

        (response.status(), serde_json::from_slice(response.body()).unwrap_or(Value::Null))
    }

    /// Deletes every endpoint, which also removes the copies of their responses in `uploads/`.
    async fn clear(&self) {
        for endpoint in self.list().await {
            self.call("DELETE", &format!("/delete/{}", endpoint["id"])).await;
        }
    }
}

fn json_of(body: &str) -> Value {
    serde_json::from_str(body).expect("the response is not JSON")
}

async fn assert_the_example_endpoints_answer(server: &Server) {
    let (list_status, users) = server.call("GET", "/api/users").await;
    let (create_status, created) = server.call("POST", "/api/users").await;
    let (_, user) = server.call("GET", "/api/users/42").await;
    let (refused_status, _) = server.call("PUT", "/api/users").await;
    let (graphql_status, graphql) = server
        .post_json("/api/graphql", json!({ "query": "query getUser { id name }" }))
        .await;
    let (grpc_status, grpc) = server
        .post_json("/grpc", json!({ "service": "com.example.BookService", "rpc": "GetBook", "request": {} }))
        .await;

    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(json_of(&users)[1]["name"], "Linus");
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(json_of(&created)["name"], "Grace");
    assert_eq!(json_of(&user), json!({ "id": "42", "name": "Ada" }));
    assert_eq!(refused_status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(graphql_status, StatusCode::OK);
    assert_eq!(graphql, json!({ "data": { "id": "123", "name": "John Doe" } }));
    assert_eq!(grpc_status, StatusCode::OK);
    assert_eq!(grpc["title"], "Mock Book");
}

#[tokio::test]
async fn test_yaml_file_registers_every_kind_of_endpoint() {
    let server = Server::new();

    let loaded = server.load("mocks.yaml").await.unwrap();
    let listed = server.list().await;
    assert_the_example_endpoints_answer(&server).await;
    server.clear().await;

    assert_eq!(loaded, Loaded { registered: 6, warnings: Vec::new() });
    assert_eq!(listed.len(), 6);
    assert_eq!(listed[0]["group"], "users");
    assert_eq!(listed[4]["type"], "GraphQL");
    assert_eq!(listed[5]["type"], "gRPC");
}

#[tokio::test]
async fn test_json_file_registers_the_same_endpoints() {
    let server = Server::new();

    let loaded = server.load("mocks.json").await.unwrap();
    assert_the_example_endpoints_answer(&server).await;
    server.clear().await;

    assert_eq!(loaded.registered, 6);
}

#[tokio::test]
async fn test_loaded_endpoint_keeps_its_settings() {
    let server = Server::new();

    server.load("mocks.yaml").await.unwrap();
    let (without_token, _) = server.call("GET", "/api/orders").await;
    let with_token = request()
        .path("/api/orders")
        .header("authorization", "Bearer secret-token")
        .reply(&server.routes())
        .await;
    let orders = server.list().await.remove(3);
    server.clear().await;

    assert_eq!(without_token, StatusCode::UNAUTHORIZED);
    assert_eq!(with_token.status(), StatusCode::OK);
    assert_eq!(orders["delay"], 20);
    assert_eq!(orders["rate_limit"], json!({ "requests": 100, "window_ms": 1000 }));
}

#[tokio::test]
async fn test_content_shows_a_response_from_a_file_and_one_written_inline() {
    let server = Server::new();

    server.load("mocks.yaml").await.unwrap();
    let listed = server.list().await;
    let (_, from_file) = server.call("GET", &format!("/content/{}", listed[0]["id"])).await;
    let (_, inline) = server.call("GET", &format!("/content/{}", listed[2]["id"])).await;
    server.clear().await;

    assert_eq!(from_file, std::fs::read_to_string(format!("{FIXTURES}/responses/users.json")).unwrap());
    assert_eq!(inline, "{ \"id\": \"{{id}}\", \"name\": \"Ada\" }");
}

#[tokio::test]
async fn test_deleting_a_loaded_endpoint_leaves_the_files_it_came_from() {
    let server = Server::new();
    let response_file = PathBuf::from(FIXTURES).join("responses/users.json");

    server.load("mocks.yaml").await.unwrap();
    let copies: Vec<String> = server.list()
        .await
        .iter()
        .map(|endpoint| endpoint["file"].as_str().unwrap_or_default().to_string())
        .collect();
    server.clear().await;

    assert!(server.endpoints.is_empty());
    assert!(response_file.exists());
    assert!(Path::new(FIXTURES).join("mocks.yaml").exists());
    assert!(copies.iter().all(|copy| copy.starts_with("uploads/") && !Path::new(copy).exists()));
}

#[tokio::test]
async fn test_postman_collection_gives_each_method_of_a_path_its_own_response() {
    let server = Server::new();

    let loaded = server.load("shop.postman_collection.json").await.unwrap();
    let listed = server.list().await;
    let (list_status, users) = server.call("GET", "/api/users").await;
    let (create_status, created) = server.call("POST", "/api/users").await;
    let (delete_status, _) = server.call("DELETE", "/api/users/7").await;
    let (health_status, _) = server.call("GET", "/health").await;
    let (login_status, _) = server.call("GET", "/login").await;
    server.clear().await;

    assert_eq!(loaded.registered, 4);
    assert_eq!(loaded.warnings.len(), 1);
    assert!(loaded.warnings[0].contains("\"Login page\" (GET /login)"));
    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(json_of(&users)[0]["name"], "Ada");
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(json_of(&created)["name"], "Grace");
    assert_eq!(delete_status, StatusCode::NO_CONTENT);
    assert_eq!(health_status, StatusCode::OK);
    assert_eq!(login_status, StatusCode::NOT_FOUND);
    assert_eq!(listed[0]["group"], "Users");
    assert_eq!(listed[2]["path"], "/api/users/{id}");
    assert_eq!(listed[2]["group"], "Users");
    assert_eq!(listed[3]["group"], Value::Null);
}

#[tokio::test]
async fn test_file_that_cannot_be_loaded_registers_nothing_and_says_why() {
    let server = Server::new();
    let directory = std::env::temp_dir().join(format!("mockiapi-broken-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let write = |name: &str, content: &str| {
        let file = directory.join(name);
        std::fs::write(&file, content).unwrap();

        file
    };
    let missing_response = write(
        "missing-response.yaml",
        "endpoints:\n  - path: /ok\n    body: {}\n  - path: /broken\n    file: nowhere.json\n",
    );
    let refused_entry = write("refused.yaml", "endpoints:\n  - path: no-slash\n    body: {}\n");
    let twice = write(
        "twice.yaml",
        "endpoints:\n  - path: /twice\n    body: {}\n  - path: /twice\n    methods: [GET, POST]\n    body: []\n",
    );
    let not_yaml = write("syntax.yaml", "endpoints:\n  - path: [\n");

    let missing_file = mocks::load_file(&directory.join("absent.yaml"), &server.endpoints, &server.registry).await;
    let missing_response = mocks::load_file(&missing_response, &server.endpoints, &server.registry).await;
    let refused_entry = mocks::load_file(&refused_entry, &server.endpoints, &server.registry).await;
    let twice = mocks::load_file(&twice, &server.endpoints, &server.registry).await;
    let not_yaml = mocks::load_file(&not_yaml, &server.endpoints, &server.registry).await;
    std::fs::remove_dir_all(&directory).unwrap();

    assert!(matches!(missing_file, Err(MocksError::Unreadable(_))));
    assert!(matches!(
        missing_response,
        Err(MocksError::Entry { position: 2, ref path, ref reason }) if path == "/broken" && reason.contains("nowhere.json")
    ));
    assert!(matches!(refused_entry, Err(MocksError::Entry { position: 1, ref reason, .. }) if reason.contains("path")));
    assert!(matches!(
        twice,
        Err(MocksError::Entry { position: 2, ref reason, .. }) if reason.contains("GET /twice") && reason.contains("endpoint 1")
    ));
    assert!(matches!(not_yaml, Err(MocksError::Invalid(_))));
    assert!(server.endpoints.is_empty());
}

#[tokio::test]
async fn test_import_registers_an_uploaded_yaml_file() {
    let server = Server::new();
    let yaml = "endpoints:\n  - path: /imported\n    methods: [GET]\n    group: imported\n    body: { ok: true }\n";

    let (status, answer) = server.import("mocks.yaml", yaml).await;
    let (called, body) = server.call("GET", "/imported").await;
    server.clear().await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer, json!({ "registered": 1, "warnings": [] }));
    assert_eq!(called, StatusCode::OK);
    assert_eq!(json_of(&body), json!({ "ok": true }));
}

#[tokio::test]
async fn test_import_registers_an_uploaded_postman_collection_and_reports_what_it_skipped() {
    let server = Server::new();
    let collection = std::fs::read_to_string(format!("{FIXTURES}/shop.postman_collection.json")).unwrap();

    let (status, answer) = server.import("shop.postman_collection.json", &collection).await;
    let (created, _) = server.call("POST", "/api/users").await;
    server.clear().await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["registered"], 4);
    assert_eq!(answer["warnings"].as_array().map(Vec::len), Some(1));
    assert_eq!(created, StatusCode::CREATED);
}

#[tokio::test]
async fn test_import_replaces_the_endpoints_it_overlaps_and_keeps_the_others() {
    let server = Server::new();

    server.import("first.json", r#"{ "endpoints": [
        { "path": "/kept", "body": { "from": "first" } },
        { "path": "/replaced", "body": { "from": "first" } }
    ] }"#).await;
    server.import("second.json", r#"{ "endpoints": [ { "path": "/replaced", "body": { "from": "second" } } ] }"#).await;
    let (_, kept) = server.call("GET", "/kept").await;
    let (_, replaced) = server.call("GET", "/replaced").await;
    let registered = server.list().await.len();
    server.clear().await;

    assert_eq!(json_of(&kept)["from"], "first");
    assert_eq!(json_of(&replaced)["from"], "second");
    assert_eq!(registered, 2);
}

#[tokio::test]
async fn test_import_refuses_a_file_it_cannot_register_whole() {
    let server = Server::new();
    let with_response_file = "endpoints:\n  - path: /ok\n    body: {}\n  - path: /from-file\n    file: users.json\n";

    let (file_status, file_reason) = server.import("mocks.yaml", with_response_file).await;
    let (syntax_status, syntax_reason) = server.import("mocks.json", "{ not json").await;
    let no_file = request()
        .method("POST")
        .path("/import")
        .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(format!("--{BOUNDARY}--\r\n"))
        .reply(&server.routes())
        .await;

    assert_eq!(file_status, StatusCode::BAD_REQUEST);
    assert!(file_reason.as_str().is_some_and(|reason| reason.contains("endpoint 2 (/from-file)") && reason.contains("body")));
    assert_eq!(syntax_status, StatusCode::BAD_REQUEST);
    assert!(syntax_reason.as_str().is_some_and(|reason| reason.contains("not a valid mocks file")));
    assert_eq!(no_file.status(), StatusCode::BAD_REQUEST);
    assert!(server.endpoints.is_empty());
}

#[test]
fn test_binary_does_not_start_with_a_mocks_file_it_cannot_load() {
    let output = Command::new(env!("CARGO_BIN_EXE_mockiapi"))
        .args(["--port", "39171", "--mocks", "tests/fixtures/mocks/absent.yaml"])
        .output()
        .expect("the binary could not be run");
    let errors = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(1));
    assert!(errors.contains("could not load tests/fixtures/mocks/absent.yaml"));
    assert!(output.stdout.is_empty());
}

#[test]
fn test_syntax_of_an_upload_comes_from_its_file_name() {
    assert_eq!(Syntax::of("shop.postman_collection.json"), Syntax::Json);
    assert_eq!(Syntax::of("mocks.yml"), Syntax::Yaml);
}
