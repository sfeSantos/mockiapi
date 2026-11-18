use std::collections::HashMap;
use bytes::Bytes;
use serde_json::Value;
use warp::http::Response;
use warp::hyper::Body;
use crate::middlewares::dynamic_vars::replace_variables;
use crate::routing::Route;
use crate::utils::json_response;

/// Answers with the uploaded content, filling in `{{name}}` placeholders when the
/// endpoint asked for dynamic variables.
///
/// Without them the stored bytes are shared with the response, not copied.
pub fn respond(route: &Route, content: &Bytes, path: &str, query: &str, body: &[u8]) -> Response<Body> {
    let endpoint = &route.endpoint;

    if !endpoint.with_dynamic_vars.unwrap_or(false) {
        return json_response(endpoint.status(), content.clone());
    }

    // A template that is not text has no placeholder to fill.
    let Ok(template) = std::str::from_utf8(content) else {
        return json_response(endpoint.status(), content.clone());
    };
    let variables = request_variables(route, path, query, body);

    json_response(endpoint.status(), replace_variables(template, &variables))
}

/// Gathers the values a template can refer to: path placeholders, query parameters and
/// the fields of a JSON body. A later source overrides an earlier one.
fn request_variables(route: &Route, path: &str, query: &str, body: &[u8]) -> HashMap<String, String> {
    let mut variables = HashMap::new();
    route.pattern.capture(path, query, &mut variables);
    insert_body_fields(body, &mut variables);

    variables
}

/// Reads the top-level fields of a JSON object body. Any other body adds nothing.
fn insert_body_fields(body: &[u8], variables: &mut HashMap<String, String>) {
    let Ok(Value::Object(fields)) = serde_json::from_slice::<Value>(body) else {
        return;
    };

    for (name, value) in fields {
        let text = match value {
            Value::String(text) => text,
            other => other.to_string(),
        };
        variables.insert(name, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_fields(body: &[u8]) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        insert_body_fields(body, &mut variables);

        variables
    }

    #[test]
    fn reads_the_fields_of_a_json_object() {
        let variables = body_fields(br#"{"name":"John","age":30,"admin":true,"tags":["a"]}"#);

        assert_eq!(variables["name"], "John");
        assert_eq!(variables["age"], "30");
        assert_eq!(variables["admin"], "true");
        assert_eq!(variables["tags"], r#"["a"]"#);
    }

    #[test]
    fn ignores_a_body_that_is_not_a_json_object() {
        assert!(body_fields(b"").is_empty());
        assert!(body_fields(b"plain text").is_empty());
        assert!(body_fields(b"[1,2,3]").is_empty());
    }
}
