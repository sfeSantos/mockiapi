use std::sync::Arc;
use log::info;
use warp::Filter;
use mockiapi::handlers::multipart::UPLOADS_DIR;
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::models::Endpoints;
use mockiapi::routes::{admin_routes, dashboard_routes, mock_routes};

#[tokio::main]
async fn main() {
    env_logger::init();

    // The directory is not part of the release binary or the Docker image.
    tokio::fs::create_dir_all(UPLOADS_DIR)
        .await
        .expect("the uploads directory could not be created");

    let endpoints = Endpoints::default();
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
    let static_files = dashboard_routes()
        .with(warp::log("static_files"));

    let routes = admin_routes(endpoints.clone(), Arc::clone(&registry))
        .or(static_files)
        .or(mock_routes(endpoints, registry))
        .with(warp::cors().allow_any_origin())
        .with(log);

    println!(".: Server running at http://localhost:3001");
    warp::serve(routes).run(([0, 0, 0, 0], 3001)).await;
}
