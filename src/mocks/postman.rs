//! Postman collections (v2.0 and v2.1), read as a set of mocks.
//!
//! A request becomes an endpoint when it has a saved example: the method and the path
//! come from the request, the status code and the body from its first example. A
//! collection always holds requests nobody meant to mock, so one that cannot become an
//! endpoint is left out with a warning and does not stop the others.

use std::collections::HashSet;
use bytes::Bytes;
use serde::de::IgnoredAny;
use serde_json::Value;
use crate::mocks::{MockEntry, Parsed, ResponseSource};
use crate::models::EndpointKind;

/// Part of the schema address every v2 collection carries in `info.schema`.
const SCHEMA_MARK: &str = "/collection/v2";
const DEFAULT_METHOD: &str = "GET";
const DEFAULT_STATUS_CODE: u16 = 200;

pub fn is_collection(document: &Value) -> bool {
    document["info"]["schema"]
        .as_str()
        .is_some_and(|schema| schema.contains(SCHEMA_MARK))
}

pub fn entries(collection: &Value) -> Parsed {
    let mut reader = Reader::default();
    reader.read_items(&collection["item"], None);

    Parsed { entries: reader.entries, warnings: reader.warnings }
}

#[derive(Default)]
struct Reader {
    entries: Vec<MockEntry>,
    warnings: Vec<String>,
    /// Method and path of every endpoint so far, to leave out a second request for one.
    taken: HashSet<(String, String)>,
}

impl Reader {
    /// Walks requests and folders at any depth. The outermost folder names the group.
    fn read_items(&mut self, items: &Value, group: Option<&str>) {
        for item in items.as_array().into_iter().flatten() {
            let name = item["name"].as_str().unwrap_or("unnamed");

            if item["item"].is_array() {
                self.read_items(&item["item"], group.or(Some(name)));
            } else if !item["request"].is_null() {
                self.read_request(item, name, group);
            }
        }
    }

    fn read_request(&mut self, item: &Value, name: &str, group: Option<&str>) {
        let request = &item["request"];
        // A request can be written as nothing but its URL, which is then a GET.
        let url = if request.is_string() { request } else { &request["url"] };
        let method = request["method"]
            .as_str()
            .unwrap_or(DEFAULT_METHOD)
            .to_ascii_uppercase();
        let path = path_of(url);
        let label = format!("\"{name}\" ({method} {path})");
        let Some(example) = item["response"].as_array().and_then(|examples| examples.first()) else {
            self.warnings.push(format!("{label} was skipped: it has no saved example to answer with"));
            return;
        };
        let body = example["body"].as_str().unwrap_or_default();

        if !body.trim().is_empty() && serde_json::from_str::<IgnoredAny>(body).is_err() {
            self.warnings.push(format!("{label} was skipped: the body of its example is not JSON"));
            return;
        }

        if !self.taken.insert((method.clone(), path.clone())) {
            self.warnings.push(format!("{label} was skipped: an earlier request already answers {method} {path}"));
            return;
        }

        let status_code = example["code"]
            .as_u64()
            .and_then(|code| u16::try_from(code).ok())
            .unwrap_or(DEFAULT_STATUS_CODE);

        self.entries.push(MockEntry {
            path,
            methods: vec![method],
            group: group.map(str::to_string),
            kind: EndpointKind::Rest,
            status_code: Some(status_code),
            delay: None,
            rate_limit: None,
            authentication: None,
            with_dynamic_vars: false,
            grpc: None,
            response: ResponseSource::Inline(Bytes::copy_from_slice(body.as_bytes())),
        });
    }
}

/// The path an endpoint answers on, from a Postman URL.
///
/// The URL is either text or an object that already has the path split in segments. The
/// host, a `{{baseUrl}}` variable in its place and the query string are left out.
fn path_of(url: &Value) -> String {
    let raw_segments: Vec<&str> = match &url["path"] {
        Value::Array(segments) => segments
            .iter()
            .filter_map(Value::as_str)
            .collect(),
        Value::String(path) => path.split('/').collect(),
        _ => path_text(url.as_str().or(url["raw"].as_str()).unwrap_or_default())
            .split('/')
            .collect(),
    };
    let segments: Vec<String> = raw_segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .map(placeholder_or_literal)
        .collect();

    format!("/{}", segments.join("/"))
}

/// What is left of a URL written as text once the host and the query are taken away.
fn path_text(raw: &str) -> &str {
    let without_query = raw
        .split(['?', '#'])
        .next()
        .unwrap_or_default();

    if without_query.starts_with('/') {
        return without_query;
    }

    // `https://host/path`, `host/path` and `{{baseUrl}}/path` all start with something
    // that is not part of the path, up to the first slash after the scheme.
    let after_scheme = without_query
        .split_once("://")
        .map_or(without_query, |(_, rest)| rest);

    after_scheme
        .find('/')
        .map_or("", |start| &after_scheme[start..])
}

/// Postman writes a path variable as `:id`, and a variable as `{{id}}`. Both become `{id}`.
fn placeholder_or_literal(segment: &str) -> String {
    let variable = segment
        .strip_prefix(':')
        .or_else(|| segment.strip_prefix("{{").and_then(|rest| rest.strip_suffix("}}")));

    match variable {
        Some(name) if !name.is_empty() => format!("{{{name}}}"),
        _ => segment.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(name: &str, method: &str, url: Value, examples: Value) -> Value {
        json!({ "name": name, "request": { "method": method, "url": url }, "response": examples })
    }

    fn example(code: u16, body: &str) -> Value {
        json!([{ "name": "example", "code": code, "body": body }])
    }

    fn read(items: Value) -> Parsed {
        entries(&json!({
            "info": { "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
            "item": items,
        }))
    }

    #[test]
    fn collection_is_told_apart_by_its_schema() {
        let collection = json!({ "info": { "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" } });
        let manifest = json!({ "endpoints": [] });

        assert!(is_collection(&collection));
        assert!(!is_collection(&manifest));
    }

    #[test]
    fn request_with_an_example_becomes_an_endpoint() {
        let parsed = read(json!([request("Create user", "post", json!("{{baseUrl}}/users"), example(201, "{\"id\":1}"))]));
        let entry = &parsed.entries[0];

        assert!(parsed.warnings.is_empty());
        assert_eq!(entry.path, "/users");
        assert_eq!(entry.methods, ["POST"]);
        assert_eq!(entry.status_code, Some(201));
        assert_eq!(entry.response, ResponseSource::Inline(Bytes::from_static(b"{\"id\":1}")));
    }

    #[test]
    fn each_method_of_a_path_keeps_its_own_example() {
        let parsed = read(json!([
            request("List", "GET", json!("{{baseUrl}}/users"), example(200, "[]")),
            request("Create", "POST", json!("{{baseUrl}}/users"), example(201, "{\"id\":1}")),
        ]));

        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].status_code, Some(200));
        assert_eq!(parsed.entries[1].status_code, Some(201));
    }

    #[test]
    fn path_is_read_from_every_way_postman_writes_a_url() {
        let as_text = json!("https://api.example.com:8443/v1/users/:id?expand=true");
        let without_scheme = json!("localhost:3001/v1/users/:id");
        let as_object = json!({
            "raw": "{{baseUrl}}/v1/users/:id?expand=true",
            "host": ["{{baseUrl}}"],
            "path": ["v1", "users", ":id"],
            "query": [{ "key": "expand", "value": "true" }],
        });
        let only_raw = json!({ "raw": "{{baseUrl}}/v1/users/{{id}}" });

        assert_eq!(path_of(&as_text), "/v1/users/{id}");
        assert_eq!(path_of(&without_scheme), "/v1/users/{id}");
        assert_eq!(path_of(&as_object), "/v1/users/{id}");
        assert_eq!(path_of(&only_raw), "/v1/users/{id}");
        assert_eq!(path_of(&json!("https://api.example.com")), "/");
        assert_eq!(path_of(&json!("/already/a/path")), "/already/a/path");
    }

    #[test]
    fn requests_in_folders_are_read_and_grouped_by_the_outermost_folder() {
        let parsed = read(json!([
            { "name": "Users", "item": [
                { "name": "Admin", "item": [
                    request("Ban", "DELETE", json!("{{baseUrl}}/users/:id"), example(204, "")),
                ] },
            ] },
            request("Health", "GET", json!("{{baseUrl}}/health"), example(200, "{}")),
        ]));

        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].path, "/users/{id}");
        assert_eq!(parsed.entries[0].group.as_deref(), Some("Users"));
        assert_eq!(parsed.entries[0].status_code, Some(204));
        assert_eq!(parsed.entries[1].group, None);
    }

    #[test]
    fn request_that_cannot_be_an_endpoint_is_skipped_with_a_warning() {
        let parsed = read(json!([
            request("No example", "GET", json!("{{baseUrl}}/none"), json!([])),
            request("Page", "GET", json!("{{baseUrl}}/page"), example(200, "<html></html>")),
            request("First", "GET", json!("{{baseUrl}}/twice"), example(200, "{}")),
            request("Second", "GET", json!("{{baseUrl}}/twice"), example(200, "[]")),
        ]));

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].path, "/twice");
        assert_eq!(parsed.warnings.len(), 3);
        assert!(parsed.warnings[0].contains("\"No example\" (GET /none)"));
        assert!(parsed.warnings[0].contains("no saved example"));
        assert!(parsed.warnings[1].contains("not JSON"));
        assert!(parsed.warnings[2].contains("\"Second\""));
    }

    #[test]
    fn request_written_as_a_url_is_a_get() {
        let parsed = read(json!([{ "name": "Ping", "request": "{{baseUrl}}/ping", "response": example(200, "{}") }]));

        assert_eq!(parsed.entries[0].methods, ["GET"]);
        assert_eq!(parsed.entries[0].path, "/ping");
    }
}
