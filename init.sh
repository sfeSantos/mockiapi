#!/bin/bash

# Exit immediately if a command fails
set -e

# Enable logging
export RUST_LOG=info

echo "🚀 Building the Rust backend..."
cargo build --release || { echo "❌ Rust build failed!"; exit 1; }

echo "📦 Installing frontend dependencies..."
cd frontend
npm install || { echo "❌ Frontend directory not found!"; exit 1; }

echo "🛠️ Building the frontend..."
npm run build || { echo "❌ Frontend build failed!"; exit 1; }
cd ..

echo "✅ Starting the server..."
./target/release/mockiapi
