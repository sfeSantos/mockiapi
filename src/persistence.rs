//! The registered endpoints, written to a file so they are still there after a restart.
//!
//! The file holds the settings of every endpoint and the name of its response file in
//! `uploads/`. It is rewritten whole when an endpoint is registered, changed or deleted,
//! and read once at startup. Serving a mock request never touches it.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::Mutex;
use crate::handlers::multipart::{Registration, RegistrationForm};
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{AuthData, EndpointKind, Endpoints, MockBody, RateLimit};
use crate::routing::Route;

/// Where the server keeps its endpoints, next to their response files.
pub const ENDPOINTS_FILE: &str = "uploads/endpoints.json";

/// The shape of the file. A change that an older server could not read gets a new number.
const VERSION: u32 = 1;

/// The whole file.
#[derive(Debug, Serialize, Deserialize)]
struct StoredEndpoints {
    version: u32,
    endpoints: Vec<StoredEndpoint>,
}

/// Only the version, read first so that a file from a newer server is named as such
/// instead of being refused for a field this one does not know.
#[derive(Debug, Deserialize)]
struct Versioned {
    version: u32,
}

/// One endpoint as it is kept on disk, with the names `GET /list` uses.
///
/// The response itself stays in its own file. For a gRPC mock the service and the RPC are
/// kept too, since the answers of the registry are built from them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEndpoint {
    id: u64,
    path: String,
    #[serde(rename = "type")]
    kind: EndpointKind,
    method: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status_code: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authentication: Option<AuthData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delay: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rate_limit: Option<RateLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    with_dynamic_vars: Option<bool>,
    #[serde(default, rename = "grpcService", skip_serializing_if = "Option::is_none")]
    grpc_service: Option<String>,
    #[serde(default, rename = "grpcRPC", skip_serializing_if = "Option::is_none")]
    grpc_rpc: Option<String>,
}

impl StoredEndpoint {
    fn of(route: &Route) -> Self {
        let endpoint = &route.endpoint;
        let (grpc_service, grpc_rpc) = match &endpoint.body {
            MockBody::Grpc { service, rpc } => (Some(service.clone()), Some(rpc.clone())),
            MockBody::Raw(_) | MockBody::GraphQl(_) => (None, None),
        };

        Self {
            id: route.id,
            path: route.path.clone(),
            kind: endpoint.kind(),
            method: endpoint.method.clone(),
            group: endpoint.group.clone(),
            file: endpoint.file.clone(),
            status_code: endpoint.status_code,
            authentication: endpoint.authentication.clone(),
            delay: endpoint.delay,
            rate_limit: endpoint.rate_limit.clone(),
            with_dynamic_vars: endpoint.with_dynamic_vars,
            grpc_service,
            grpc_rpc,
        }
    }

    /// How a warning names the endpoint.
    fn label(&self) -> String {
        format!("{} {}", self.method.join(","), self.path)
    }

    /// Builds the endpoint again from its settings and its response file.
    ///
    /// It goes through the validation a registration over HTTP goes through, which is what
    /// parses a GraphQL document and prepares a gRPC answer. The response is already where
    /// the endpoint says, so nothing is copied or written.
    async fn registration(self) -> Result<Registration, String> {
        let response = fs::read(&self.file)
            .await
            .map_err(|error| format!("the response file {} could not be read: {error}", self.file))?;
        let file = self.file.clone();
        let mut registration = self.into_form(Bytes::from(response))
            .validate(None)
            .map_err(|error| error.reason().to_string())?;

        registration.endpoint.file = file;
        registration.upload = None;

        Ok(registration)
    }

    /// The endpoint as the form the dashboard would send for it.
    fn into_form(self, response: Bytes) -> RegistrationForm {
        RegistrationForm {
            id: None,
            path: Some(self.path),
            group: self.group,
            methods: Some(self.method.join(",")),
            status_code: self.status_code.map(|code| code.to_string()),
            delay: self.delay.map(|delay| delay.to_string()),
            rate_limit: self.rate_limit.map(|limit| format!("{}/{}", limit.requests, limit.window_ms)),
            authentication: self.authentication
                .and_then(|authentication| serde_json::to_string(&authentication).ok()),
            with_dynamic_vars: self.with_dynamic_vars.map(|flag| flag.to_string()),
            is_graphql: Some((self.kind == EndpointKind::GraphQl).to_string()),
            grpc_service: self.grpc_service,
            grpc_rpc: self.grpc_rpc,
            file: Some(response),
        }
    }
}

/// Why the saved endpoints could not be read. Nothing is restored when this is returned.
#[derive(Debug, PartialEq, Eq)]
pub enum PersistenceError {
    Unreadable(String),
    /// The file is not valid JSON, or does not have the shape the server writes.
    Invalid(String),
    /// The file was written by a server that keeps its endpoints in another shape.
    UnknownVersion(u32),
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistenceError::Unreadable(reason) => write!(formatter, "the file could not be read: {reason}"),
            PersistenceError::Invalid(reason) => write!(formatter, "the file is not valid: {reason}"),
            PersistenceError::UnknownVersion(version) => {
                write!(formatter, "the file has version {version}, and this server reads version {VERSION}")
            }
        }
    }
}

/// The saved endpoints as the text of the file.
pub fn snapshot<'routes>(routes: impl Iterator<Item = &'routes Arc<Route>>) -> Vec<u8> {
    let stored = StoredEndpoints {
        version: VERSION,
        endpoints: routes
            .map(|route| StoredEndpoint::of(route))
            .collect(),
    };

    // Serializing cannot fail: the keys are strings and every value is plain data.
    serde_json::to_vec_pretty(&stored).unwrap_or_default()
}

/// Reads the text of the file.
pub fn parse(content: &[u8]) -> Result<Vec<StoredEndpoint>, PersistenceError> {
    let invalid = |error: serde_json::Error| PersistenceError::Invalid(error.to_string());
    let versioned: Versioned = serde_json::from_slice(content).map_err(invalid)?;

    if versioned.version != VERSION {
        return Err(PersistenceError::UnknownVersion(versioned.version));
    }

    let stored: StoredEndpoints = serde_json::from_slice(content).map_err(invalid)?;

    Ok(stored.endpoints)
}

/// The file the endpoints are saved in.
#[derive(Debug)]
pub struct EndpointsFile {
    path: PathBuf,
    /// Held for a whole write, so two changes made at once cannot leave the older one on disk.
    writing: Mutex<()>,
}

impl EndpointsFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), writing: Mutex::new(()) }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The endpoints the file holds. A file that does not exist yet holds none.
    pub async fn read(&self) -> Result<Vec<StoredEndpoint>, PersistenceError> {
        match fs::read(&self.path).await {
            Ok(content) => parse(&content),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(PersistenceError::Unreadable(error.to_string())),
        }
    }

    /// Replaces the file with what `snapshot` returns.
    ///
    /// The snapshot is taken once the previous write is over, so the last write to finish
    /// is also the one that saw the latest endpoints. The text goes to a temporary file
    /// that then takes the place of the real one: a crash in the middle leaves the previous
    /// file whole.
    pub async fn write(&self, snapshot: impl FnOnce() -> Vec<u8> + Send) -> io::Result<()> {
        let _writing = self.writing.lock().await;
        let content = snapshot();
        let temporary = self.temporary_path();

        fs::write(&temporary, content).await?;

        fs::rename(&temporary, &self.path).await
    }

    fn temporary_path(&self) -> PathBuf {
        let mut name = self.path
            .file_name()
            .unwrap_or_default()
            .to_os_string();
        name.push(".tmp");

        self.path.with_file_name(name)
    }
}

/// What reading the saved endpoints at startup did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Restored {
    pub loaded: usize,
    /// One line for each endpoint that was left out, with the reason.
    pub skipped: Vec<String>,
}

/// Registers the saved endpoints again, each under the id it had.
///
/// An endpoint whose response file is gone, or no longer fits its type, is skipped and
/// reported, and the others are restored. Endpoints kept in memory only have nothing saved.
pub async fn restore(endpoints: &Endpoints, grpc_registry: &GrpcRegistry) -> Result<Restored, PersistenceError> {
    let Some(file) = endpoints.saved_file() else {
        return Ok(Restored::default());
    };
    let mut restored = Restored::default();

    for stored in file.read().await? {
        let id = stored.id;
        let label = stored.label();

        match stored.registration().await {
            Ok(registration) => {
                if let Some(grpc) = registration.grpc {
                    grpc_registry.register_mock(&grpc.service, &grpc.rpc, grpc.response);
                }

                endpoints.insert(registration.path, registration.endpoint, Some(id));
                restored.loaded += 1;
            }
            Err(reason) => restored.skipped.push(format!("{label}: {reason}")),
        }
    }

    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(content: &str) -> Result<Vec<StoredEndpoint>, PersistenceError> {
        parse(content.as_bytes())
    }

    #[test]
    fn reads_an_endpoint_with_only_the_fields_it_needs() {
        let endpoints = stored(
            r#"{ "version": 1, "endpoints": [
                { "id": 4, "path": "/health", "type": "REST", "method": ["GET"], "file": "uploads/health.json" }
            ] }"#,
        )
        .unwrap();

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].id, 4);
        assert_eq!(endpoints[0].label(), "GET /health");
        assert_eq!(endpoints[0].status_code, None);
        assert_eq!(endpoints[0].with_dynamic_vars, None);
    }

    #[test]
    fn every_setting_survives_being_written_and_read() {
        let content = r#"{ "version": 1, "endpoints": [ {
            "id": 7, "path": "/grpc", "type": "gRPC", "method": ["POST"], "group": "books",
            "file": "uploads/book.json", "status_code": 201, "authentication": { "tokenData": "secret" },
            "delay": 150, "rate_limit": { "requests": 10, "window_ms": 1000 }, "with_dynamic_vars": true,
            "grpcService": "Books", "grpcRPC": "GetBook"
        } ] }"#;
        let endpoints = stored(content).unwrap();
        let written = serde_json::to_vec(&StoredEndpoints { version: VERSION, endpoints: endpoints.clone() }).unwrap();

        assert_eq!(parse(&written).unwrap(), endpoints);
        assert_eq!(endpoints[0].grpc_service.as_deref(), Some("Books"));
        assert_eq!(endpoints[0].rate_limit, Some(RateLimit { requests: 10, window_ms: 1000 }));
    }

    #[test]
    fn form_of_a_stored_endpoint_carries_its_settings() {
        let endpoint = stored(
            r#"{ "version": 1, "endpoints": [ {
                "id": 1, "path": "/users", "type": "GraphQL", "method": ["POST"], "file": "uploads/users.json",
                "status_code": 201, "delay": 50, "rate_limit": { "requests": 2, "window_ms": 500 }
            } ] }"#,
        )
        .unwrap()
        .remove(0);
        let form = endpoint.into_form(Bytes::from_static(b"{}"));

        assert_eq!(form.methods.as_deref(), Some("POST"));
        assert_eq!(form.status_code.as_deref(), Some("201"));
        assert_eq!(form.delay.as_deref(), Some("50"));
        assert_eq!(form.rate_limit.as_deref(), Some("2/500"));
        assert_eq!(form.is_graphql.as_deref(), Some("true"));
        assert_eq!(form.with_dynamic_vars, None);
    }

    #[test]
    fn file_of_another_version_is_named_as_such() {
        let newer = stored(r#"{ "version": 2, "sets": {} }"#).unwrap_err();

        assert_eq!(newer, PersistenceError::UnknownVersion(2));
        assert!(newer.to_string().contains("version 2"));
    }

    #[test]
    fn text_that_is_not_the_file_is_refused_with_the_place() {
        let broken = stored("{ \"version\": 1, \"endpoints\": [ { \"id\": } ] }").unwrap_err();
        let wrong_shape = stored(r#"{ "version": 1 }"#).unwrap_err();

        assert!(matches!(broken, PersistenceError::Invalid(ref reason) if reason.contains("line 1")));
        assert!(matches!(wrong_shape, PersistenceError::Invalid(ref reason) if reason.contains("endpoints")));
    }

    #[test]
    fn temporary_file_sits_next_to_the_real_one() {
        let file = EndpointsFile::new("uploads/endpoints.json");

        assert_eq!(file.temporary_path(), Path::new("uploads").join("endpoints.json.tmp"));
    }
}
