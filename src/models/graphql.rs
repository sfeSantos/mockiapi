use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GraphQLRequest {
    pub query: String,
    pub operation_name: Option<String>,
}