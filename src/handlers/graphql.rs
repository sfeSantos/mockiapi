use std::borrow::Cow;
use graphql_parser::parse_query;
use graphql_parser::query::{Definition, OperationDefinition, Selection, SelectionSet};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use warp::http::{Response, StatusCode};
use warp::hyper::Body;
use crate::models::MockError;
use crate::utils::json_response;

/// The body is not a named GraphQL query or mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidGraphQLRequest;

#[derive(Deserialize)]
struct GraphQLRequest<'a> {
    #[serde(borrow)]
    query: Cow<'a, str>,
}

/// Answers a GraphQL request over HTTP from the mock document of an endpoint.
pub fn respond(status: StatusCode, mock: &Value, body: &[u8]) -> Result<Response<Body>, MockError> {
    let body = std::str::from_utf8(body).map_err(|_| MockError::InvalidGraphQLRequest)?;
    let answer = process_graphql(body, mock)
        .map_err(|_| MockError::InvalidGraphQLRequest)?
        .ok_or(MockError::NotFound)?;
    // Serializing a `Value` cannot fail: its keys are strings and it holds no custom types.
    let answer = serde_json::to_vec(&answer).unwrap_or_default();

    Ok(json_response(status, answer))
}

/// Answers a GraphQL request from a mock document shaped as
/// `{ "query": { "<operation>": { "data": ... } }, "mutation": { ... } }`.
///
/// Returns `Ok(None)` when the document has no mock for the named operation, and an
/// error when the body is not a named query or mutation.
pub fn process_graphql(body: &str, mock: &Value) -> Result<Option<Value>, InvalidGraphQLRequest> {
    let request: GraphQLRequest = serde_json::from_str(body).map_err(|_| InvalidGraphQLRequest)?;
    let document = parse_query::<&str>(&request.query).map_err(|_| InvalidGraphQLRequest)?;
    let operation = document.definitions
        .iter()
        .find_map(|definition| match definition {
            Definition::Operation(operation) => Some(operation),
            Definition::Fragment(_) => None,
        })
        .ok_or(InvalidGraphQLRequest)?;
    let (section, name, selection_set) = match operation {
        OperationDefinition::Query(query) => ("query", query.name, &query.selection_set),
        OperationDefinition::Mutation(mutation) => ("mutation", mutation.name, &mutation.selection_set),
        OperationDefinition::Subscription(_) | OperationDefinition::SelectionSet(_) => {
            return Err(InvalidGraphQLRequest);
        }
    };
    let name = name.ok_or(InvalidGraphQLRequest)?;

    let Some(operation_mock) = mock.get(section).and_then(|operations| operations.get(name)) else {
        return Ok(None);
    };
    let data = match operation_mock.get("data") {
        Some(data @ Value::Object(_)) => select(data, selection_set),
        _ => Value::Object(Map::new()),
    };

    Ok(Some(json!({ "data": data })))
}

/// Keeps only the fields the selection asks for, following nested selections into
/// objects and into every element of a list.
fn select<'query>(value: &Value, selection_set: &SelectionSet<'query, &'query str>) -> Value {
    match value {
        Value::Object(fields) => Value::Object(selection_set.items
            .iter()
            .filter_map(|selection| match selection {
                Selection::Field(field) => fields
                    .get(field.name)
                    .map(|field_value| (field.name.to_string(), select_field(field_value, &field.selection_set))),
                Selection::FragmentSpread(_) | Selection::InlineFragment(_) => None,
            })
            .collect()),
        Value::Array(items) => Value::Array(items
            .iter()
            .map(|item| select(item, selection_set))
            .collect()),
        scalar => scalar.clone(),
    }
}

fn select_field<'query>(value: &Value, selection_set: &SelectionSet<'query, &'query str>) -> Value {
    if selection_set.items.is_empty() {
        value.clone()
    } else {
        select(value, selection_set)
    }
}
