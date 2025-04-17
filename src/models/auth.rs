use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthData {
    pub(crate) username: Option<String>,
    pub(crate) password: Option<String>,
    pub(crate) token_data: Option<String>,
}