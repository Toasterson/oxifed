# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Start infrastructure (MongoDB + LavinMQ)
docker-compose up -d mongodb lavinmq

# Build everything
cargo build

# Build a specific crate
cargo build -p domainservd

# Run all tests
cargo test --all-features --workspace

# Run tests for a specific crate
cargo test --package domainservd

# Run a single test by name
cargo test --package oxifed test_name -- --nocapture

# Run doc tests
cargo test --doc --workspace

# Formatting (CI enforces this)
cargo fmt --all -- --check

# Linting (CI enforces -D warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Run domainservd locally (requires MongoDB + LavinMQ running)
cargo run -p domainservd

# Run the admin CLI
cargo run -p oxiadm -- --help
```

## Architecture

Oxifed is a multi-domain ActivityPub federation platform built as communicating microservices connected via message queue (LavinMQ/RabbitMQ).

### Crate Layout

- **`oxifed`** (root `src/`): Shared library containing ActivityPub types, MongoDB database manager, HTTP signature implementation, PKI module, WebFinger protocol, and the ActivityPub HTTP client. All other crates depend on this.
- **`domainservd`** (`crates/domainservd/`): Axum web server exposing ActivityPub endpoints (inbox, outbox, actor, webfinger). Consumes RabbitMQ messages for domain/user management. Binds to port 8080.
- **`publisherd`** (`crates/publisherd/`): Worker daemon that consumes activities from RabbitMQ and delivers them to remote inboxes with HTTP signatures. Configurable worker count and retry logic.
- **`oxiadm`** (`crates/oxiadm/`): Clap-based CLI for administration. Sends commands via RabbitMQ messages and uses RPC for queries (domain/user listing).
- **`oxifed-operator`** (`crates/oxifed-operator/`): Kubernetes operator managing `Domain` CRDs (v1alpha1). Generates cryptographic keys, stores them in K8s Secrets, and syncs to MongoDB.

### Communication Flow

```
oxiadm (CLI) --[AMQP]--> domainservd (HTTP API + message consumer)
                              |
                         [AMQP publish]
                              |
                         publisherd (delivery workers) --[HTTP+signatures]--> remote inboxes
```

All services share MongoDB as the data store. RabbitMQ/LavinMQ handles async messaging with defined exchanges: `EXCHANGE_ACTIVITYPUB_PUBLISH`, `EXCHANGE_RPC_REQUEST`, `EXCHANGE_RPC_RESPONSE`, `EXCHANGE_DOMAIN_MANAGEMENT`.

### Key Modules in the Root Crate

- `database.rs`: MongoDB `DatabaseManager` with collections for actors, objects, keys, domains, followers, following. Handles index creation.
- `messaging.rs`: Message trait system with `MessageEnum` for all inter-service message types and RPC request/response types.
- `httpsignature.rs`: HTTP Signature creation and verification (RSA-SHA256, Ed25519).
- `pki.rs`: Key generation, trust levels (`Unverified`, `DomainVerified`, `MasterSigned`, `InstanceActor`), fingerprinting.
- `client.rs`: `ActivityPubClient` for fetching remote actors/objects and sending to inboxes.
- `lib.rs`: Core ActivityPub/ActivityStreams types (`Object`, `Activity`, `Actor`, `Collection`, enums for object/activity types).

## Conventions

- **Rust edition 2024** (nightly features may be used)
- **Error handling**: Use `thiserror` for error types; use `miette` diagnostic patterns in user-facing CLI output (oxiadm) to tell the user what to do
- **Commits**: Conventional commits format — `feat(scope):`, `fix(scope):`, `chore:`, etc.
- **Branch naming**: `feature/`, `fix/`, `docs/`, `refactor/`, `test/` prefixes
- **Workspace dependencies**: Shared dependency versions are declared in root `Cargo.toml` `[workspace.dependencies]` — use `{ workspace = true }` in member crates
- **Releases**: Managed via `cargo-release` with `git-cliff` for changelogs. Version is workspace-wide in `[workspace.package]`.
- **Docker images**: Multi-stage builds in `docker/*/Dockerfile`, published to GHCR. Use current Rust nightly, not old versions.

## Environment Variables

| Variable | Default | Used by |
|---|---|---|
| `MONGODB_URI` | `mongodb://root:password@localhost:27017` | domainservd, publisherd, operator |
| `MONGODB_DBNAME` | `domainservd` | domainservd, publisherd, operator |
| `AMQP_URI` / `AMQP_URL` | `amqp://guest:guest@localhost:5672` | all services |
| `BIND_ADDRESS` | `0.0.0.0:8080` | domainservd |
| `RUST_LOG` | `info` | all services |
| `PUBLISHER_WORKERS` | `4` | publisherd |
| `PUBLISHER_RETRY_ATTEMPTS` | `3` | publisherd |
| `PUBLISHER_RETRY_DELAY_MS` | `1000` | publisherd |
