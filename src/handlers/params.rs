use std::collections::HashMap;
use bytes::Bytes;
use regex::Regex;
use serde_json::Value;
use url::Url;

/// A helper function to extract parameters (example: from query or path)
pub fn get_params_from_request(path: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Build full URL from incoming path
    let base_url = format!("http://localhost:3001{}", path);
    let url = match Url::parse(&base_url) {
        Ok(url) => url,
        Err(_) => return params, // Return empty if parsing fails
    };

    // Extract query parameters (e.g. ?key=value)
    for (key, value) in url.query_pairs() {
        params.insert(key.to_string(), value.to_string());
    }

    // Extract path parameters, ignoring 'api' and version segments (e.g., 'v1')
    let version_regex = Regex::new(r"^v\d+$").unwrap();
    let path_segments: Vec<&str> = url
        .path_segments()
        .unwrap()
        .filter(|segment| !segment.is_empty() && *segment != "api" && !version_regex.is_match(segment))
        .collect();

    // Assume key/value pairs in the remaining path segments
    let mut segment_iter = path_segments.iter();
    while let (Some(key), Some(value)) = (segment_iter.next(), segment_iter.next()) {
        params.insert((*key).to_string(), (*value).to_string());
    }

    params
}

///
pub fn get_body_from_request(body: Bytes) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Try to parse the body as JSON
    let Ok(json) = serde_json::from_slice::<Value>(&body) else {
        return params;
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

    params
}