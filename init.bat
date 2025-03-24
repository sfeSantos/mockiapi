@echo off
set RUST_LOG=info

echo 🚀 Building Rust backend...
cargo build --release || (
    echo ❌ Rust build failed!
    exit /b 1
)

echo 🚀 Building Svelte frontend...
cd frontend || (
    echo ❌ Frontend directory not found!
    exit /b 1
)
npm install && npm run build || (
    echo ❌ Frontend build failed!
    exit /b 1
)
cd ..

echo 🚀 Running the application...
target\release\mockiapi
