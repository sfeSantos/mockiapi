use std::net::SocketAddr;
use std::path::Path;
use std::process::exit;
use std::sync::Arc;
use log::info;
use warp::Filter;
use mockiapi::config::{self, Command, Environment, USAGE};
use mockiapi::handlers::multipart::UPLOADS_DIR;
use mockiapi::middlewares::grpc_registry::GrpcRegistry;
use mockiapi::mocks;
use mockiapi::persistence::{self, ENDPOINTS_FILE};
use mockiapi::models::Endpoints;
use mockiapi::routes::{admin_routes, dashboard_routes, mock_routes};

#[tokio::main]
async fn main() {
    let (port, mocks_file, persist) = match config::parse(std::env::args().skip(1), Environment::read()) {
        Ok(Command::Serve { port, mocks, persist }) => (port, mocks, persist),
        Ok(Command::Help) => {
            println!("{USAGE}");
            return;
        }
        Err(error) => {
            eprintln!("error: {error}\n\n{USAGE}");
            exit(2);
        }
    };

    env_logger::init();

    // The directory is not part of the release binary or the Docker image.
    tokio::fs::create_dir_all(UPLOADS_DIR)
        .await
        .expect("the uploads directory could not be created");

    let endpoints = if persist { Endpoints::saved_in(ENDPOINTS_FILE) } else { Endpoints::default() };
    let registry = Arc::new(GrpcRegistry::new());

    // Before the port is opened, so the first request already finds the endpoints.
    restore_endpoints(&endpoints, &registry).await;

    // After the saved endpoints, so the file wins where both answer a method of a path.
    if let Some(file) = mocks_file {
        load_mocks(&file, &endpoints, &registry).await;
        endpoints.save().await;
    }

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

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    // Binding first means the line below is only printed once the port is really taken.
    let server = match warp::serve(routes).try_bind_ephemeral(address) {
        Ok((_, server)) => server,
        Err(error) => {
            eprintln!("error: could not listen on port {port}: {error}");
            eprintln!("Is another server using it? Choose another port with --port <PORT>.");
            exit(1);
        }
    };

    println!(".: Server running at http://localhost:{port}");
    server.await;
}

/// Registers the endpoints an earlier run saved, or stops the process when their file cannot
/// be read: starting empty would write over it at the next change.
async fn restore_endpoints(endpoints: &Endpoints, registry: &GrpcRegistry) {
    match persistence::restore(endpoints, registry).await {
        Ok(restored) => {
            for skipped in &restored.skipped {
                eprintln!("warning: skipped a saved endpoint: {skipped}");
            }

            if restored.loaded > 0 {
                println!(".: Restored {} endpoints from {ENDPOINTS_FILE}", restored.loaded);
            }
        }
        Err(error) => {
            eprintln!("error: could not restore the endpoints saved in {ENDPOINTS_FILE}: {error}");
            eprintln!("Fix or remove the file, or start without it with --no-persist.");
            exit(1);
        }
    }
}

/// Registers the endpoints of the mocks file, or stops the process when it cannot be loaded:
/// a server that starts without the mocks it was asked for would fail later and less clearly.
async fn load_mocks(file: &Path, endpoints: &Endpoints, registry: &GrpcRegistry) {
    match mocks::load_file(file, endpoints, registry).await {
        Ok(loaded) => {
            // Printed, not logged: without RUST_LOG the logger only shows errors.
            for warning in &loaded.warnings {
                eprintln!("warning: {warning}");
            }

            println!(".: Loaded {} endpoints from {}", loaded.registered, file.display());
        }
        Err(error) => {
            eprintln!("error: could not load {}: {error}", file.display());
            exit(1);
        }
    }
}
