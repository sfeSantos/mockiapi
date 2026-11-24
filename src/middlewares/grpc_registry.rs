use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use bytes::Bytes;
use warp::http::StatusCode;
use crate::models::grpc::GrpcMockResponse;

type Methods = HashMap<String, Arc<GrpcMock>>;
type Services = HashMap<String, Methods>;

/// A registered gRPC answer, serialized once so serving it copies nothing.
#[derive(Debug)]
pub struct GrpcMock {
    pub body: Bytes,
    pub delay_ms: Option<u64>,
    pub status: StatusCode,
}

impl From<GrpcMockResponse> for GrpcMock {
    fn from(response: GrpcMockResponse) -> Self {
        // Serializing a `Value` cannot fail: its keys are strings and it holds no custom types.
        let body = serde_json::to_vec(&response.output).unwrap_or_default();
        let status = response.status
            .and_then(|code| StatusCode::from_u16(code).ok())
            .unwrap_or(StatusCode::OK);

        Self { body: Bytes::from(body), delay_ms: response.delay_ms, status }
    }
}

/// gRPC mocks, looked up by service and then by method.
///
/// Lookups borrow the request strings, so answering a call allocates no key. The lock is
/// never held across an await, which is why a blocking `RwLock` is enough.
#[derive(Debug, Default)]
pub struct GrpcRegistry {
    services: RwLock<Services>,
}

impl GrpcRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_mock(&self, service: &str, method: &str, response: GrpcMockResponse) {
        self.write()
            .entry(service.to_string())
            .or_default()
            .insert(method.to_string(), Arc::new(response.into()));
    }

    pub fn get_mock(&self, service: &str, method: &str) -> Option<Arc<GrpcMock>> {
        let services = self.read();
        let Some(methods) = services.get(service) else {
            log::warn!("gRPC service '{}' not found", service);
            return None;
        };
        let mock = methods.get(method).cloned();

        if mock.is_none() {
            log::warn!("gRPC method '{}' not found in service '{}'", method, service);
        }

        mock
    }

    /// Removes one mock. A service left with no method goes away with it.
    pub fn remove_mock(&self, service: &str, method: &str) {
        let mut services = self.write();
        let Some(methods) = services.get_mut(service) else {
            return;
        };

        methods.remove(method);

        if methods.is_empty() {
            services.remove(service);
        }
    }

    // A poisoned lock only means another request panicked. The map itself is still valid.
    fn read(&self) -> RwLockReadGuard<'_, Services> {
        self.services.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Services> {
        self.services.write().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registry_with(mocks: &[(&str, &str)]) -> GrpcRegistry {
        let registry = GrpcRegistry::new();

        for (service, method) in mocks {
            registry.register_mock(service, method, GrpcMockResponse { output: json!({}), delay_ms: None, status: None });
        }

        registry
    }

    #[test]
    fn removing_a_mock_leaves_the_others_of_its_service() {
        let registry = registry_with(&[("Books", "GetBook"), ("Books", "ListBooks"), ("Users", "GetUser")]);

        registry.remove_mock("Books", "GetBook");

        assert!(registry.get_mock("Books", "GetBook").is_none());
        assert!(registry.get_mock("Books", "ListBooks").is_some());
        assert!(registry.get_mock("Users", "GetUser").is_some());
    }

    #[test]
    fn service_goes_away_with_its_last_mock() {
        let registry = registry_with(&[("Books", "GetBook")]);

        registry.remove_mock("Books", "GetBook");

        assert!(registry.read().is_empty());
    }

    #[test]
    fn removing_a_mock_nobody_registered_does_nothing() {
        let registry = registry_with(&[("Books", "GetBook")]);

        registry.remove_mock("Books", "DeleteBook");
        registry.remove_mock("Orders", "GetOrder");

        assert!(registry.get_mock("Books", "GetBook").is_some());
    }
}
