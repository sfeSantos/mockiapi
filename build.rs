use std::path::Path;
use std::process::exit;

const DASHBOARD_DIR: &str = "frontend/dist";
const DASHBOARD_ENTRY: &str = "frontend/dist/index.html";

/// Stops the build early when the dashboard bundle has not been built.
///
/// The bundle is embedded in the binary, so compiling without it would fail anyway, with
/// an error that does not say what to do.
fn main() {
    // A rebuilt bundle has to end up in the next release binary.
    println!("cargo:rerun-if-changed={DASHBOARD_DIR}");

    if !Path::new(DASHBOARD_ENTRY).exists() {
        eprintln!("error: {DASHBOARD_ENTRY} is missing.");
        eprintln!("The dashboard is embedded in the binary and has to be built first:");
        eprintln!();
        eprintln!("    cd frontend && npm install && npm run build");
        eprintln!();
        exit(1);
    }
}
