use base64::{Engine as _, engine::general_purpose};
use crate::models::AuthData;

/// Checks an `Authorization` header against the credentials of an endpoint.
pub fn validate_auth(expected: &AuthData, auth_header: Option<&str>) -> bool {
    let Some(header) = auth_header else {
        return false;
    };

    if let Some(encoded_credentials) = header.strip_prefix("Basic ") {
        return validate_basic_auth(expected, encoded_credentials);
    }

    if let Some(token) = header.strip_prefix("Bearer ") {
        return expected.token_data.as_deref() == Some(token);
    }

    false
}

fn validate_basic_auth(expected: &AuthData, encoded_credentials: &str) -> bool {
    let Ok(decoded) = general_purpose::STANDARD.decode(encoded_credentials) else {
        return false;
    };
    let Ok(credentials) = std::str::from_utf8(&decoded) else {
        return false;
    };
    // Only the first colon separates the two: a password may contain more of them.
    let Some((username, password)) = credentials.split_once(':') else {
        return false;
    };

    expected.username.as_deref() == Some(username) && expected.password.as_deref() == Some(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic(username: &str, password: &str) -> AuthData {
        AuthData {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
            token_data: None,
        }
    }

    fn header(credentials: &str) -> String {
        format!("Basic {}", general_purpose::STANDARD.encode(credentials))
    }

    #[test]
    fn basic_credentials_must_match() {
        let expected = basic("user", "pass");

        assert!(validate_auth(&expected, Some(&header("user:pass"))));
        assert!(!validate_auth(&expected, Some(&header("user:other"))));
        assert!(!validate_auth(&expected, Some(&header("userpass"))));
        assert!(!validate_auth(&expected, Some("Basic not-base64!")));
        assert!(!validate_auth(&expected, None));
    }

    #[test]
    fn password_may_contain_a_colon() {
        let expected = basic("user", "pa:ss");

        assert!(validate_auth(&expected, Some(&header("user:pa:ss"))));
    }

    #[test]
    fn bearer_token_must_match() {
        let expected = AuthData { token_data: Some("SECRET".to_string()), ..AuthData::default() };

        assert!(validate_auth(&expected, Some("Bearer SECRET")));
        assert!(!validate_auth(&expected, Some("Bearer OTHER")));
        assert!(!validate_auth(&expected, Some("SECRET")));
    }

    #[test]
    fn credentials_of_the_other_scheme_are_not_accepted() {
        assert!(!validate_auth(&basic("user", "pass"), Some("Bearer pass")));
    }
}
