use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use warp::reject::Reject;

pub type Endpoints = Arc<Mutex<HashMap<String, Endpoint>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub method: Vec<String>,
    pub file: String,
    pub status_code: Option<u16>,
    pub authentication: Option<String>,
    pub delay: Option<u64>,
    pub rate_limit: Option<RateLimit>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimit {
    pub requests: usize, // Allowed requests per window
    pub window_ms: u64,  // Time window in milliseconds
}

#[derive(Debug)]
pub struct RateLimited;
impl Reject for RateLimited {}

#[derive(Deserialize)]
pub struct AuthData {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) tokenData: Option<String>,
}