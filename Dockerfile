# Build stage for frontend
FROM node:18-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/ ./
RUN npm install
RUN npm run build

# Build stage for Rust backend
FROM rust:latest AS backend-builder
WORKDIR /app
COPY . .
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist
RUN cargo build --release
# Find the binary name from the Cargo.toml file
RUN BINARY_NAME=$(grep -m 1 "name" Cargo.toml | sed 's/.*"\(.*\)".*/\1/') && \
    echo "Binary name is $BINARY_NAME" && \
    cp /app/target/release/$BINARY_NAME /app/mockiapi

# Final stage
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the build stage
COPY --from=backend-builder /app/mockiapi /app/mockiapi
# Copy frontend static files
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Expose the port the app runs on
EXPOSE 3001

# Set the environment variable for the port
ENV PORT=3001

# Command to run the application
CMD ["/app/mockiapi"]