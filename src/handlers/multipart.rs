use bytes::{BufMut, Bytes, BytesMut};
use futures::{StreamExt, TryStreamExt};
use uuid::Uuid;
use warp::http::StatusCode;
use warp::multipart::{FormData, Part};
use crate::models::grpc::GrpcMockResponse;
use crate::models::{AuthData, Endpoint, EndpointKind, RateLimit, RegistrationError};

/// Directory the uploaded mock files are written to, relative to the working directory.
pub const UPLOADS_DIR: &str = "uploads";

const DEFAULT_STATUS_CODE: u16 = 200;

/// The fields of the dashboard form, exactly as they were sent.
#[derive(Debug, Default)]
pub struct RegistrationForm {
    pub path: Option<String>,
    pub methods: Option<String>,
    pub status_code: Option<String>,
    pub delay: Option<String>,
    pub rate_limit: Option<String>,
    pub authentication: Option<String>,
    pub with_dynamic_vars: Option<String>,
    pub is_graphql: Option<String>,
    pub grpc_service: Option<String>,
    pub grpc_rpc: Option<String>,
    pub file: Option<Bytes>,
}

/// A validated form, ready to be stored.
#[derive(Debug)]
pub struct Registration {
    pub path: String,
    pub endpoint: Endpoint,
    pub upload: Bytes,
    pub grpc: Option<GrpcRegistration>,
}

/// The gRPC mock a registration adds to the registry.
#[derive(Debug)]
pub struct GrpcRegistration {
    pub service: String,
    pub rpc: String,
    pub response: GrpcMockResponse,
}

impl RegistrationForm {
    /// Reads the multipart stream. Fields the server does not know are ignored.
    pub async fn read(form: FormData) -> Result<Self, RegistrationError> {
        let mut fields = Self::default();
        let mut parts = form.into_stream();

        while let Some(part) = parts.next().await {
            let part = part.map_err(|_| UNREADABLE_FORM)?;
            let name = part.name().to_string();

            if name == "file" {
                fields.file = Some(part_bytes(part).await?);
                continue;
            }

            let Some(field) = fields.text_field(&name) else {
                continue;
            };
            *field = Some(part_text(part).await?);
        }

        Ok(fields)
    }

    fn text_field(&mut self, name: &str) -> Option<&mut Option<String>> {
        match name {
            "path" => Some(&mut self.path),
            "methods" => Some(&mut self.methods),
            "status_code" => Some(&mut self.status_code),
            "delay" => Some(&mut self.delay),
            "rate_limit" => Some(&mut self.rate_limit),
            "authentication" => Some(&mut self.authentication),
            "with_dynamic_vars" => Some(&mut self.with_dynamic_vars),
            "isGraphQL" => Some(&mut self.is_graphql),
            "grpcService" => Some(&mut self.grpc_service),
            "grpcRPC" => Some(&mut self.grpc_rpc),
            _ => None,
        }
    }

    /// Checks every field and builds the endpoint. Nothing is written or stored here.
    pub fn validate(self) -> Result<Registration, RegistrationError> {
        let path = non_empty(self.path)
            .filter(|path| path.starts_with('/'))
            .ok_or(RegistrationError::Invalid("path is required and must start with /"))?;
        let upload = self.file
            .ok_or(RegistrationError::Invalid("a JSON response file is required"))?;
        let method = parse_methods(self.methods.as_deref().unwrap_or_default())?;
        let status_code = parse_status_code(self.status_code.as_deref().unwrap_or_default())?;
        let delay = self.delay.and_then(|delay| delay.trim().parse::<u64>().ok());
        let rate_limit = parse_rate_limit(self.rate_limit.as_deref().unwrap_or_default())?;
        let authentication = parse_authentication(self.authentication.as_deref().unwrap_or_default())?;
        let with_dynamic_vars = self.with_dynamic_vars.map(|flag| flag == "true");
        let grpc_target = parse_grpc_target(non_empty(self.grpc_service), non_empty(self.grpc_rpc))?;

        let kind = if grpc_target.is_some() {
            EndpointKind::Grpc
        } else if self.is_graphql.as_deref() == Some("true") {
            EndpointKind::GraphQl
        } else {
            EndpointKind::Rest
        };

        if kind == EndpointKind::GraphQl && serde_json::from_slice::<serde_json::Value>(&upload).is_err() {
            return Err(INVALID_JSON_FILE);
        }

        let grpc = match grpc_target {
            Some((service, rpc)) => {
                let output = serde_json::from_slice(&upload).map_err(|_| INVALID_JSON_FILE)?;
                let response = GrpcMockResponse { output, delay_ms: delay, status: Some(status_code) };

                Some(GrpcRegistration { service, rpc, response })
            }
            None => None,
        };

        let endpoint = Endpoint {
            kind,
            method,
            file: format!("{UPLOADS_DIR}/{}.json", Uuid::new_v4()),
            status_code: Some(status_code),
            authentication,
            delay,
            rate_limit,
            with_dynamic_vars,
        };

        Ok(Registration { path, endpoint, upload, grpc })
    }
}

const UNREADABLE_FORM: RegistrationError = RegistrationError::Invalid("the form could not be read");
const INVALID_JSON_FILE: RegistrationError =
    RegistrationError::Invalid("the response file must be valid JSON for GraphQL and gRPC endpoints");

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn parse_methods(value: &str) -> Result<Vec<String>, RegistrationError> {
    let methods: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|method| !method.is_empty())
        .map(str::to_ascii_uppercase)
        .collect();

    if methods.is_empty() {
        return Err(RegistrationError::Invalid("at least one HTTP method is required"));
    }

    Ok(methods)
}

/// A value that is not a number, which is what an empty form field sends, means the default.
fn parse_status_code(value: &str) -> Result<u16, RegistrationError> {
    let Ok(code) = value.trim().parse::<u16>() else {
        return Ok(DEFAULT_STATUS_CODE);
    };

    StatusCode::from_u16(code)
        .map(|status| status.as_u16())
        .map_err(|_| RegistrationError::Invalid("status_code must be between 100 and 999"))
}

/// Reads `<requests>/<window in ms>`. An empty value means no rate limit.
fn parse_rate_limit(value: &str) -> Result<Option<RateLimit>, RegistrationError> {
    const INVALID: RegistrationError =
        RegistrationError::Invalid("rate_limit must be <requests>/<window in ms>, for example 10/1000");

    if value.is_empty() {
        return Ok(None);
    }

    let (requests, window_ms) = value.split_once('/').ok_or(INVALID)?;
    let requests = requests.trim().parse::<usize>().map_err(|_| INVALID)?;
    let window_ms = window_ms.trim().parse::<u64>().map_err(|_| INVALID)?;

    Ok(Some(RateLimit { requests, window_ms }))
}

/// Reads the credentials as JSON. An empty value or `null` means the endpoint is open.
fn parse_authentication(value: &str) -> Result<Option<AuthData>, RegistrationError> {
    if value.is_empty() || value == "null" {
        return Ok(None);
    }

    serde_json::from_str(value)
        .map(Some)
        .map_err(|_| RegistrationError::Invalid(
            "authentication must be a JSON object with username and password, or with tokenData",
        ))
}

fn parse_grpc_target(
    service: Option<String>,
    rpc: Option<String>,
) -> Result<Option<(String, String)>, RegistrationError> {
    match (service, rpc) {
        (Some(service), Some(rpc)) => Ok(Some((service, rpc))),
        (Some(_), None) => Err(RegistrationError::Invalid("grpcRPC is required when grpcService is set")),
        // The form sends grpcRPC for every kind of endpoint, so alone it means nothing.
        (None, _) => Ok(None),
    }
}

async fn part_text(part: Part) -> Result<String, RegistrationError> {
    let bytes = part_bytes(part).await?;

    String::from_utf8(bytes.into()).map_err(|_| RegistrationError::Invalid("form fields must be UTF-8 text"))
}

async fn part_bytes(part: Part) -> Result<Bytes, RegistrationError> {
    part.stream()
        .try_fold(BytesMut::new(), |mut buffer, chunk| async move {
            buffer.put(chunk);

            Ok(buffer)
        })
        .await
        .map(BytesMut::freeze)
        .map_err(|_| UNREADABLE_FORM)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> RegistrationForm {
        RegistrationForm {
            path: Some("/api/hello".to_string()),
            methods: Some("GET".to_string()),
            file: Some(Bytes::from_static(b"{}")),
            ..RegistrationForm::default()
        }
    }

    fn reason(form: RegistrationForm) -> &'static str {
        form.validate().unwrap_err().reason()
    }

    #[test]
    fn minimal_form_registers_an_open_rest_endpoint() {
        let registration = form().validate().unwrap();

        assert_eq!(registration.path, "/api/hello");
        assert_eq!(registration.endpoint.kind, EndpointKind::Rest);
        assert_eq!(registration.endpoint.method, ["GET"]);
        assert_eq!(registration.endpoint.status_code, Some(200));
        assert!(registration.endpoint.authentication.is_none());
        assert!(registration.endpoint.rate_limit.is_none());
        assert!(registration.grpc.is_none());
    }

    #[test]
    fn values_the_form_sends_for_empty_fields_fall_back_to_defaults() {
        let registration = RegistrationForm {
            status_code: Some("undefined".to_string()),
            delay: Some("null".to_string()),
            rate_limit: Some(String::new()),
            authentication: Some("null".to_string()),
            grpc_service: Some(String::new()),
            grpc_rpc: Some(String::new()),
            ..form()
        }
        .validate()
        .unwrap();

        assert_eq!(registration.endpoint.kind, EndpointKind::Rest);
        assert_eq!(registration.endpoint.status_code, Some(200));
        assert_eq!(registration.endpoint.delay, None);
        assert!(registration.endpoint.authentication.is_none());
    }

    #[test]
    fn methods_are_trimmed_and_uppercased() {
        let registration = RegistrationForm { methods: Some("get, post,".to_string()), ..form() }
            .validate()
            .unwrap();

        assert_eq!(registration.endpoint.method, ["GET", "POST"]);
    }

    #[test]
    fn rate_limit_is_read_as_requests_per_window() {
        let registration = RegistrationForm { rate_limit: Some("10/1000".to_string()), ..form() }
            .validate()
            .unwrap();
        let rate_limit = registration.endpoint.rate_limit.unwrap();

        assert_eq!(rate_limit.requests, 10);
        assert_eq!(rate_limit.window_ms, 1000);
    }

    #[test]
    fn grpc_form_builds_the_mock_from_the_file() {
        let registration = RegistrationForm {
            path: Some("/grpc".to_string()),
            delay: Some("50".to_string()),
            status_code: Some("201".to_string()),
            grpc_service: Some("com.example.BookService".to_string()),
            grpc_rpc: Some("GetBook".to_string()),
            file: Some(Bytes::from_static(br#"{"id":"b123"}"#)),
            ..form()
        }
        .validate()
        .unwrap();
        let grpc = registration.grpc.unwrap();

        assert_eq!(registration.endpoint.kind, EndpointKind::Grpc);
        assert_eq!(grpc.service, "com.example.BookService");
        assert_eq!(grpc.rpc, "GetBook");
        assert_eq!(grpc.response.output["id"], "b123");
        assert_eq!(grpc.response.delay_ms, Some(50));
        assert_eq!(grpc.response.status, Some(201));
    }

    #[test]
    fn unusable_fields_are_refused_with_a_reason() {
        assert!(reason(RegistrationForm { path: None, ..form() }).contains("path"));
        assert!(reason(RegistrationForm { path: Some("api/hello".to_string()), ..form() }).contains("path"));
        assert!(reason(RegistrationForm { file: None, ..form() }).contains("file"));
        assert!(reason(RegistrationForm { methods: Some(",".to_string()), ..form() }).contains("method"));
        assert!(reason(RegistrationForm { status_code: Some("42".to_string()), ..form() }).contains("status_code"));
        assert!(reason(RegistrationForm { rate_limit: Some("10".to_string()), ..form() }).contains("rate_limit"));
        assert!(reason(RegistrationForm { authentication: Some("secret".to_string()), ..form() })
            .contains("authentication"));
        assert!(reason(RegistrationForm { grpc_service: Some("Books".to_string()), ..form() }).contains("grpcRPC"));
    }

    #[test]
    fn graphql_and_grpc_files_must_be_json() {
        let not_json = Some(Bytes::from_static(b"not json"));
        let graphql = RegistrationForm { is_graphql: Some("true".to_string()), file: not_json.clone(), ..form() };
        let grpc = RegistrationForm {
            grpc_service: Some("Books".to_string()),
            grpc_rpc: Some("GetBook".to_string()),
            file: not_json.clone(),
            ..form()
        };
        let rest = RegistrationForm { file: not_json, ..form() };

        assert!(reason(graphql).contains("valid JSON"));
        assert!(reason(grpc).contains("valid JSON"));
        assert!(rest.validate().is_ok());
    }
}
