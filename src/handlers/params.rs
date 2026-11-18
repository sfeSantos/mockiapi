use std::collections::HashMap;
use bytes::Bytes;
use serde_json::Value;

/// Reads the top-level fields of a JSON object body as dynamic variables.
pub fn get_body_from_request(body: &Bytes, params: &mut HashMap<String, String>) {
    // Try to parse the body as JSON
    let Ok(json) = serde_json::from_slice::<Value>(body) else {
        return;
    };

    // Extract key-value pairs from a JSON object
    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            let value_str = value.as_str()
                .map(String::from)
                .unwrap_or_else(|| value.to_string());

            params.insert(key.to_owned(), value_str);
        }
    }
}