use serde::{Deserialize, Serialize};

/// Credentials an endpoint asks for. Basic auth uses the username and password, a bearer
/// token is compared with `token_data`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthData {
    pub username: Option<String>,
    pub password: Option<String>,
    pub token_data: Option<String>,
}
