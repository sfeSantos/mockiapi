use std::collections::HashSet;
use graphql_parser::parse_query;
use graphql_parser::query::{Definition, OperationDefinition, Selection, SelectionSet};
use serde_json::Value;
use warp::http::{HeaderValue, Response, StatusCode};
use warp::http::header::CONTENT_TYPE;
use warp::Rejection;
use crate::models::{Endpoint, GraphQLRequest, InvalidGraphQLRequest};

pub fn handle_graphql(
    body_str: &str,
    endpoint: &Endpoint,
    json_file_content: &str,
) -> Option<Response<String>> {
    if body_str.contains("\"query\"") {
        if let Ok(Some(gql_data)) = process_graphql(body_str, json_file_content) {
            let status_code = StatusCode::from_u16(endpoint.status_code.unwrap_or(200))
                .unwrap_or(StatusCode::OK);
            let response = Response::builder()
                .status(status_code)
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(gql_data.into())
                .ok()?;
            return Some(response);
        }
    }
    None
}

/// Extract the operation name and load the mocked data
pub fn process_graphql(body: &str, json_data: &str) -> Result<Option<String>, Rejection> {
    // Parse the GraphQL request body
    let gql_req: GraphQLRequest = serde_json::from_str(body)
        .map_err(|_| warp::reject::custom(InvalidGraphQLRequest))?;

    // Parse the JSON mock
    let json: Value = serde_json::from_str(json_data)
        .map_err(|_| warp::reject::custom(InvalidGraphQLRequest))?;

    // Extract operation name from the query
    let operation_name = extract_operation_name(&gql_req.query)
        .ok_or_else(|| warp::reject::custom(InvalidGraphQLRequest))?;

    let requested_fields = extract_requested_fields(&gql_req.query);

    // Check if it's a query or mutation
    let is_query = gql_req.query.trim_start().starts_with("query");
    let is_mutation = gql_req.query.trim_start().starts_with("mutation");

    // Try to get the relevant mock data
    let maybe_target = if is_query {
        json.get("query").and_then(|q| q.get(&operation_name))
    } else if is_mutation {
        json.get("mutation").and_then(|m| m.get(&operation_name))
    } else {
        None
    };

    let Some(target_data) = maybe_target else {
        return Ok(None);
    };

    // Filter by requested fields
    let mut filtered_data = serde_json::Map::new();
    if let Some(obj) = target_data.get("data")
        .and_then(|v| v.as_object()) {
        for field in &requested_fields {
            if let Some(value) = obj.get(field) {
                filtered_data.insert(field.clone(), value.clone());
            }
        }
    }

    let final_json = serde_json::json!({
        "data": filtered_data
    });

    let result = serde_json::to_string(&final_json)
        .map_err(|_| warp::reject::custom(InvalidGraphQLRequest))?;

    Ok(Some(result))
}

fn extract_operation_name(query: &str) -> Option<String> {
    let ast = parse_query::<&str>(query).ok();
    if let Some(ast) = ast {
        for definition in ast.definitions {
            if let Definition::Operation(OperationDefinition::Query(q)) = definition {
                return q.name.map(|s| s.to_string());
            } else if let Definition::Operation(OperationDefinition::Mutation(m)) = definition {
                return m.name.map(|s| s.to_string());
            }
        }
    }

    // Fallback: Try simple regex
    let tokens: Vec<&str> = query.trim().split_whitespace().collect();
    if tokens.len() >= 2 && (tokens[0] == "query" || tokens[0] == "mutation") {
        return Some(tokens[1].to_string());
    }

    None
}

fn extract_requested_fields(query: &str) -> HashSet<String> {
    let ast = parse_query::<&str>(query).expect("Failed to parse query");
    let mut fields = HashSet::new();

    for definition in ast.definitions {
        if let Definition::Operation(operation) = definition {
            let owned_selection_set = match operation {
                OperationDefinition::Query(q) => q.selection_set,
                OperationDefinition::Mutation(m) => m.selection_set,
                OperationDefinition::Subscription(s) => s.selection_set,
                OperationDefinition::SelectionSet(s) => s,
            };

            if owned_selection_set.items.is_empty() {
                return HashSet::new();
            }

            collect_field_names_owned(&owned_selection_set, &mut fields);
        }
    }

    fields
}

fn collect_field_names_owned<'a>(selection_set: &SelectionSet<'a, &'a str>, fields: &mut HashSet<String>) {
    for selection in &selection_set.items {
        if let Selection::Field(field) = selection {
            fields.insert(field.name.to_string());
            if !field.selection_set.items.is_empty() {
                collect_field_names_owned(&field.selection_set, fields);
            }
        }
    }
}