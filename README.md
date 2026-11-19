# mockiapi

mockiapi is a mock HTTP server. You tell it "when a request arrives at this path, answer with this JSON file", and it does. Your frontend, mobile app or integration test can then talk to `http://localhost:3001` while the real API is not ready or not reachable.

Endpoints are registered while the server is running, from a web dashboard or with `curl`. Nothing is configured in files and nothing needs a restart.

What an endpoint can do:

- answer with a fixed JSON file, on the HTTP methods you choose
- accept path parameters such as `/api/user/{id}` and copy request values into the response
- answer with any status code, after a delay, or under a rate limit
- require Basic auth or a bearer token
- answer GraphQL queries and mutations, returning only the selected fields
- answer simulated gRPC calls sent as JSON over HTTP

Two things to know before you start:

- Endpoints are kept in memory. When the server stops, they are gone and have to be registered again.
- The response file must be JSON. The server always answers with `Content-Type: application/json`.

## Contents

1. [Start the server](#1-start-the-server)
2. [Register your first endpoint](#2-register-your-first-endpoint)
3. [Use path parameters and dynamic values](#3-use-path-parameters-and-dynamic-values)
4. [Simulate errors, slow responses and rate limits](#4-simulate-errors-slow-responses-and-rate-limits)
5. [Protect an endpoint](#5-protect-an-endpoint)
6. [Mock a GraphQL API](#6-mock-a-graphql-api)
7. [Mock a gRPC call](#7-mock-a-grpc-call)
8. [List and delete endpoints](#8-list-and-delete-endpoints)
9. [Reference](#reference)

## 1. Start the server

Pick one of the three ways below. In all of them the server listens on port 3001, unless you [choose another port](#on-another-port).

### With Docker

```bash
docker compose up -d
```

Run it from the repository root. It uses the `docker-compose.yml` there, which starts the published `edufsantos/mockiapi` image and maps port 3001.

### From source

Install [Rust](https://www.rust-lang.org/tools/install) and [Node.js with npm](https://nodejs.org/), then:

```bash
# Linux or macOS
chmod +x init.sh
./init.sh
```

On Windows, run `init.bat`.

The script builds the dashboard into `frontend/dist`, builds the server with the dashboard embedded in it, and starts the server with request logging on.

### With a release binary

- [Linux (amd64)](https://github.com/sfeSantos/mockiapi/releases/latest/download/mockiapi-linux-amd64)
- [macOS (amd64)](https://github.com/sfeSantos/mockiapi/releases/latest/download/mockiapi-macos-amd64)
- [Windows (amd64)](https://github.com/sfeSantos/mockiapi/releases/latest/download/mockiapi-windows-amd64) (add the `.exe` extension after downloading)

```bash
chmod +x mockiapi-linux-amd64
RUST_LOG=info ./mockiapi-linux-amd64
```

The download is a single executable with the dashboard inside it. There is nothing else to download, and it can be started from any directory.

### On another port

Pass `--port`, or set the `PORT` environment variable. The argument wins when both are given.

```bash
./mockiapi-linux-amd64 --port 8080
PORT=8080 ./mockiapi-linux-amd64
```

The init scripts hand their arguments to the server, so `./init.sh --port 8080` works too. With Docker, set the variable and publish the same port:

```bash
docker run -e PORT=8080 -p 8080:8080 edufsantos/mockiapi
```

If the port is already in use, the server says so and stops. The rest of this tutorial uses 3001.

### Check that it is running

The server prints:

```
.: Server running at http://localhost:3001
```

Open http://localhost:3001 in a browser. You should see the **MockiAPI Dashboard**, with a registration form and an empty list of endpoints.

The server creates an `uploads` directory in the directory it was started from. That is where it keeps a copy of each response file.

## 2. Register your first endpoint

Create a file named `hello.json`:

```json
{ "message": "Hello, world!" }
```

**In the dashboard**, stay on the **REST** tab and fill in:

| Field | Value |
|---|---|
| Path | `/api/hello` |
| HTTP Methods | GET |
| Response (JSON File) | `hello.json` |

Leave the other fields empty and click **Register**. The endpoint shows up in the list.

**With curl**, the same registration is:

```bash
curl -X POST http://localhost:3001/register \
  -F path=/api/hello \
  -F methods=GET \
  -F file=@hello.json
```

```
"Registered successfully"
```

Now call it:

```bash
curl http://localhost:3001/api/hello
```

```json
{ "message": "Hello, world!" }
```

The rules for a registration:

- The path is required and must start with `/`.
- At least one HTTP method is required. For several, tick more than one box, or send them comma-separated: `-F methods=GET,POST`.
- The response file is required.
- Registering a path that already exists replaces the endpoint that was there.

And what callers get when a request does not fit:

| Request | Answer |
|---|---|
| A path nobody registered | `404 Resource not found` |
| A registered path with a method it was not registered for | `405 Method not allowed` |

## 3. Use path parameters and dynamic values

A `{name}` segment in the path matches any value. Registering `/api/user/{id}` answers `/api/user/1`, `/api/user/2` and every other id with the same file.

To make the response change with the request, write `{{name}}` placeholders in the response file and turn on **Dynamic Vars**. Create `user.json`:

```json
{
  "message": "Hello, {{name}}!",
  "user_id": "{{id}}",
  "requested_item": "{{item}}",
  "timestamp": "{{timestamp}}"
}
```

**In the dashboard:**

| Field | Value |
|---|---|
| Path | `/api/user/{id}/item/{item}?name={name}` |
| HTTP Methods | GET |
| Dynamic Vars | checked |
| Response (JSON File) | `user.json` |

**With curl:**

```bash
curl -X POST http://localhost:3001/register \
  -F 'path=/api/user/{id}/item/{item}?name={name}' \
  -F methods=GET \
  -F with_dynamic_vars=true \
  -F file=@user.json
```

Call it:

```bash
curl "http://localhost:3001/api/user/123/item/laptop?name=John"
```

```json
{
  "message": "Hello, John!",
  "user_id": "123",
  "requested_item": "laptop",
  "timestamp": "2026-11-18T22:34:07.857749903+00:00"
}
```

Note the two kinds of braces: single braces `{id}` in the **path** say where a value is read from, double braces `{{id}}` in the **file** say where it is written.

Where placeholder values come from:

- `{name}` segments of the registered path
- query parameters of the request, by their own name
- top-level fields of a JSON request body, so a `POST` with `{"name": "John"}` fills `{{name}}`
- `{{timestamp}}`, which is always the current UTC time

How it behaves at the edges:

- Without **Dynamic Vars** checked, the file is sent exactly as uploaded, placeholders included.
- A placeholder with no matching value is left in the response as it is.
- A fixed query value in the registered path, such as `/search?kind=book`, must be present in the request for the endpoint to match. Its position among the other parameters does not matter.

## 4. Simulate errors, slow responses and rate limits

Three optional fields change how an endpoint answers:

| Field | Format | Effect |
|---|---|---|
| Status Code | A number, for example `503` | The status of the response. Empty means 200. |
| Delay (ms) | A number of milliseconds | The server waits that long before answering. |
| Rate Limit | `<requests>/<window in ms>` | Requests over the budget get `429 Rate limit exceeded`. |

This endpoint answers `503` after half a second, and accepts two requests every ten seconds:

**In the dashboard:**

| Field | Value |
|---|---|
| Path | `/api/slow` |
| HTTP Methods | GET |
| Status Code | `503` |
| Delay (ms) | `500` |
| Rate Limit | `2/10000` |
| Response (JSON File) | `hello.json` |

**With curl:**

```bash
curl -X POST http://localhost:3001/register \
  -F path=/api/slow \
  -F methods=GET \
  -F status_code=503 \
  -F delay=500 \
  -F rate_limit=2/10000 \
  -F file=@hello.json
```

Call it three times in a row and print the status and the time each call took:

```bash
for i in 1 2 3; do
  curl -s -o /dev/null -w "%{http_code} %{time_total}s\n" http://localhost:3001/api/slow
done
```

```
503 0.501404s
503 0.502501s
429 0.000696s
```

The first two calls waited 500 ms and got the registered status. The third was over the limit and was refused right away.

The rate limit is counted per endpoint and per HTTP method. With `10/1000` on an endpoint registered for GET and POST, each method gets its own 10 requests per second.

## 5. Protect an endpoint

An endpoint with authentication answers `401 Unauthorized` unless the request carries the right `Authorization` header. There are two kinds.

### Basic auth

**In the dashboard**, set **Authentication Type** to **Basic Auth** and fill in the username and password.

**With curl**, send the credentials as JSON in the `authentication` field:

```bash
curl -X POST http://localhost:3001/register \
  -F path=/api/private \
  -F methods=GET \
  -F 'authentication={"username":"admin","password":"secret"}' \
  -F file=@hello.json
```

```bash
curl http://localhost:3001/api/private
# 401 Unauthorized

curl -u admin:secret http://localhost:3001/api/private
# { "message": "Hello, world!" }
```

`curl -u` builds the header for you. Other clients send `Authorization: Basic <base64 of username:password>`.

### Bearer token

**In the dashboard**, set **Authentication Type** to **Token Auth**. The **Token Data (JSON)** box expects a JSON object with a `tokenData` field, not the bare token:

```json
{ "tokenData": "my-secret-token" }
```

**With curl:**

```bash
curl -X POST http://localhost:3001/register \
  -F path=/api/token \
  -F methods=GET \
  -F 'authentication={"tokenData":"my-secret-token"}' \
  -F file=@hello.json
```

```bash
curl -H "Authorization: Bearer my-secret-token" http://localhost:3001/api/token
# { "message": "Hello, world!" }
```

These credentials are test data. The server stores them as plain text and returns them in `/list`, so don't use real ones.

## 6. Mock a GraphQL API

A GraphQL endpoint takes one file that describes every operation it answers. The file has a `query` section and a `mutation` section. Inside each, the key is the operation name and `data` is the full object that operation can return.

Create `graphql.json`:

```json
{
  "query": {
    "getUser": {
      "data": { "id": "123", "name": "John Doe", "email": "john@example.com", "age": 30 }
    }
  },
  "mutation": {
    "createUser": {
      "data": { "success": true, "user": { "id": "789", "name": "New User" } }
    }
  }
}
```

**In the dashboard**, open the **GraphQL** tab, choose `graphql.json` and click **Register**. The path and method fields are hidden because they are fixed: the endpoint is always `POST /api/graphql`.

**With curl:**

```bash
curl -X POST http://localhost:3001/register \
  -F path=/api/graphql \
  -F methods=POST \
  -F isGraphQL=true \
  -F file=@graphql.json
```

Send a query. The operation name, `getUser`, picks the mock:

```bash
curl -X POST http://localhost:3001/api/graphql \
  -d '{ "query": "query getUser { id name email }" }'
```

```json
{"data":{"email":"john@example.com","id":"123","name":"John Doe"}}
```

`age` is in the file but was not selected, so it is not in the response. Nested selections work the same way:

```bash
curl -X POST http://localhost:3001/api/graphql \
  -d '{ "query": "mutation createUser { success user { id } }" }'
```

```json
{"data":{"success":true,"user":{"id":"789"}}}
```

What the request has to look like:

- The body is JSON with a `query` field holding the GraphQL text.
- The operation must be a **named** query or mutation. `{ id }` with no name, or a subscription, gets `400 Invalid GraphQL request`.
- A name that is not in the file gets `404 Resource not found`.
- Arguments and variables are accepted by the parser and ignored. The answer depends only on the operation name and the selected fields.

A longer sample file is in `uploads/graphql.json`.

## 7. Mock a gRPC call

mockiapi does not speak the gRPC wire protocol. It simulates a call as a plain HTTP request: the client posts JSON naming the service and the method, and gets back the JSON you registered for that pair.

Create `book.json`, the answer for `BookService.GetBook`:

```json
{
  "id": "b123",
  "title": "The Rust Programming Language",
  "author": "Steve Klabnik"
}
```

**In the dashboard**, open the **gRPC** tab and fill in:

| Field | Value |
|---|---|
| Service | `com.example.BookService` |
| RPC Method | `GetBook` |
| Response (JSON File) | `book.json` |

**With curl:**

```bash
curl -X POST http://localhost:3001/register \
  -F path=/grpc \
  -F methods=POST \
  -F grpcService=com.example.BookService \
  -F grpcRPC=GetBook \
  -F file=@book.json
```

Every gRPC mock is called at `POST /grpc`. The body names the service and the method:

```bash
curl -X POST http://localhost:3001/grpc \
  -d '{ "service": "com.example.BookService", "rpc": "GetBook", "request": { "id": "b123" } }'
```

```json
{"author":"Steve Klabnik","id":"b123","title":"The Rust Programming Language"}
```

How it behaves:

- To mock more methods, register again with another service and RPC method. They all share `/grpc`.
- The `request` field is accepted but not looked at. Any payload gets the same answer.
- A service or method that was not registered gets `404` with `{"error":"Mock not found"}`.
- The status code and delay from the form apply to the answer.

## 8. List and delete endpoints

The dashboard lists every endpoint with its type, path, methods, status and whether it asks for authentication. The **Delete** button removes one.

**With curl**, list them:

```bash
curl http://localhost:3001/list
```

```json
{
  "/api/slow": {
    "type": "REST",
    "method": ["GET"],
    "file": "uploads/b2bc44bf-f5eb-4c0e-aebf-4061dd9f3e1b.json",
    "status_code": 503,
    "delay": 500,
    "rate_limit": { "requests": 2, "window_ms": 10000 },
    "with_dynamic_vars": null
  }
}
```

Delete one by its registered path, URL-encoded (`/` becomes `%2F`):

```bash
curl -X DELETE http://localhost:3001/delete/%2Fapi%2Fhello
```

```
Deleted successfully
```

After that, `GET /api/hello` is back to `404`.

## Reference

### Registration fields

`POST /register` takes a multipart form. These are the fields the dashboard sends.

| Field | Required | Value |
|---|---|---|
| `path` | yes | Starts with `/`. May contain `{name}` segments and a query string. |
| `methods` | yes | Comma-separated HTTP methods, for example `GET,POST` |
| `file` | yes | The JSON response file, 5 MB at most |
| `status_code` | no | Defaults to 200 |
| `delay` | no | Milliseconds |
| `rate_limit` | no | `<requests>/<window in ms>` |
| `authentication` | no | `{"username":"...","password":"..."}` or `{"tokenData":"..."}` |
| `with_dynamic_vars` | no | `true` to replace `{{name}}` placeholders (REST only) |
| `isGraphQL` | no | `true` for a GraphQL endpoint |
| `grpcService`, `grpcRPC` | no | Both set for a gRPC mock |

A form the server refuses gets `400` with the reason as a JSON string, for example `"path is required and must start with /"`.

### Reserved paths

`POST /register`, `GET /list` and `DELETE /delete/...` belong to the server, and so do the dashboard's own files (`/`, `/assets/...`). They are matched before the mocks, so a mock registered on one of them is never reached.

### Answers the server gives on its own

| Status | Body | When |
|---|---|---|
| 400 | `Invalid GraphQL request` | The GraphQL body is not a named query or mutation |
| 400 | `Invalid gRPC request` | The gRPC body has no `service` or `rpc` |
| 401 | `Unauthorized` | Missing or wrong `Authorization` header |
| 404 | `Resource not found` | No endpoint matches the path |
| 405 | `Method not allowed` | The endpoint exists but not for this method |
| 429 | `Rate limit exceeded` | The endpoint's budget for this method is used up |

### Logs

Logging is off unless the `RUST_LOG` environment variable is set. With `RUST_LOG=info`, the server writes one line per request with the client address, method, path, status, time taken and headers.

### Development

The server embeds the dashboard, so the dashboard is built first:

```bash
cd frontend
npm install
npm run build
cd ..
cargo test
```

`npm run build` produces `frontend/dist`, with a gzip copy (`.gz`) next to each text file. The server sends that copy to clients that accept gzip, so it never compresses anything itself. Without `frontend/dist`, `cargo build` and `cargo test` stop with a message that says to build the frontend.

The dashboard is written in TypeScript. `npm run check` inside `frontend/` type-checks it.

How the server gets the dashboard depends on the build:

- A release build (`cargo build --release`) compiles `frontend/dist` into the binary. Rebuild the server after changing the dashboard.
- A debug build (`cargo run`) reads `frontend/dist` from disk on every request, so `npm run build` is enough to see a dashboard change.

## License

Licensed under either of [MIT](LICENSE) or [Apache 2.0](LICENSE-APACHE), at your option.
