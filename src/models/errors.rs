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
pub struct InvalidMultipart;
impl Reject for InvalidMultipart {}

#[derive(Debug)]
pub struct FileError;
impl Reject for FileError {}

#[derive(Debug)]
pub struct Utf8Error;
impl Reject for Utf8Error {}

#[derive(Debug)]
pub struct InvalidGraphQLRequest;
impl Reject for InvalidGraphQLRequest {}