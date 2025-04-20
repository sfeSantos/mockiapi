use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMock {
    pub path: String,
    pub behavior: WebSocketBehavior,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketBehavior {
    pub on_connect: Option<Vec<MockMessage>>,
    pub on_message: Option<Vec<OnMessageBehavior>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockMessage {
    pub text: String,
}

#[derive(Debug, Clone, Serialize,  Deserialize)]
pub struct OnMessageBehavior {
    pub match_regex: String,
    pub response: MockMessage,
}