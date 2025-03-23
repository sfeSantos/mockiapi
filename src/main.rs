use std::collections::HashMap;
use std::sync::{Arc};
use tokio::sync::Mutex;
use warp::{Filter};
use warp::http::header::AUTHORIZATION;
use mockiapi::endpoint_handler::{delete_endpoint, list_endpoint, register_endpoint, serve_dynamic_response, with_endpoints};
use mockiapi::models::{Endpoints};
use mockiapi::rate_limit::new_rate_limit;
use mockiapi::utils::{handle_rejection, with_rate_limiter};

#[tokio::main]
async fn main() {
    let endpoints: Endpoints = Arc::new(Mutex::new(HashMap::new()));
    let rate_limiter = new_rate_limit();
    
    let register = warp::post()
        .and(warp::path!("register"))
        .and(warp::multipart::form().max_length(5_000_000)) // 5MB
        .and(with_endpoints(endpoints.clone()))
        .and_then(register_endpoint);

    let list = warp::get()
        .and(warp::path!("list"))
        .and(with_endpoints(endpoints.clone()))
        .and_then(list_endpoint);

    let delete = warp::delete()
        .and(warp::path!("delete" / String))
        .and(with_endpoints(endpoints.clone()))
        .and_then(delete_endpoint);

    let dynamic_routes = warp::path::full()
        .and(warp::header::optional::<String>(AUTHORIZATION.as_str()))
        .and(with_endpoints(endpoints.clone()))
        .and(with_rate_limiter(rate_limiter.clone()))
        .and_then(serve_dynamic_response)
        .recover(handle_rejection);
    
    let static_files = warp::fs::dir("frontend/dist");
    
    let routes = register
        .or(list)
        .or(delete)
        .or(dynamic_routes)
        .or(static_files)
        .with(warp::cors().allow_any_origin());
    
    println!(".: Server running at http://localhost:3001");
    warp::serve(routes).run(([127, 0, 0, 1], 3001)).await;
}