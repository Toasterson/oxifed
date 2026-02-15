# Oxifed

Multi-domain ActivityPub federation server written in Rust. Uses MongoDB for storage and LavinMQ/RabbitMQ for inter-service messaging.

**Status: Pre-alpha. Experimental. Not production-ready.**

This project started as an AI coding experiment. See [DOCUMENTATION.md](DOCUMENTATION.md) for what works and what doesn't.

## Architecture

```
oxiadm (CLI) --[AMQP]--> domainservd (HTTP + message consumer)
                              |
                         [AMQP publish]
                              |
                         publisherd (delivery workers) --[HTTP+signatures]--> remote inboxes
```

All services share MongoDB. LavinMQ/RabbitMQ handles async messaging.

| Crate | Description |
|-------|-------------|
| `oxifed` (`src/`) | Shared library: ActivityPub types, database manager, HTTP signatures (RFC 9421), PKI, WebFinger, ActivityPub client |
| `domainservd` (`crates/domainservd/`) | Axum web server exposing ActivityPub, WebFinger, and NodeInfo endpoints. Port 8080. |
| `publisherd` (`crates/publisherd/`) | Worker daemon delivering activities to remote inboxes with HTTP signatures and retry logic |
| `oxiadm` (`crates/oxiadm/`) | CLI for domain, user, profile, note, and activity management via AMQP |
| `oxifed-operator` (`crates/oxifed-operator/`) | Kubernetes operator for Domain CRDs (v1alpha1) |

## Running

### Prerequisites

- Rust nightly (edition 2024)
- MongoDB 6.0+
- LavinMQ or RabbitMQ

### Quick Start

```bash
# Start infrastructure
docker-compose up -d mongodb lavinmq

# Build
cargo build

# Run the server
cargo run -p domainservd

# In another terminal, create a domain
cargo run -p oxiadm -- domain create example.com --name "Example"

# Create a user profile
cargo run -p oxiadm -- person create alice@example.com

# Start the publisher daemon
cargo run -p publisherd
```

### Environment Variables

| Variable | Default | Used by |
|----------|---------|---------|
| `MONGODB_URI` | `mongodb://root:password@localhost:27017` | domainservd, publisherd, operator |
| `MONGODB_DBNAME` | `domainservd` | domainservd, publisherd, operator |
| `AMQP_URI` / `AMQP_URL` | `amqp://guest:guest@localhost:5672` | all services |
| `BIND_ADDRESS` | `0.0.0.0:8080` | domainservd |
| `RUST_LOG` | `info` | all services |
| `PUBLISHER_WORKERS` | `4` | publisherd |
| `PUBLISHER_RETRY_ATTEMPTS` | `3` | publisherd |
| `PUBLISHER_RETRY_DELAY_MS` | `1000` | publisherd |

## Testing

```bash
# All tests
cargo test --all-features --workspace

# Formatting (CI enforces)
cargo fmt --all -- --check

# Linting (CI enforces -D warnings)
cargo clippy --all-targets --all-features -- -D warnings
```

End-to-end federation tests (Oxifed-to-Oxifed and interop with snac2/Mitra) are in the [e2e/](e2e/) directory.

## Contributing

Contributions welcome, both AI-assisted and manual. This project is an experiment in AI-assisted development. See the AI experiment context in the original README below.

### Original Context

> With the feedback from my followers I decided to put this file here manually. This is a Experiment. A very out there couple of hours on the side running AI and seeing how it goes Experiment.
>
> If others want to join and run AI against it as well I would be happy to make this a multiplayer experiment.

## License

See [LICENSE](LICENSE) for details.
