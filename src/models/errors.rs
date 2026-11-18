use warp::http::{Response, StatusCode};
use warp::hyper::Body;

/// Why a mock endpoint did not answer with its registered response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockError {
    NotFound,
    MethodNotAllowed,
    Unauthorized,
    RateLimited,
    InvalidGraphQLRequest,
    InvalidGrpcRequest,
}

impl MockError {
    pub fn status(self) -> StatusCode {
        match self {
            MockError::NotFound => StatusCode::NOT_FOUND,
            MockError::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            MockError::Unauthorized => StatusCode::UNAUTHORIZED,
            MockError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            MockError::InvalidGraphQLRequest | MockError::InvalidGrpcRequest => StatusCode::BAD_REQUEST,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            MockError::NotFound => "Resource not found\n",
            MockError::MethodNotAllowed => "Method not allowed\n",
            MockError::Unauthorized => "Unauthorized\n",
            MockError::RateLimited => "Rate limit exceeded\n",
            MockError::InvalidGraphQLRequest => "Invalid GraphQL request\n",
            MockError::InvalidGrpcRequest => "Invalid gRPC request\n",
        }
    }

    pub fn into_response(self) -> Response<Body> {
        let mut response = Response::new(Body::from(self.message()));
        *response.status_mut() = self.status();

        response
    }
}

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
