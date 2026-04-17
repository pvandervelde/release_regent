# =============================================================================
# Release Regent — multi-stage Docker build
#
# Produces a minimal runtime image containing only the `rr-server` binary.
#
# Required environment variables at runtime
# -----------------------------------------
# GITHUB_WEBHOOK_SECRET  – HMAC-SHA256 secret shared with the GitHub App
# GITHUB_APP_ID          – Numeric GitHub App identifier
# GITHUB_PRIVATE_KEY     – PEM-encoded GitHub App private key
# GITHUB_INSTALLATION_ID – GitHub App installation identifier
#
# Optional environment variables
# -----------------------------------------
# CONFIG_DIR             – Directory containing .release-regent.toml (default: cwd)
# ALLOWED_REPOS          – Comma-separated owner/repo list, or * for all (default: *)
# EVENT_CHANNEL_CAPACITY – In-flight event queue depth (default: 1024)
# PORT                   – TCP port to listen on (default: 8080)
# RUST_LOG               – Log filter string (default: info)
#
# Health check
# -----------------------------------------
# GET /health returns {"status":"healthy","service":"release-regent-webhook"}
# =============================================================================

# =============================================================================
# Stage 1 — dependency compilation cache
#
# Copy only the workspace manifests and create minimal stub source files.
# This layer is cached as long as Cargo.toml / Cargo.lock are unchanged,
# so external crate compilation is skipped on source-only changes.
# =============================================================================
# Pin to a specific Rust release so that builds on different days produce
# identical binaries.  Update this together with rust-version in Cargo.toml
# when raising the MSRV.  Current stable at the time of writing: 1.95.
FROM rust:1.95-slim AS deps

# TARGETARCH is injected by BuildKit (values: amd64, arm64, etc.).
# nasm is only available for x86/amd64 in Debian; aws-lc-sys uses it for
# x86 assembly optimisations.  On arm64 it is not available and not needed
# (aws-lc-sys uses ARMv8 crypto intrinsics instead).
# cmake, clang, and pkg-config are required on all platforms.
ARG TARGETARCH
RUN set -e; \
    apt-get update; \
    apt-get install -y --no-install-recommends cmake clang pkg-config; \
    if [ "${TARGETARCH}" = "amd64" ]; then \
    apt-get install -y --no-install-recommends nasm; \
    fi; \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace and per-crate manifests.
COPY Cargo.toml Cargo.lock ./
COPY crates/cli/Cargo.toml           crates/cli/Cargo.toml
COPY crates/config_provider/Cargo.toml crates/config_provider/Cargo.toml
COPY crates/core/Cargo.toml          crates/core/Cargo.toml
COPY crates/github_client/Cargo.toml crates/github_client/Cargo.toml
COPY crates/server/Cargo.toml        crates/server/Cargo.toml
COPY crates/testing/Cargo.toml       crates/testing/Cargo.toml

# Create minimal stub sources for every workspace member so that Cargo can
# resolve and compile external dependencies.  The stubs intentionally omit
# symbols that the server imports from workspace crates, so the overall build
# will fail (expected); the || true absorbs that failure while Cargo's
# dependency compilation output remains cached in target/.
RUN mkdir -p \
    crates/cli/src \
    crates/config_provider/src \
    crates/core/src \
    crates/github_client/src \
    crates/server/src \
    crates/testing/src \
    && printf 'fn main() {}\n' > crates/cli/src/main.rs \
    && printf 'pub fn _stub() {}\n' > crates/config_provider/src/lib.rs \
    && printf 'pub fn _stub() {}\n' > crates/core/src/lib.rs \
    && printf 'pub fn _stub() {}\n' > crates/github_client/src/lib.rs \
    && printf 'fn main() {}\n' > crates/server/src/main.rs \
    && printf 'pub fn _stub() {}\n' > crates/testing/src/lib.rs \
    && cargo build --release --locked --package release-regent-server || true

# =============================================================================
# Stage 2 — final compilation
#
# Copy the real source code on top of the stub layer.  Cargo reuses the
# cached external-crate artifacts from Stage 1 and only recompiles the
# workspace crates whose source has changed.
# =============================================================================
FROM deps AS builder

COPY crates/ crates/

# Touch all .rs files so that Cargo detects them as newer than the stubs.
RUN find crates -name '*.rs' -exec touch {} + \
    && cargo build --release --locked --package release-regent-server

# =============================================================================
# Stage 3 — minimal runtime image
# =============================================================================
FROM debian:bookworm-slim AS runtime

# ca-certificates is required for outbound TLS connections to the GitHub API.
# wget is used by the HEALTHCHECK instruction below.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Run as a dedicated non-root system account.
RUN useradd \
    --system \
    --no-create-home \
    --shell /bin/false \
    --uid 10001 \
    rr

COPY --from=builder /build/target/release/rr-server /usr/local/bin/rr-server

USER rr

# Webhook server port.
EXPOSE 8080

# Container health check — polls the /health endpoint.
# start-period gives the server time to bind and start accepting connections.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q --spider http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/rr-server"]
