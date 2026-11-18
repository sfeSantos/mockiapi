use serde_json::Value;
use mockiapi::handlers::graphql::process_graphql;

fn mock_json_data() -> Value {
    parse(r#"
        {
          "query": {
            "getUser": {
              "data": {
                "id": "123",
                "name": "John Doe",
                "email": "john@example.com",
                "age": 30,
                "address": {
                  "street": "123 Main St",
                  "city": "Anytown",
                  "state": "CA",
                  "zip": "12345"
                }
              }
            },
            "getUsers": {
              "data": [
                {
                  "id": "123",
                  "name": "John Doe",
                  "email": "john@example.com"
                },
                {
                  "id": "456",
                  "name": "Jane Smith",
                  "email": "jane@example.com"
                }
              ]
            }
          },
          "mutation": {
            "createUser": {
              "data": {
                "success": true,
                "user": {
                  "id": "789",
                  "name": "New User",
                  "email": "new@example.com"
                }
              }
            },
            "updateUser": {
              "data": {
                "success": true,
                "user": {
                  "id": "123",
                  "name": "Updated Name",
                  "email": "john@example.com"
                }
              }
            },
            "deleteUser": {
              "data": {
                "success": true,
                "id": "123"
              }
            }
          }
        }
    "#)
}

fn parse(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

#[test]
fn test_valid_query() {
    let request = r#"{ "query": "query getUser { id name email }" }"#;
    let json = process_graphql(request, &mock_json_data()).unwrap().unwrap();
    assert_eq!(json["data"]["id"], "123");
    assert_eq!(json["data"]["name"], "John Doe");
    assert_eq!(json["data"]["email"], "john@example.com");
}

#[test]
fn test_valid_mutation() {
    let request = r#"{ "query": "mutation createUser { success user { id name  email } }" }"#;
    let json = process_graphql(request, &mock_json_data()).unwrap().unwrap();
    assert_eq!(json["data"]["user"]["id"], "789");
    assert_eq!(json["data"]["user"]["name"], "New User");
    assert_eq!(json["data"]["user"]["email"], "new@example.com");
}

#[test]
fn test_nonexistent_query() {
    let request = r#"{ "query": "query nonExistent { id }" }"#;
    let result = process_graphql(request, &mock_json_data()).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_invalid_json_request() {
    let bad_json = r#"this is not json"#;
    let result = process_graphql(bad_json, &mock_json_data());
    assert!(result.is_err());
}

#[test]
fn test_query_that_does_not_parse() {
    let request = r#"{ "query": "query getUser { id " }"#;
    let result = process_graphql(request, &mock_json_data());
    assert!(result.is_err());
}

#[test]
fn test_partial_fields() {
    let request = r#"{ "query": "query getUser { name }" }"#;
    let json = process_graphql(request, &mock_json_data()).unwrap().unwrap();
    assert_eq!(json.as_object().unwrap().len(), 1);
    assert_eq!(json["data"]["name"], "John Doe");
}

#[test]
fn test_nested_fields() {
    let nested_data = r#"
        {
          "query": {
            "getUser": {
                "data": {
                  "id": "1",
                  "profile": {
                    "bio": "Hello",
                    "avatar": "url"
                }
                }
            }
          }
        }
        "#;
    let request = r#"{ "query": "query getUser { profile { bio } }" }"#;
    let json = process_graphql(request, &parse(nested_data)).unwrap().unwrap();
    assert_eq!(json, serde_json::json!({ "data": { "profile": { "bio": "Hello" } } }));
}

#[test]
fn test_nested_fields_inside_a_list() {
    let list_data = r#"
        {
          "query": {
            "listUsers": {
              "data": {
                "users": [
                  { "id": "1", "name": "Alice", "email": "alice@example.com" },
                  { "id": "2", "name": "Bob", "email": "bob@example.com" }
                ]
              }
            }
          }
        }
        "#;
    let request = r#"{ "query": "query listUsers { users { id name } }" }"#;
    let json = process_graphql(request, &parse(list_data)).unwrap().unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "data": { "users": [{ "id": "1", "name": "Alice" }, { "id": "2", "name": "Bob" }] } })
    );
}

#[test]
fn test_anonymous_query_is_rejected() {
    let request = r#"{ "query": "{ id name }" }"#;
    let result = process_graphql(request, &mock_json_data());
    assert!(result.is_err());
}
