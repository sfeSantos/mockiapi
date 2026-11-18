# 🚀 Dynamic Mock API

A powerful, flexible mock server to simulate REST & GraphQL APIs with customizable behavior—ideal for frontend development, integration testing, and rapid prototyping.

---

## 🌟 What Is It?

**Dynamic Mock API** lets you:

- Define endpoints via a web interface
- Return custom JSON responses
- Simulate authentication, rate limits, delays, and more
- Mock GraphQL queries and mutations
- Use dynamic variables in responses

Whether you're building web apps, mobile clients, or testing backend integrations—this tool helps you simulate real-world APIs effortlessly.

---

## ✨ Key Features

| Feature                        | Description                                                             |
|--------------------------------|-------------------------------------------------------------------------|
| 🧩 Easy Endpoint Setup         | Register REST/GraphQL endpoints using a friendly UI                     |
| 📄 JSON Mock Responses         | Return static or dynamic JSON responses                                 |
| 🔒 Authentication              | Support for Basic Auth and Bearer Token validation                      |
| ⏱️ Rate Limiting               | Limit number of requests per time window                                |
| ⏳ Configurable Delays          | Simulate network latency in milliseconds                                |
| 🔁 Custom HTTP Status Codes    | Return success, redirects, client or server error responses             |
| 📊 Request Logging             | Logs every request with metadata                                        |
| 🧪 GraphQL Support             | Define mock responses for queries and mutations                         |
| 🧠 Dynamic Response Variables  | Insert request values into your JSON response (e.g., path/query/header) |
| 🔌 gRPC Simulation             | Mock gRPC service calls using HTTP-based endpoints                      |

---

## ⚙️ Installation & Setup

## 📦 Latest Release

Download pre-built binaries for your platform:

- 🐧 [Linux (amd64)](https://github.com/sfeSantos/mockiapi/releases/latest/download/mockiapi-linux-amd64)  
- 🍎 [macOS (amd64)](https://github.com/sfeSantos/mockiapi/releases/latest/download/mockiapi-macos-amd64)  
- 🪟 [Windows (amd64)](https://github.com/sfeSantos/mockiapi/releases/latest/download/mockiapi-windows-amd64) (**don't forget to add the extension `.exe`**)

### 🔧 Prerequisites

- **Rust**: [Install Rust](https://www.rust-lang.org/tools/install)
- **Node.js + npm**: [Install Node.js](https://nodejs.org/) (LTS version recommended)

### 🏗️ Build & Run

#### Linux / macOS
```bash
cd mockiapi
chmod +x init.sh
./init.sh
```

#### Windows
```bash
double-click init.bat
```

### Access the App

Open your browser: http://localhost:3001

## 📚 Usage Examples

### 1️⃣ Register a Simple GET Endpoint

**Create a file `hello.json`:**
```json
{ "message": "Hello, world!" }
```
**Register in UI:**

| Path | Method | Response          |
|------|--------|-------------------|
|`/api/hello`| GET    | `hello.json`|

### 2️⃣ Dynamic Response Variables
```json
{
  "message": "Hello, {{name}}!",
  "user_id": "{{id}}",
  "requested_item": "{{item}}",
  "timestamp": "{{timestamp}}"
}
```
**Register in UI:**

| Path                                   | Dynamic Vars |
|----------------------------------------|--------------|
| `/api/user/{id}/item/{item}?name={name}` |       ✅      |

**Request:**
```http request
GET /api/user/123/item/laptop?name=John
```
**Response:**
```json
{
  "message": "Hello, John!",
  "user_id": "123",
  "requested_item": "laptop",
  "timestamp": "2025-03-31T12:00:00Z"
}
```
The `Dynamic Vars` option has to be checked for the placeholders to be replaced. Values come from:

- `{name}` segments of the registered path
- query parameters of the request, by their own name
- top-level fields of a JSON request body

A `{name}` segment matches any value, so one endpoint answers `/api/user/1`, `/api/user/2` and so on. A fixed query value in the registered path, such as `/search?kind=book`, must be present in the request, in any position.

### ⏱️ Rate Limit Format

The rate limit field takes `<requests>/<window in ms>`. For example, `10/1000` allows 10 requests per second for each HTTP method of the endpoint. Leave it empty for no limit.

### 3️⃣ GraphQL Mock Example

_Check `graphql.json` in uploads folder_

**Request:**
```json
{ "query": "query getUser { id name email }" }
```
**Or:**
```json
{ "query": "mutation createUser { success user { id name email } }" }
```
**Response (automatically matched):**
```json
{
  "data": {
   "id": "123",
   "name": "Alice",
   "email": "alice@example.com"
  }
}
```
### 1️⃣ Simulate a gRPC Call

You can simulate gRPC service methods via HTTP by registering an endpoint with the following structure:

**Example: `POST /grpc` &rarr; will be the default endpoint**

Fill the form with the information required and register the following json:
```json
{
  "id": "b123",
  "title": "The Rust Programming Language",
  "author": "Steve Klabnik"
}
```

Then do a POST to /grpc with the following **request body**:

```json
{
  "service": "com.example.BookService",
  "rpc": "GetBook",
  "request": { "id": "b123" }
}
```
The response will be the contents of the json registered

## 🧠 Why Use This?

| Benefit               | Description                                                                 |
|------------------------|-----------------------------------------------------------------------------|
| ✅ Developer-Friendly  | Configure endpoints easily through a UI—no need to edit JSON or YAML files |
| 🚫 No Java Required    | Built in Rust, so there’s no need for a Java runtime or heavy frameworks   |
| ⚙️ Feature-Rich        | Includes auth, delays, rate limiting, and dynamic vars out of the box      |
| 🔁 Instant Mock Updates| Change responses on the fly without restarting or redeploying              |
| 🌍 Language-Agnostic   | Works with any tech stack that supports HTTP requests                      |
| 🧪 GraphQL Support     | Simulate GraphQL queries and mutations with minimal setup                  |
| 🔐 Authentication      | Easily test Basic or Token-protected endpoints                            |

