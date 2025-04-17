use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrpcMockRequest {
    pub service: String,
    #[serde(rename = "rpc")]
    pub method: String,
    #[serde(rename = "request")]
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrpcMockResponse {
    pub output: serde_json::Value,
    pub delay_ms: Option<u64>,
    pub status: Option<u16>,
}