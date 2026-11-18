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

    pub fn clear(&self) {
        self.write().clear();
    }

    // A poisoned lock only means another request panicked. The map itself is still valid.
    fn read(&self) -> RwLockReadGuard<'_, Services> {
        self.services.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Services> {
        self.services.write().unwrap_or_else(PoisonError::into_inner)
    }
}
