//! Endpoints described in a file, registered at startup or sent to `POST /import`.
//!
//! Three kinds of file are read: mockiapi's own format in JSON or in YAML, and a Postman
//! collection. Each one is turned into the same list of entries, and every entry goes
//! through the validation and the storage a registration over HTTP goes through, so an
//! endpoint that came from a file is no different from any other.

pub mod manifest;
pub mod postman;

use std::fmt;
use std::path::Path;
use bytes::Bytes;
use serde::Serialize;
use serde_json::Value;
use tokio::fs;
use crate::handlers::multipart::RegistrationForm;
use crate::handlers::registration::register;
use crate::middlewares::grpc_registry::GrpcRegistry;
use crate::models::{AuthData, EndpointKind, Endpoints};

/// How the text of a mocks file is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Json,
    Yaml,
}

impl Syntax {
    /// Goes by the extension: `.yaml` and `.yml` are YAML, anything else is JSON.
    pub fn of(file_name: &str) -> Self {
        let extension = Path::new(file_name)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();

        if extension.eq_ignore_ascii_case("yaml") || extension.eq_ignore_ascii_case("yml") {
            Syntax::Yaml
        } else {
            Syntax::Json
        }
    }
}

/// Where the response of an entry comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseSource {
    /// Written in the mocks file itself.
    Inline(Bytes),
    /// A file, named relative to the mocks file.
    File(String),
}

/// One endpoint to register, whatever format it was written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockEntry {
    pub path: String,
    /// Uppercased, and never empty.
    pub methods: Vec<String>,
    pub group: Option<String>,
    pub kind: EndpointKind,
    pub status_code: Option<u16>,
    pub delay: Option<u64>,
    /// `<requests>/<window in ms>`, as the registration form takes it.
    pub rate_limit: Option<String>,
    pub authentication: Option<AuthData>,
    pub with_dynamic_vars: bool,
    /// The service and the RPC of a gRPC mock.
    pub grpc: Option<(String, String)>,
    pub response: ResponseSource,
}

impl MockEntry {
    /// The entry as the form the dashboard would send for it.
    fn into_form(self, response: Bytes) -> RegistrationForm {
        let (grpc_service, grpc_rpc) = self.grpc.unzip();

        RegistrationForm {
            id: None,
            path: Some(self.path),
            group: self.group,
            methods: Some(self.methods.join(",")),
            status_code: self.status_code.map(|code| code.to_string()),
            delay: self.delay.map(|delay| delay.to_string()),
            rate_limit: self.rate_limit,
            authentication: self.authentication
                .and_then(|authentication| serde_json::to_string(&authentication).ok()),
            with_dynamic_vars: Some(self.with_dynamic_vars.to_string()),
            is_graphql: Some((self.kind == EndpointKind::GraphQl).to_string()),
            grpc_service,
            grpc_rpc,
            file: Some(response),
        }
    }

    fn shares_a_method_with(&self, other: &MockEntry) -> Option<&str> {
        self.methods
            .iter()
            .find(|method| other.methods.contains(method))
            .map(String::as_str)
    }
}

/// What a mocks file holds once it has been read.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub entries: Vec<MockEntry>,
    /// Things that were left out on purpose, such as a Postman request with no example.
    pub warnings: Vec<String>,
}

/// What loading a mocks file did.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Loaded {
    pub registered: usize,
    pub warnings: Vec<String>,
}

/// Why a mocks file was not loaded. Nothing of it is registered when this is returned.
#[derive(Debug, PartialEq, Eq)]
pub enum MocksError {
    Unreadable(String),
    /// The file is not valid JSON or YAML, or does not have the shape of a mocks file.
    Invalid(String),
    /// One endpoint of the file cannot be registered. `position` counts from 1.
    Entry { position: usize, path: String, reason: String },
}

impl fmt::Display for MocksError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MocksError::Unreadable(reason) => write!(formatter, "the file could not be read: {reason}"),
            MocksError::Invalid(reason) => write!(formatter, "the file is not a valid mocks file: {reason}"),
            MocksError::Entry { position, path, reason } => {
                write!(formatter, "endpoint {position} ({path}): {reason}")
            }
        }
    }
}

/// Reads the text of a mocks file. A JSON file that is a Postman collection is converted.
pub fn parse(content: &[u8], syntax: Syntax) -> Result<Parsed, MocksError> {
    match syntax {
        Syntax::Yaml => {
            let manifest = serde_norway::from_slice(content).map_err(invalid)?;

            manifest::entries(manifest)
        }
        Syntax::Json => {
            let document: Value = serde_json::from_slice(content).map_err(invalid)?;

            if postman::is_collection(&document) {
                return Ok(postman::entries(&document));
            }

            // Read again from the text: the error then says the line and the column.
            let manifest = serde_json::from_slice(content).map_err(invalid)?;

            manifest::entries(manifest)
        }
    }
}

fn invalid(error: impl fmt::Display) -> MocksError {
    MocksError::Invalid(error.to_string())
}

/// Registers the endpoints of a file on disk. Response files are found next to it.
pub async fn load_file(
    file: &Path,
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<Loaded, MocksError> {
    let content = fs::read(file)
        .await
        .map_err(|error| MocksError::Unreadable(error.to_string()))?;
    let syntax = Syntax::of(&file.to_string_lossy());
    let directory = file.parent().unwrap_or(Path::new(""));

    register_all(parse(&content, syntax)?, Some(directory), endpoints, grpc_registry).await
}

/// Registers the endpoints of a mocks file that was uploaded, which has no directory of
/// its own: every response has to be written in the file.
pub async fn import(
    content: &[u8],
    syntax: Syntax,
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<Loaded, MocksError> {
    register_all(parse(content, syntax)?, None, endpoints, grpc_registry).await
}

/// Checks every entry before storing any, so a file is registered whole or not at all.
async fn register_all(
    parsed: Parsed,
    directory: Option<&Path>,
    endpoints: &Endpoints,
    grpc_registry: &GrpcRegistry,
) -> Result<Loaded, MocksError> {
    let mut forms = Vec::with_capacity(parsed.entries.len());

    for (index, entry) in parsed.entries.iter().enumerate() {
        let refused = |reason: String| MocksError::Entry { position: index + 1, path: entry.path.clone(), reason };

        if let Some(reason) = clash_with_earlier(entry, &parsed.entries[..index]) {
            return Err(refused(reason));
        }

        let response = read_response(&entry.response, directory)
            .await
            .map_err(refused)?;
        let form = entry.clone().into_form(response);

        // Cloning the form shares the response bytes, it does not copy them.
        if let Err(error) = form.clone().validate(None) {
            return Err(refused(error.reason().to_string()));
        }

        forms.push((index + 1, form));
    }

    let registered = forms.len();

    for (position, form) in forms {
        let path = form.path.clone().unwrap_or_default();

        register(form, endpoints, grpc_registry)
            .await
            .map_err(|error| MocksError::Entry { position, path, reason: error.reason().to_string() })?;
    }

    Ok(Loaded { registered, warnings: parsed.warnings })
}

/// Two entries of one file that answer the same method on the same path would silently
/// replace each other, which is never what the file meant.
fn clash_with_earlier(entry: &MockEntry, earlier: &[MockEntry]) -> Option<String> {
    earlier
        .iter()
        .enumerate()
        .filter(|(_, other)| other.path == entry.path)
        .find_map(|(index, other)| entry
            .shares_a_method_with(other)
            .map(|method| format!("{method} {} is already answered by endpoint {} of this file", entry.path, index + 1)))
}

async fn read_response(source: &ResponseSource, directory: Option<&Path>) -> Result<Bytes, String> {
    match (source, directory) {
        (ResponseSource::Inline(content), _) => Ok(content.clone()),
        (ResponseSource::File(name), Some(directory)) => fs::read(directory.join(name))
            .await
            .map(Bytes::from)
            .map_err(|error| format!("the response file {name} could not be read: {error}")),
        (ResponseSource::File(_), None) => {
            Err("`file` only works for a mocks file given with --mocks. Write the response in `body`".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn syntax_goes_by_the_extension() {
        assert_eq!(Syntax::of("mocks.yaml"), Syntax::Yaml);
        assert_eq!(Syntax::of("mocks.YML"), Syntax::Yaml);
        assert_eq!(Syntax::of("mocks.json"), Syntax::Json);
        assert_eq!(Syntax::of("api.postman_collection.json"), Syntax::Json);
        assert_eq!(Syntax::of("mocks"), Syntax::Json);
    }

    #[test]
    fn json_and_yaml_describe_the_same_endpoints() {
        let json = br#"{ "endpoints": [
            { "path": "/users/{id}", "methods": ["get"], "group": "users", "status_code": 200, "body": { "id": 1 } }
        ] }"#;
        let yaml = b"endpoints:\n  - path: /users/{id}\n    methods: [get]\n    group: users\n    status_code: 200\n    body:\n      id: 1\n";

        let from_json = parse(json, Syntax::Json).unwrap();
        let from_yaml = parse(yaml, Syntax::Yaml).unwrap();

        assert_eq!(from_json, from_yaml);
        assert_eq!(from_json.entries[0].methods, ["GET"]);
    }

    #[test]
    fn text_that_is_not_a_mocks_file_is_refused_with_the_place() {
        let broken_json = parse(b"{ \"endpoints\": [ { \"path\": } ] }", Syntax::Json).unwrap_err();
        let broken_yaml = parse(b"endpoints:\n  - path: [", Syntax::Yaml).unwrap_err();
        let wrong_shape = parse(b"{ \"routes\": [] }", Syntax::Json).unwrap_err();

        assert!(matches!(broken_json, MocksError::Invalid(ref reason) if reason.contains("line 1")));
        assert!(matches!(broken_yaml, MocksError::Invalid(_)));
        assert!(matches!(wrong_shape, MocksError::Invalid(ref reason) if reason.contains("routes")));
    }

    #[test]
    fn postman_collection_is_recognised_inside_json() {
        let collection = br#"{
            "info": { "name": "Shop", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
            "item": [ { "name": "List", "request": { "method": "GET", "url": "{{baseUrl}}/users" },
                        "response": [ { "code": 200, "body": "[]" } ] } ]
        }"#;

        let parsed = parse(collection, Syntax::Json).unwrap();

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].path, "/users");
    }
}
