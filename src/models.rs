use std::collections::HashMap;
use std::sync::{Arc};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use warp::reject::Reject;

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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimit {
    pub requests: usize, // Allowed requests per window
    pub window_ms: u64,  // Time window in milliseconds
}


#[derive(Debug)]
pub struct RateLimited;
impl Reject for RateLimited {}

#[derive(Debug)]
pub struct NotFound;
impl Reject for NotFound {}


#[derive(Debug)]
pub struct Unauthorized;
impl Reject for Unauthorized {}

#[derive(Debug)]
pub struct InvalidMultipart;
impl Reject for InvalidMultipart {}

#[derive(Debug)]
pub struct FileError;
impl Reject for FileError {}

#[derive(Debug)]
pub struct Utf8Error;
impl Reject for Utf8Error {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthData {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) token_data: Option<String>,
}

pub struct MultipartHandler;
