# Dynamic Mock API 🚀

A powerful and easy-to-use mock API server that allows you to define endpoints, return custom JSON responses, and simulate real-world API behavior.

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

## **✨ Dynamic Response Variables**
Use placeholders in JSON responses that get replaced at runtime based on request parameters. This allows for **customized responses** based on URL path, query parameters, or request headers.

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

1. **Linux / Mac**:
   ```bash
   chmod +x init.sh
   cd mockiapi
   ./init.sh
   ```
2. **Windows**
   ```bash
   double-click init.bat
   ```

3. **Access the application:**
- Open your browser and go to http://localhost:3001

# Examples

### **1️⃣ Simple GET Request**
Create a JSON file (`hello.json`)
```json
{
  "message": "Hello, world!"
}
```
### **2️⃣ Register an Endpoint**
Define an endpoint that uses dynamic variables:

| Path         | Response File        |
|--------------|----------------------|
| `/api/hello` | `hello.json` |       |

### **1️⃣ Simple POST Request with custom Http Status**
If you want, you can provide a [dynamic json](#dynamic-vars-usage) to as return. But is not mandatory

| Path        | Response File |Method| Status Copde       |
|-------------|---------------|------|--------------------|
| `/api/save` | `optional`    | `POST`  | `202 (or any other)`  |

## Dynamic Vars usage

### **1️⃣ Define Your Response (JSON File)**
Create a JSON file (`user-response.json`) with placeholders:
```json
{
  "message": "Hello, {{name}}!",
  "user_id": "{{id}}",
  "requested_item": "{{item}}",
  "timestamp": "{{timestamp}}"
}
```
### **2️⃣ Register an Endpoint**
Define an endpoint that uses dynamic variables:

| Path                         | Response File            | With Dynamic Vars |  
|------------------------------|--------------------------|-------------------|  
| `/api/user/{id}/item/{item}?name={name}` | `uploads/user-response.json` | ✅ |

### **3️⃣ Make a Request**
Request:
```http
GET /api/user/123/item/laptop?name=John
```
### **4️⃣ MockiAPI Responds Dynamically**  
Response:
```json
{
  "message": "Hello, John!",
  "user_id": "123",
  "requested_item": "laptop",
  "timestamp": "2025-03-31T12:00:00Z"
}
```

### **1️⃣ Update with Delay**

| Path                | Response File | Method | Status Copde | Delay       |
|---------------------|---------------|--------|--------------|-------------|
| `/api/delay/update` | `optional`    | `PUT`  | `optional`   | `3000 (ms)` |

### **1️⃣ GET Request with Rate Limiting**
Register a `GET` endpoint that enforces rate limiting (max 5 requests per minute).

```json
{
  "id": "1ds",
  "key": "value"
}
```
### **2️⃣ Register an Endpoint**

| Path         | Response File | Method | Status Copde | Rate Limit |
|--------------|---------------|--------|--------------|------------|
| `/api/limited` | `optional`    | `GET`  | `optional`   | `10/60000` |

### **3️⃣ Make a Request**
Request:
```http
GET /api/limited
```