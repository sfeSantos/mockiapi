use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde_json::{json, Value};
use warp::http::StatusCode;
use warp::test::request;
use warp::{Filter, Rejection, Reply};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::mocks;
use mockiapi::models::Endpoints;
use mockiapi::persistence::{self, PersistenceError, Restored};
use mockiapi::routes::{admin_routes, mock_routes};

const BOUNDARY: &str = "mockiapi-test-boundary";
const GRAPHQL_MOCK: &str = r#"{ "query": { "getUser": { "data": { "id": "123", "name": "John Doe" } } } }"#;
const BOOK_CALL: &str = r#"{ "service": "Books", "rpc": "GetBook", "request": {} }"#;

/// An endpoints file of its own for a test, in a directory that starts out empty.
fn endpoints_file(test: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("mockiapi-persistence-{}-{test}", std::process::id()));

    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the test directory could not be created");

    directory.join("endpoints.json")
}

/// A server over an endpoints file, built from the same filters the binary serves.
struct Server {
    endpoints: Endpoints,
    registry: Arc<GrpcRegistry>,
}

impl Server {
    /// Starts the way the binary does: an empty store, then whatever the file holds.
    async fn start(file: &Path) -> (Self, Restored) {
        let (server, restored) = Self::try_start(file).await;

        (server, restored.expect("the saved endpoints could not be restored"))
    }

    async fn try_start(file: &Path) -> (Self, Result<Restored, PersistenceError>) {
        let server = Self { endpoints: Endpoints::saved_in(file), registry: Arc::new(GrpcRegistry::new()) };
        let restored = persistence::restore(&server.endpoints, &server.registry).await;

        (server, restored)
    }

    fn routes(&self) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone + 'static {
        admin_routes(self.endpoints.clone(), Arc::clone(&self.registry))
            .or(mock_routes(self.endpoints.clone(), Arc::clone(&self.registry)))
    }

    async fn call(&self, method: &str, path: &str, body: &str) -> (StatusCode, String) {
        let response = request()
            .method(method)
            .path(path)
            .body(body)
            .reply(&self.routes())
            .await;

        (response.status(), String::from_utf8_lossy(response.body()).into_owned())
    }

    async fn get(&self, path: &str) -> (StatusCode, String) {
        self.call("GET", path, "").await
    }

    async fn register(&self, fields: &[(&str, &str)]) {
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

        let response = request()
            .method("POST")
            .path("/register")
            .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
            .body(body)
            .reply(&self.routes())
            .await;

        assert_eq!(response.status(), StatusCode::OK, "{}", String::from_utf8_lossy(response.body()));
    }

    async fn list(&self) -> Vec<Value> {
        let (_, body) = self.get("/list").await;

        serde_json::from_str(&body).expect("/list did not answer a JSON list")
    }

    /// The file an endpoint keeps its response in.
    async fn upload_of(&self, path: &str) -> PathBuf {
        let listed = self.list().await;
        let endpoint = listed
            .iter()
            .find(|endpoint| endpoint["path"] == path)
            .unwrap_or_else(|| panic!("{path} is not registered"));

        PathBuf::from(endpoint["file"].as_str().expect("the endpoint lists no file"))
    }

    async fn delete(&self, path: &str) -> StatusCode {
        self.call("DELETE", &format!("/delete/{}", urlencoding::encode(path)), "")
            .await
            .0
    }

    /// Deletes every endpoint, which also removes their responses from `uploads/`.
    async fn clear(&self) {
        for endpoint in self.list().await {
            self.call("DELETE", &format!("/delete/{}", endpoint["id"]), "").await;
        }
    }
}

/// What the endpoints file holds, as the server wrote it.
fn saved(file: &Path) -> Value {
    let content = std::fs::read(file).expect("the endpoints file was not written");

    serde_json::from_slice(&content).expect("the endpoints file is not JSON")
}

fn saved_paths(file: &Path) -> Vec<String> {
    saved(file)["endpoints"]
        .as_array()
        .expect("the endpoints file holds no list")
        .iter()
        .map(|endpoint| endpoint["path"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio::test]
async fn endpoints_of_every_kind_answer_the_same_after_a_restart() {
    let file = endpoints_file("kinds");
    let (server, restored) = Server::start(&file).await;

    assert_eq!(restored, Restored::default());

    server
        .register(&[
            ("path", "/persist/kinds/users/{id}"),
            ("methods", "GET"),
            ("group", "users"),
            ("status_code", "201"),
            ("rate_limit", "50/1000"),
            ("authentication", r#"{"tokenData":"secret"}"#),
            ("with_dynamic_vars", "true"),
            ("file", r#"{ "id": "{{id}}" }"#),
        ])
        .await;
    server
        .register(&[
            ("path", "/persist/kinds/graphql"),
            ("methods", "POST"),
            ("isGraphQL", "true"),
            ("file", GRAPHQL_MOCK),
        ])
        .await;
    server
        .register(&[
            ("path", "/persist/kinds/grpc"),
            ("methods", "POST"),
            ("status_code", "202"),
            ("grpcService", "Books"),
            ("grpcRPC", "GetBook"),
            ("file", r#"{ "id": "b123" }"#),
        ])
        .await;

    let graphql_call = json!({ "query": "query getUser { id }" }).to_string();
    let listed_before = server.list().await;
    let graphql_before = server.call("POST", "/persist/kinds/graphql", &graphql_call).await;
    let grpc_before = server.call("POST", "/persist/kinds/grpc", BOOK_CALL).await;

    let (restarted, restored) = Server::start(&file).await;

    assert_eq!(restored, Restored { loaded: 3, skipped: Vec::new() });
    assert_eq!(restarted.list().await, listed_before);

    // The REST endpoint came back with its authentication, its status and its template.
    let refused = restarted.get("/persist/kinds/users/7").await;
    let answered = request()
        .path("/persist/kinds/users/7")
        .header("authorization", "Bearer secret")
        .reply(&restarted.routes())
        .await;

    assert_eq!(refused.0, StatusCode::UNAUTHORIZED);
    assert_eq!(answered.status(), StatusCode::CREATED);
    assert_eq!(String::from_utf8_lossy(answered.body()), r#"{ "id": "7" }"#);

    assert_eq!(graphql_before, (StatusCode::OK, r#"{"data":{"id":"123"}}"#.to_string()));
    assert_eq!(restarted.call("POST", "/persist/kinds/graphql", &graphql_call).await, graphql_before);
    assert_eq!(grpc_before, (StatusCode::ACCEPTED, r#"{"id":"b123"}"#.to_string()));
    assert_eq!(restarted.call("POST", "/persist/kinds/grpc", BOOK_CALL).await, grpc_before);

    restarted.clear().await;
}

#[tokio::test]
async fn change_made_without_a_file_is_there_after_a_restart() {
    let file = endpoints_file("change");
    let (server, _) = Server::start(&file).await;

    server
        .register(&[("path", "/persist/change"), ("methods", "GET"), ("file", r#"{"kept":true}"#)])
        .await;
    server
        .register(&[("path", "/persist/change"), ("methods", "GET"), ("status_code", "503"), ("group", "down")])
        .await;

    let (restarted, _) = Server::start(&file).await;
    let listed = restarted.list().await;

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["status_code"], 503);
    assert_eq!(listed[0]["group"], "down");
    assert_eq!(
        restarted.get("/persist/change").await,
        (StatusCode::SERVICE_UNAVAILABLE, r#"{"kept":true}"#.to_string())
    );

    restarted.clear().await;
}

#[tokio::test]
async fn deleted_endpoint_stays_gone_and_leaves_no_file() {
    let file = endpoints_file("delete");
    let (server, _) = Server::start(&file).await;

    server
        .register(&[("path", "/persist/delete/gone"), ("methods", "GET"), ("file", "{}")])
        .await;
    server
        .register(&[("path", "/persist/delete/stays"), ("methods", "GET"), ("file", "{}")])
        .await;

    let upload = server.upload_of("/persist/delete/gone").await;

    assert!(upload.exists());
    assert_eq!(server.delete("/persist/delete/gone").await, StatusCode::OK);
    assert!(!upload.exists());
    assert_eq!(saved_paths(&file), ["/persist/delete/stays"]);

    let (restarted, restored) = Server::start(&file).await;

    assert_eq!(restored.loaded, 1);
    assert_eq!(restarted.get("/persist/delete/gone").await.0, StatusCode::NOT_FOUND);
    assert_eq!(restarted.get("/persist/delete/stays").await.0, StatusCode::OK);

    restarted.clear().await;

    assert_eq!(saved_paths(&file), Vec::<String>::new());
}

#[tokio::test]
async fn endpoint_whose_response_file_is_gone_is_skipped_and_the_others_are_served() {
    let file = endpoints_file("missing");
    let (server, _) = Server::start(&file).await;

    server
        .register(&[("path", "/persist/missing/lost"), ("methods", "GET"), ("file", "{}")])
        .await;
    server
        .register(&[("path", "/persist/missing/kept"), ("methods", "GET"), ("file", r#"{"ok":true}"#)])
        .await;
    std::fs::remove_file(server.upload_of("/persist/missing/lost").await).expect("the upload could not be removed");

    let (restarted, restored) = Server::start(&file).await;

    assert_eq!(restored.loaded, 1);
    assert_eq!(restored.skipped.len(), 1);
    assert!(restored.skipped[0].starts_with("GET /persist/missing/lost: the response file"));
    assert_eq!(restarted.get("/persist/missing/lost").await.0, StatusCode::NOT_FOUND);
    assert_eq!(restarted.get("/persist/missing/kept").await, (StatusCode::OK, r#"{"ok":true}"#.to_string()));

    restarted.clear().await;
}

#[tokio::test]
async fn ids_survive_a_restart_and_are_not_handed_out_again() {
    let file = endpoints_file("ids");
    let (server, _) = Server::start(&file).await;

    for path in ["/persist/ids/first", "/persist/ids/second", "/persist/ids/third"] {
        server
            .register(&[("path", path), ("methods", "GET"), ("file", "{}")])
            .await;
    }

    server.delete("/persist/ids/first").await;
    server.delete("/persist/ids/third").await;

    let (restarted, _) = Server::start(&file).await;
    restarted
        .register(&[("path", "/persist/ids/fourth"), ("methods", "GET"), ("file", "{}")])
        .await;
    let listed = restarted.list().await;

    assert_eq!(listed[0]["path"], "/persist/ids/second");
    assert_eq!(listed[0]["id"], 2);
    assert_eq!(listed[1]["path"], "/persist/ids/fourth");
    assert_eq!(listed[1]["id"], 3);

    restarted.clear().await;
}

#[tokio::test]
async fn mocks_file_is_registered_over_the_saved_endpoints() {
    let file = endpoints_file("mocks");
    let (server, _) = Server::start(&file).await;

    server
        .register(&[("path", "/api/users"), ("methods", "GET"), ("file", r#"{"from":"dashboard"}"#)])
        .await;
    server
        .register(&[("path", "/persist/mocks/own"), ("methods", "GET"), ("file", r#"{"own":true}"#)])
        .await;

    let (restarted, _) = Server::start(&file).await;
    let loaded = mocks::load_file(Path::new("tests/fixtures/mocks/mocks.json"), &restarted.endpoints, &restarted.registry)
        .await
        .expect("the mocks file could not be loaded");
    restarted.endpoints.save().await;

    assert_eq!(loaded.registered, 6);
    assert_eq!(restarted.list().await.len(), 7);
    assert!(restarted.get("/api/users").await.1.contains("Ada"));
    assert_eq!(restarted.get("/persist/mocks/own").await.1, r#"{"own":true}"#);
    assert_eq!(saved_paths(&file).len(), 7);

    restarted.clear().await;
}

#[tokio::test]
async fn file_that_cannot_be_read_is_refused_and_left_as_it_is() {
    let file = endpoints_file("broken");
    let newer = endpoints_file("newer");

    std::fs::write(&file, "{ not json").unwrap();
    std::fs::write(&newer, r#"{ "version": 2, "sets": [] }"#).unwrap();

    let (server, broken) = Server::try_start(&file).await;
    let (_, unknown) = Server::try_start(&newer).await;

    assert!(matches!(broken, Err(PersistenceError::Invalid(_))));
    assert_eq!(unknown, Err(PersistenceError::UnknownVersion(2)));
    assert!(server.endpoints.is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
}

#[tokio::test]
async fn endpoints_kept_in_memory_restore_and_save_nothing() {
    let endpoints = Endpoints::default();
    let registry = GrpcRegistry::new();

    endpoints.save().await;

    assert!(endpoints.saved_file().is_none());
    assert_eq!(persistence::restore(&endpoints, &registry).await, Ok(Restored::default()));
}
