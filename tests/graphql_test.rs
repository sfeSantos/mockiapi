use mockiapi::handlers::graphql::process_graphql;

fn mock_json_data() -> &'static str {
    r#"
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
    "#
}

#[test]
fn test_valid_query() {
    let request = r#"{ "query": "query getUser { id name email }" }"#;
    let result = process_graphql(request, mock_json_data()).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json["data"]["id"], "123");
    assert_eq!(json["data"]["name"], "John Doe");
    assert_eq!(json["data"]["email"], "john@example.com");
}

#[test]
fn test_valid_mutation() {
    let request = r#"{ "query": "mutation createUser { success user { id name  email } }" }"#;
    let result = process_graphql(request, mock_json_data()).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json["data"]["user"]["id"], "789");
    assert_eq!(json["data"]["user"]["name"], "New User");
    assert_eq!(json["data"]["user"]["email"], "new@example.com");
}

#[test]
fn test_nonexistent_query() {
    let request = r#"{ "query": "query nonExistent { id }" }"#;
    let result = process_graphql(request, mock_json_data()).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_invalid_json_request() {
    let bad_json = r#"this is not json"#;
    let result = process_graphql(bad_json, mock_json_data());
    assert!(result.is_err());
}

#[test]
fn test_partial_fields() {
    let request = r#"{ "query": "query getUser { name }" }"#;
    let result = process_graphql(request, mock_json_data()).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
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
    let result = process_graphql(request, nested_data).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 1);
    assert_eq!(json["data"]["profile"]["bio"], "Hello");
}