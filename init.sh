#!/bin/bash

# Exit immediately if a command fails
set -e

# Enable logging
export RUST_LOG=info

# The frontend goes first: the Rust build embeds frontend/dist in the binary.
echo "📦 Installing frontend dependencies..."
cd frontend
npm install || { echo "❌ Frontend directory not found!"; exit 1; }

echo "🛠️ Building the frontend..."
npm run build || { echo "❌ Frontend build failed!"; exit 1; }
cd ..

echo "🚀 Building the Rust backend..."
cargo build --release || { echo "❌ Rust build failed!"; exit 1; }

echo "✅ Starting the server..."
./target/release/mockiapi "$@"
