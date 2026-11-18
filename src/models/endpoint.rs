use bytes::Bytes;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use warp::http::{Method, StatusCode};
use crate::models::AuthData;

/// Protocol an endpoint was registered for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointKind {
    #[serde(rename = "REST")]
    Rest,
    #[serde(rename = "GraphQL")]
    GraphQl,
    #[serde(rename = "gRPC")]
    Grpc,
}

/// What an endpoint answers with, prepared once at registration so that serving a
/// request reads no file and parses no mock.
#[derive(Debug, Clone)]
pub enum MockBody {
    /// REST: the uploaded file, sent as it is or used as a template for dynamic variables.
    Raw(Bytes),
    /// GraphQL: the uploaded mock document, already parsed.
    GraphQl(Value),
    /// gRPC: the answers live in the gRPC registry, keyed by service and method.
    Grpc,
}

impl MockBody {
    pub fn kind(&self) -> EndpointKind {
        match self {
            MockBody::Raw(_) => EndpointKind::Rest,
            MockBody::GraphQl(_) => EndpointKind::GraphQl,
            MockBody::Grpc => EndpointKind::Grpc,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Endpoint {
    /// Listed as the endpoint type. The content itself is never sent to the dashboard.
    #[serde(rename = "type", serialize_with = "serialize_kind")]
    pub body: MockBody,
    pub method: Vec<String>,
    /// Path of the uploaded file on disk, kept so it can be removed with the endpoint.
    pub file: String,
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<AuthData>,
    pub delay: Option<u64>,
    pub rate_limit: Option<RateLimit>,
    pub with_dynamic_vars: Option<bool>,
}

impl Endpoint {
    pub fn kind(&self) -> EndpointKind {
        self.body.kind()
    }

    pub fn allows(&self, method: &Method) -> bool {
        self.method
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(method.as_str()))
    }

    /// The status to answer with. Registration only stores codes that are valid.
    pub fn status(&self) -> StatusCode {
        self.status_code
            .and_then(|code| StatusCode::from_u16(code).ok())
            .unwrap_or(StatusCode::OK)
    }
}

fn serialize_kind<S: Serializer>(body: &MockBody, serializer: S) -> Result<S::Ok, S::Error> {
    body.kind().serialize(serializer)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimit {
    pub requests: usize,
    pub window_ms: u64,
}
