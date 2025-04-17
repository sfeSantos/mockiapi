use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::models::grpc::GrpcMockResponse;

type MethodKey = String; // format: "MyService.MyMethod"

#[derive(Default)]
pub struct GrpcRegistry {
    mocks: RwLock<HashMap<MethodKey, GrpcMockResponse>>,
}

impl GrpcRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_mock(&self, service: &str, method: &str, response: GrpcMockResponse) {
        let key = format!("{}.{}", service, method);
        self.mocks.write().await.insert(key, response);
    }

    pub async fn get_mock(&self, service: &str, method: &str) -> Option<GrpcMockResponse> {
        let key = format!("{}.{}", service, method);
        self.mocks.read().await.get(&key).cloned()
    }
}