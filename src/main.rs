use std::collections::HashMap;
use std::sync::{Arc};
use log::info;
use tokio::sync::Mutex;
use warp::{Filter};
use warp::http::header::AUTHORIZATION;
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::{Endpoints};
use mockiapi::middlewares::rate_limit::new_rate_limit;
use mockiapi::routes::endpoints::{delete_endpoint, list_endpoint, register_endpoint, with_endpoints};
use mockiapi::routes::dynamic_response::serve_dynamic_response;
use mockiapi::utils::{handle_rejection, with_rate_limiter};

#[tokio::main]
async fn main() {
    env_logger::init();
    let endpoints: Endpoints = Arc::new(Mutex::new(HashMap::new()));
    let rate_limiter = new_rate_limit();
    let registry = Arc::new(GrpcRegistry::new());
    let registry_filter = warp::any().map({
        let registry = Arc::clone(&registry);
        move || Arc::clone(&registry)
    });

    let log = warp::log::custom(|info| {
        info!("{} - {} {} {} [{}] {:?}",
            info.remote_addr()
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            info.method(),
            info.path(),
            info.status(),
            info.elapsed().as_millis(),
            info.request_headers()
        );
    });
    
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

    let dynamic_routes = warp::method()
        .and(warp::path::full())
        .and(warp::query::<HashMap<String, String>>()
            .map(Some)
            .or(warp::any().map(|| None))
            .unify())
        .and(warp::header::optional::<String>(AUTHORIZATION.as_str()))
        .and(with_endpoints(endpoints.clone()))
        .and(with_rate_limiter(rate_limiter.clone()))
        .and(warp::body::bytes()
            .map(Some)
            .or(warp::any().map(|| None))
            .unify())
        .and(registry_filter.clone())
        .and_then(serve_dynamic_response)
        .recover(handle_rejection);
    
    let static_files = warp::fs::dir("frontend/dist")
        .with(warp::log("static_files"));
    
    let routes = register
        .or(list)
        .or(delete)
        .or(static_files)
        .or(dynamic_routes)
        .with(warp::cors().allow_any_origin())
        .with(log);
    
    println!(".: Server running at http://localhost:3001");
    warp::serve(routes).run(([0, 0, 0, 0], 3001)).await;
}