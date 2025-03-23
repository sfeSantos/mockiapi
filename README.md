# Dynamic Mock API 🚀

A flexible, feature-rich mock API server with an intuitive frontend that simplifies development and testing across any programming language.

## Features

### 🔌 Easy Endpoint Registration
Register custom API endpoints through a user-friendly web interface. Simply provide a path, JSON response file, and configuration options.

### 📄 JSON Response Mocking
Upload or paste JSON files that will be returned when your endpoint is called, making it simple to simulate various API responses.

### 🔒 Authentication Support
Configure endpoints to require authentication, helping you test secure API interactions without setting up complex infrastructure.

### ⏱️ Rate Limiting
Apply rate limits to endpoints to simulate real-world API constraints and test how your application handles them.

### ⏳ Configurable Response Delays
Add realistic network delays to endpoint responses to test how your application handles latency.

### 🔄 Dynamic HTTP Status Codes
Set custom HTTP status codes for each endpoint to simulate various response scenarios, from success to server errors.

### 📊 Request Logging
Automatically log all requests made to your mock endpoints for debugging and analysis.

### 🌐 Cross-Platform Compatibility
Works with applications built in any programming language - if it can make HTTP requests, it can work with Dynamic Mock API.

## Why Choose Dynamic Mock API?

When developing applications that rely on external APIs, you need a way to test your integration without relying on actual endpoints. Dynamic Mock API solves this challenge by providing a flexible, feature-rich solution that outshines existing alternatives:

### Advantages over Wiremock:

- **User-Friendly Interface**: Unlike Wiremock's JSON configuration files, our intuitive web interface makes endpoint configuration accessible to all team members, not just developers.

- **No Java Dependency**: Built as a lightweight, standalone application that doesn't require a Java environment.

- **Simplified Setup**: Get started in seconds with our easy-to-use interface - no complex configuration required.

### Advantages over other mocking frameworks:

- **Language Agnostic**: Not tied to any specific programming language or framework.

- **No Code Changes Required**: Use our mock server without modifying your application code - just point your API requests to our server.

- **Feature-Rich Out of the Box**: Advanced features like authentication, rate limiting, and configurable delays come standard - no plugins or extensions needed.

## Getting Started

1. Install and run the Dynamic Mock API server
2. Access the web interface at `http://localhost:3000` (or your configured port)
3. Register your first endpoint by providing a path and JSON response
4. Start making requests to your mock endpoint

## Example Usage

- Register an endpoint at /api/users that returns a list of users
- Point your application to http://localhost:3000/api/users
- Dynamic Mock API will return your specified JSON response

## Installation

Dynamic Mock API consists of a Rust backend and a Svelte frontend. Don't worry if you've never used either of these technologies - the installation process is straightforward.

### Prerequisites

1. **Install Rust**:
    - Follow the instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
    - For Windows users: Choose the "rustup-init.exe" option
    - For macOS/Linux users: Run `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
    - Verify installation by opening a new terminal and typing `rustc --version`

2. **Install Node.js and npm** (required for the Svelte frontend):
    - Download and install from [https://nodejs.org/](https://nodejs.org/) (choose the LTS version)
    - Verify installation with `node --version` and `npm --version`

### Building and Running the Application

1. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/mockiapi.git
   cd mockiapi
   ```
2. **Build and run the backend**
   ```bash
   # Compile and run the Rust backend
   cargo run --release
   ```
3. **Build and run the frontend**
   ```bash
   # Navigate to the frontend directory
    cd frontend
    
    # Install dependencies
    npm install
    
    # Start the development server
    npm run dev
   ```
4. **Access the application:**
- Open your browser and go to http://localhost:3001
