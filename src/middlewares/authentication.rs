use base64::{Engine as _, engine::{general_purpose}};
use crate::models::AuthData;

/// Function to validate the authorization based on the `auth_type` and `auth_data`
pub fn validate_auth(auth_data: Option<String>, auth_header: Option<String>) -> bool {
    if let Some(header) = auth_header {
        if header.starts_with("Basic ") {
            return validate_basic_auth(auth_data, header);
        }

        if header.starts_with("Bearer ") {
            return validate_bearer_token(auth_data, header);
        }
    }

    false
}

/// Function to validate Basic Authentication
fn validate_basic_auth(auth_data: Option<String>, header: String) -> bool {
    if let Some(auth_data) = auth_data {
        let encoded_credentials = header.trim_start_matches("Basic ").to_string();
        if let Ok(decoded) = general_purpose::STANDARD.decode(&encoded_credentials) {
            if let Ok(creds) = String::from_utf8(decoded) {
                let mut parts = creds.split(':');
                if let (Some(username), Some(password)) = (parts.next(), parts.next()) {
                    if let Ok(auth_struct) = serde_json::from_str::<AuthData>(&auth_data) {
                        // Now check if the fields are Some and match
                        let username_matches = auth_struct.username
                            .map_or(false, |u| u == username);
                        let password_matches = auth_struct.password
                            .map_or(false, |p| p == password);

                        return username_matches && password_matches;
                    }
                }
            }
        }
    }
    false
}

/// Function to validate Bearer Token Authentication
fn validate_bearer_token(auth_data: Option<String>, header: String) -> bool {
    if let Some(auth_data) = auth_data {
        if let Ok(auth_struct) = serde_json::from_str::<AuthData>(&auth_data) {
            if let Some(expected_token) = auth_struct.token_data {
                let token = header.trim_start_matches("Bearer ").to_string();
                return token == expected_token;
            }
        }
    }

    false
}
