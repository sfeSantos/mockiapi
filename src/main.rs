use std::collections::HashMap;
use std::sync::{Arc};
use log::info;
use tokio::sync::Mutex;
use warp::{Filter};
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::{Endpoints};
use mockiapi::middlewares::rate_limit::new_rate_limit;
use mockiapi::routes::{admin_routes, mock_routes};

#[tokio::main]
async fn main() {
    env_logger::init();
    let endpoints: Endpoints = Arc::new(Mutex::new(HashMap::new()));
    let rate_limiter = new_rate_limit();
    let registry = Arc::new(GrpcRegistry::new());
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
    
    let static_files = warp::fs::dir("frontend/dist")
        .with(warp::log("static_files"));

    let routes = admin_routes(endpoints.clone(), Arc::clone(&registry))
        .or(static_files)
        .or(mock_routes(endpoints, rate_limiter, registry))
        .with(warp::cors().allow_any_origin())
        .with(log);

    println!(".: Server running at http://localhost:3001");
    warp::serve(routes).run(([0, 0, 0, 0], 3001)).await;
}