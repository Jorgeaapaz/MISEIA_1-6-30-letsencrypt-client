# ADR-002: Use Axum for the HTTP-01 Challenge Server

## Status
Accepted

## Date
2026-06-29

## Context

The ACME HTTP-01 challenge requires serving a key-authorization string at
`/.well-known/acme-challenge/<token>` over plain HTTP on port 80 (or a configurable port)
for the duration of the challenge. The server must:

1. Start and stop programmatically within an async Rust process.
2. Serve multiple tokens concurrently (multi-domain orders).
3. Shut down cleanly once the challenge is complete.

Three options were considered:

- **`std::net::TcpListener` + manual HTTP parsing** — minimal deps, but writing correct
  HTTP/1.1 parsing inline is error-prone and untested.
- **`hyper` raw service** — fine-grained control, but requires writing `Service` boilerplate
  manually.
- **`axum`** — already a project dependency (Tokio ecosystem), provides a safe routing macro,
  and integrates with `tokio::sync::oneshot` for graceful shutdown.

## Decision

Use `axum` with a `Router` to serve the single challenge route. Token state is shared via
`Arc<Mutex<HashMap<String, String>>>` injected through `axum`'s typed `State`. Graceful
shutdown is wired through a `tokio::sync::oneshot::channel` so the main flow can stop the
server with a single `sender.send(())`.

## Consequences

### Positive
- Route definition is explicit and easy to test; `serve_challenge` is a plain async function.
- Graceful shutdown via `oneshot` avoids abrupt socket closure during challenge validation.
- `Arc<Mutex<HashMap>>` pattern is trivially testable without a running network (unit tests
  call `add_token` directly on the shared map).
- No additional dependency — `axum` is already used elsewhere in the async runtime.

### Negative / Trade-offs
- `axum` brings `tower` and `hyper` as transitive dependencies; a raw `TcpListener` would
  have zero extra cost.
- `Mutex` introduces a lock on every incoming request; acceptable at challenge-server
  concurrency levels (typically one or two concurrent requests per domain).

### Neutral
- The challenge server runs only for the duration of the ACME authorization round (seconds),
  not as a long-lived service, so its performance profile is not a concern.
