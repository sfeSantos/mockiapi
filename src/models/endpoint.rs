use std::collections::HashMap;
use std::sync::{Arc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub type Endpoints = Arc<Mutex<HashMap<String, Endpoint>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
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