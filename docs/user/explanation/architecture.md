---
title: Architecture
description: How the Release Regent crates fit together and what deployment shapes are possible
---

# Architecture

Release Regent is implemented as a Cargo workspace of focused crates that communicate through
well-defined interfaces. The design follows the **hexagonal architecture** (ports and adapters)
pattern, which keeps the core release logic independent of GitHub, the file system, and the
deployment target.

## Crate map

```
release_regent/
├── crates/
│   ├── core/           # All release logic — no I/O dependencies
│   ├── cli/            # rr binary: local testing and config tools
│   ├── server/         # rr-server binary: webhook HTTP server
│   ├── github_client/  # GitHub API adapter
│   ├── config_provider/# Configuration file adapter
│   └── testing/        # Shared test utilities
```

### `core`

The `core` crate contains every piece of release logic:

- Version calculation from conventional commits
- Changelog generation (via git-cliff-core)
- Release PR creation and update decisions
- GitHub release creation
- PR comment command processing

`core` defines **port traits** (`GitHubOperations`, `ConfigurationProvider`, `VersionCalculator`,
`GitOperations`) but contains no concrete implementations of them. This is the hexagonal
pattern: core business logic depends on abstractions, not on I/O implementations.

### `github_client`

Implements the `GitHubOperations` trait using the GitHub REST API. Handles:

- JWT signing and installation token exchange
- Rate limiting and exponential backoff retry
- REST API calls for PRs, releases, tags, commits, and labels
- Webhook HMAC-SHA256 signature validation

### `config_provider`

Implements the `ConfigurationProvider` trait by reading `.release-regent.toml` files from the
file system.

### `server`

Wires together `core`, `github_client`, and `config_provider` in a long-running Axum HTTP
server. Accepts GitHub webhooks, validates signatures using the `github-bot-sdk` crate, and
feeds events into `core` through an in-memory `mpsc` channel.

The server supports graceful shutdown: a `CancellationToken` is shared between the HTTP
listener and the event processing loop. `SIGINT` / `SIGTERM` cancel the token, causing both
to drain and exit cleanly.

### `cli`

Wires together `core`, `github_client`, and `config_provider` for local use. The CLI reads
events from JSON files rather than over the network, which makes it useful for:

- Testing configuration against real commit history (`rr test`)
- Replaying an event file without a live GitHub connection (`rr run --mock`)
- Generating sample configuration and webhook fixtures (`rr init`, `rr generate`)

### `testing`

Shared test infrastructure: in-memory mock implementations of the port traits that the `core`
unit tests and CLI tests use. Not shipped in production binaries.

## Dependency graph

```
         ┌───────────────────────────────┐
         │              core             │
         │  (traits + business logic)    │
         └──────┬──────────┬────────────┘
                │          │
     implements │          │ implements
                │          │
    ┌───────────▼──┐    ┌──▼────────────────┐
    │ github_client│    │  config_provider   │
    └──────────────┘    └────────────────────┘
           ▲                    ▲
           │ wired by           │ wired by
      ┌────┴────────────────────┴────┐
      │    server  /  cli            │
      └──────────────────────────────┘
```

## Deployment shapes

Because the runtime wiring happens in the binary crates (`server` and `cli`), new deployment
shapes can be added without touching `core`:

| Shape | Binary | Notes |
| :--- | :--- | :--- |
| Long-running server | `rr-server` | Production; receives live webhooks |
| Local CLI | `rr` | Development and testing |
| Azure Function | (example in `docs/examples/`) | Serverless; instantiated per event |
| MCP server | (possible future shape) | Integrates with AI agent tooling |

The Azure Function shape is documented as a code example rather than a shipped binary because
it requires a different build target and deployment pipeline. The core logic is identical.

---

## Related reading

- [ADR-001: Hexagonal architecture](https://github.com/pvandervelde/release_regent/blob/main/docs/adr/ADR-001-hexagonal-architecture.md)
- [ADR-002: Long-running server deployment model](https://github.com/pvandervelde/release_regent/blob/main/docs/adr/ADR-002-long-running-server-deployment-model.md)
- [Deploy the server](../how-to/setup/install-server.md)
