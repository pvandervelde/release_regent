# ADR-002: Container-Based Deployment Model

Status: Accepted
Date: 2026-03-13
Owners: ReleaseRegent team

## Context

The architecture spec (`docs/specs/architecture/overview.md`) described a serverless,
function-based deployment target (Azure Functions / AWS Lambda). That model has been
superseded: Release Regent will be deployed as a container (Docker / OCI image) running in a
container orchestration platform (e.g. Kubernetes, Azure Container Apps, AWS ECS).

The `crates/server` crate implements a long-running Axum HTTP server, which maps directly onto
this model.

This ADR records the decision to adopt containers as the deployment target, deprecating the
serverless design in the spec.

## Decision

Deploy Release Regent as an OCI container image built from `crates/server`. The hosting target
is a container orchestration platform, not a serverless functions runtime.

The serverless model described in the original architecture spec is **superseded** by this
decision.

## Consequences

**Enables:**

- Consistent deployment in any OCI-compatible environment (Kubernetes, ACA, ECS, Docker Compose).
- `cargo run` and `docker run` local development with no extra tooling.
- Full control over process lifecycle, connection pooling, and in-memory state.
- Straightforward horizontal scaling via container replicas with a load balancer in front.
- Predictable latency — no cold-start penalty.

**Forbids:**

- Deploying directly to a managed serverless runtime (Azure Functions, AWS Lambda) without an
  HTTP reverse-proxy shim; that deployment path is no longer a supported target.

**Trade-offs:**

- Operators must provision and manage container infrastructure (orchestrator, image registry,
  ingress). There is no auto-scale-to-zero.
- Always-on compute costs are higher at very low traffic volumes compared to a per-invocation
  billing model.

## Alternatives considered

### Option A: Serverless functions runtime (original spec)

**Why not**: Requires platform-specific trigger bindings and local emulators for development.
The team is targeting container platforms where this overhead adds no value. Custom Handlers
(Azure) and Lambda Web Adapter (AWS) partially close the gap but add operational complexity
without meaningful benefit over a container deployment.

### Option B: Separate crates per deployment target

Create `crates/server` (long-running) and `crates/azure-function` / `crates/lambda` in
parallel.

**Why not**: Premature given the container direction. Additional host crates can be added later
if a serverless deployment requirement emerges; the hexagonal architecture (ADR-001) makes
that straightforward without touching core logic.

## Implementation notes

### Container image

The `crates/server` binary is the container entrypoint. A minimal Dockerfile pattern:

```dockerfile
FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p release-regent-server

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/rr-server /usr/local/bin/rr-server
ENTRYPOINT ["rr-server"]
```

### Configuration via environment variables

| Variable                  | Description                                          | Default |
|---------------------------|------------------------------------------------------|---------|
| `GITHUB_WEBHOOK_SECRET`   | HMAC-SHA256 secret (**required**)                    | —       |
| `GITHUB_APP_ID`           | GitHub App numeric ID (**required**)                 | —       |
| `GITHUB_PRIVATE_KEY`      | PEM-encoded GitHub App private key (**required**)    | —       |
| `CONFIG_DIR`              | Directory containing `.release-regent.toml`          | current directory |
| `ALLOWED_REPOS`           | Comma-separated `owner/repo` list, or `*`            | `*`     |
| `EVENT_CHANNEL_CAPACITY`  | Bounded channel depth for in-flight events           | `1024`  |
| `PORT`                    | TCP port the server binds                            | `8080`  |

### Health check

`GET /` returns `{"status":"healthy"}` with HTTP 200 and should be used as the container
liveness and readiness probe.

## References

- ADR-001: Hexagonal Architecture for ReleaseRegent
- `docs/specs/architecture/overview.md` — system architecture spec (updated by this ADR)
- `crates/server/src/main.rs` — Axum server entry point
