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
        let registry = self.mocks.read().await;

        let service_prefix = format!("{}.", service);
        let mut service_exists = false;

        for key in registry.keys() {
            if key.starts_with(&service_prefix) {
                service_exists = true;
                if key == &format!("{}.{}", service, method) {
                    return registry.get(key).cloned();
                }
            }
        }

        if !service_exists {
            log::warn!("gRPC service '{}' not found", service);
        } else {
            log::warn!("gRPC method '{}' not found in service '{}'", method, service);
        }

        None
    }
}