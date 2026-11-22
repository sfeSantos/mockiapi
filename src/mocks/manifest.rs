//! mockiapi's own mocks file. The same shape is read from JSON and from YAML.

use bytes::Bytes;
use serde::Deserialize;
use serde_json::Value;
use crate::mocks::{MockEntry, MocksError, Parsed, ResponseSource};
use crate::models::{AuthData, EndpointKind, RateLimit};

/// The whole file. A field nobody knows is refused, which catches a misspelled one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    endpoints: Vec<ManifestEntry>,
}

/// One endpoint, with the names `POST /register` and `GET /list` use.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestEntry {
    path: String,
    #[serde(default, alias = "method")]
    methods: Vec<String>,
    group: Option<String>,
    #[serde(rename = "type")]
    kind: Option<EndpointKind>,
    status_code: Option<u16>,
    delay: Option<u64>,
    rate_limit: Option<RateLimitSpec>,
    authentication: Option<AuthData>,
    #[serde(default)]
    with_dynamic_vars: bool,
    #[serde(rename = "grpcService")]
    grpc_service: Option<String>,
    #[serde(rename = "grpcRPC")]
    grpc_rpc: Option<String>,
    /// The response, in a file named relative to the mocks file.
    file: Option<String>,
    /// The response, written here.
    body: Option<Value>,
}

/// A rate limit as the form takes it (`10/1000`) or as `/list` shows it.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RateLimitSpec {
    Text(String),
    Parts(RateLimit),
}

impl RateLimitSpec {
    fn into_text(self) -> String {
        match self {
            RateLimitSpec::Text(text) => text,
            RateLimitSpec::Parts(limit) => format!("{}/{}", limit.requests, limit.window_ms),
        }
    }
}

pub fn entries(manifest: Manifest) -> Result<Parsed, MocksError> {
    let entries = manifest.endpoints
        .into_iter()
        .enumerate()
        .map(|(index, entry)| entry.into_mock(index + 1))
        .collect::<Result<_, _>>()?;

    Ok(Parsed { entries, warnings: Vec::new() })
}

impl ManifestEntry {
    fn into_mock(self, position: usize) -> Result<MockEntry, MocksError> {
        let refused = |reason: &str| MocksError::Entry { position, path: self.path.clone(), reason: reason.to_string() };
        let kind = match (self.kind, self.grpc_service.is_some()) {
            (None, true) | (Some(EndpointKind::Grpc), true) => EndpointKind::Grpc,
            (Some(EndpointKind::Grpc), false) => {
                return Err(refused("a gRPC endpoint needs grpcService and grpcRPC"));
            }
            (Some(_), true) => return Err(refused("grpcService is only for an endpoint of type gRPC")),
            (kind, false) => kind.unwrap_or(EndpointKind::Rest),
        };
        let grpc = match (self.grpc_service, self.grpc_rpc) {
            (Some(service), Some(rpc)) => Some((service, rpc)),
            (Some(_), None) => return Err(refused("grpcRPC is required when grpcService is set")),
            (None, _) => None,
        };
        let response = match (self.file, self.body) {
            (Some(file), None) => ResponseSource::File(file),
            (None, Some(body)) => ResponseSource::Inline(inline_response(body)),
            (Some(_), Some(_)) => return Err(refused("give the response as `file` or as `body`, not both")),
            (None, None) => return Err(refused("a response is required, as `file` or as `body`")),
        };

        Ok(MockEntry {
            path: self.path,
            methods: methods_or_default(self.methods, kind),
            group: self.group,
            kind,
            status_code: self.status_code,
            delay: self.delay,
            rate_limit: self.rate_limit.map(RateLimitSpec::into_text),
            authentication: self.authentication,
            with_dynamic_vars: self.with_dynamic_vars,
            grpc,
            response,
        })
    }
}

/// A REST endpoint answers GET unless it says otherwise. GraphQL and gRPC are always POST.
fn methods_or_default(methods: Vec<String>, kind: EndpointKind) -> Vec<String> {
    if methods.is_empty() {
        let method = if kind == EndpointKind::Rest { "GET" } else { "POST" };

        return vec![method.to_string()];
    }

    methods
        .iter()
        .map(|method| method.trim().to_ascii_uppercase())
        .collect()
}

/// Text is taken as it is, so a template with bare `{{name}}` placeholders, which is not
/// valid JSON, can be written as a string. Anything else is the JSON it describes.
fn inline_response(body: Value) -> Bytes {
    match body {
        Value::String(text) => Bytes::from(text),
        // Serializing a `Value` cannot fail: its keys are strings.
        other => Bytes::from(serde_json::to_vec_pretty(&other).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(yaml: &str) -> Result<Parsed, MocksError> {
        let manifest = serde_norway::from_str(yaml).map_err(|error| MocksError::Invalid(error.to_string()))?;

        entries(manifest)
    }

    fn only_entry(yaml: &str) -> MockEntry {
        parsed(yaml).unwrap().entries.remove(0)
    }

    fn reason(yaml: &str) -> String {
        match parsed(yaml).unwrap_err() {
            MocksError::Entry { reason, .. } | MocksError::Invalid(reason) => reason,
            MocksError::Unreadable(reason) => reason,
        }
    }

    #[test]
    fn minimal_entry_is_an_open_rest_endpoint_on_get() {
        let entry = only_entry("endpoints:\n  - path: /health\n    body: { status: ok }\n");

        assert_eq!(entry.path, "/health");
        assert_eq!(entry.methods, ["GET"]);
        assert_eq!(entry.kind, EndpointKind::Rest);
        assert_eq!(entry.status_code, None);
        assert!(!entry.with_dynamic_vars);
        assert!(matches!(entry.response, ResponseSource::Inline(ref body) if body.starts_with(b"{")));
    }

    #[test]
    fn every_setting_of_a_registration_can_be_written() {
        let entry = only_entry(
            "endpoints:
  - path: /users/{id}
    methods: [get, Put]
    group: users
    status_code: 201
    delay: 150
    rate_limit: 10/1000
    authentication: { tokenData: secret }
    with_dynamic_vars: true
    file: responses/user.json
",
        );

        assert_eq!(entry.methods, ["GET", "PUT"]);
        assert_eq!(entry.group.as_deref(), Some("users"));
        assert_eq!(entry.status_code, Some(201));
        assert_eq!(entry.delay, Some(150));
        assert_eq!(entry.rate_limit.as_deref(), Some("10/1000"));
        assert_eq!(entry.authentication.unwrap().token_data.as_deref(), Some("secret"));
        assert!(entry.with_dynamic_vars);
        assert_eq!(entry.response, ResponseSource::File("responses/user.json".to_string()));
    }

    #[test]
    fn rate_limit_is_also_read_the_way_list_shows_it() {
        let entry = only_entry(
            "endpoints:\n  - path: /limited\n    rate_limit: { requests: 2, window_ms: 500 }\n    body: {}\n",
        );

        assert_eq!(entry.rate_limit.as_deref(), Some("2/500"));
    }

    #[test]
    fn text_body_is_kept_as_it_is_for_templates() {
        let entry = only_entry("endpoints:\n  - path: /users/{id}\n    body: '{ \"id\": {{id}} }'\n");

        assert_eq!(entry.response, ResponseSource::Inline(Bytes::from_static(b"{ \"id\": {{id}} }")));
    }

    #[test]
    fn graphql_and_grpc_default_to_post() {
        let graphql = only_entry("endpoints:\n  - path: /api/graphql\n    type: GraphQL\n    body: {}\n");
        let grpc = only_entry(
            "endpoints:\n  - path: /grpc\n    grpcService: Books\n    grpcRPC: GetBook\n    body: { id: b1 }\n",
        );

        assert_eq!(graphql.kind, EndpointKind::GraphQl);
        assert_eq!(graphql.methods, ["POST"]);
        assert_eq!(grpc.kind, EndpointKind::Grpc);
        assert_eq!(grpc.methods, ["POST"]);
        assert_eq!(grpc.grpc, Some(("Books".to_string(), "GetBook".to_string())));
    }

    #[test]
    fn entry_that_cannot_be_an_endpoint_is_refused_with_a_reason() {
        assert!(reason("endpoints:\n  - path: /a\n").contains("response is required"));
        assert!(reason("endpoints:\n  - path: /a\n    file: a.json\n    body: {}\n").contains("not both"));
        assert!(reason("endpoints:\n  - path: /a\n    type: gRPC\n    body: {}\n").contains("grpcService"));
        assert!(reason("endpoints:\n  - path: /a\n    grpcService: Books\n    body: {}\n").contains("grpcRPC"));
        assert!(reason("endpoints:\n  - path: /a\n    type: REST\n    grpcService: B\n    grpcRPC: G\n    body: {}\n")
            .contains("type gRPC"));
    }

    #[test]
    fn misspelled_field_and_missing_path_are_refused() {
        assert!(reason("endpoints:\n  - path: /a\n    statuscode: 200\n    body: {}\n").contains("statuscode"));
        assert!(reason("endpoints:\n  - body: {}\n").contains("path"));
    }

    #[test]
    fn refusal_names_the_position_and_the_path_of_the_entry() {
        let error = parsed("endpoints:\n  - path: /ok\n    body: {}\n  - path: /broken\n").unwrap_err();

        assert!(matches!(error, MocksError::Entry { position: 2, ref path, .. } if path == "/broken"));
    }
}
