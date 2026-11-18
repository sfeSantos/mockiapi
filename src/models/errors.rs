use warp::http::StatusCode;
use warp::reject::Reject;

#[derive(Debug)]
pub struct RateLimited;
impl Reject for RateLimited {}

#[derive(Debug)]
pub struct NotFound;
impl Reject for NotFound {}

#[derive(Debug)]
pub struct Unauthorized;
impl Reject for Unauthorized {}

#[derive(Debug)]
pub struct InvalidGraphQLRequest;
impl Reject for InvalidGraphQLRequest {}

#[derive(Debug)]
pub struct InvalidGrpcRequest;
impl Reject for InvalidGrpcRequest {}

#[derive(Debug)]
pub struct MethodNotAllowed;
impl Reject for MethodNotAllowed {}

/// Why an endpoint could not be registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    /// A form field is missing or malformed. The text is shown to the client.
    Invalid(&'static str),
    /// The uploaded file could not be written to disk.
    Storage,
}

impl RegistrationError {
    pub fn status(self) -> StatusCode {
        match self {
            RegistrationError::Invalid(_) => StatusCode::BAD_REQUEST,
            RegistrationError::Storage => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            RegistrationError::Invalid(reason) => reason,
            RegistrationError::Storage => "the response file could not be stored",
        }
    }
}
