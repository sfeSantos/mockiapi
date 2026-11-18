use std::sync::{Arc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use crate::routing::EndpointStore;

pub type Endpoints = Arc<Mutex<EndpointStore>>;

/// Protocol an endpoint was registered for. It decides how a request is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EndpointKind {
    #[default]
    #[serde(rename = "REST")]
    Rest,
    #[serde(rename = "GraphQL")]
    GraphQl,
    #[serde(rename = "gRPC")]
    Grpc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    #[serde(rename = "type", default)]
    pub kind: EndpointKind,
    pub method: Vec<String>,
    pub file: String,
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<String>,
    pub delay: Option<u64>,
    pub rate_limit: Option<RateLimit>,
    pub with_dynamic_vars: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimit {
    pub requests: usize,
    pub window_ms: u64,
}