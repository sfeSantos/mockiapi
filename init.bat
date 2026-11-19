@echo off
set RUST_LOG=info

rem The frontend goes first: the Rust build embeds frontend\dist in the binary.
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

echo 🚀 Building Rust backend...
cargo build --release || (
    echo ❌ Rust build failed!
    exit /b 1
)

echo 🚀 Running the application...
target\release\mockiapi %*
