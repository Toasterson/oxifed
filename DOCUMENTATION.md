# Oxifed Documentation

Oxifed is a multi-domain ActivityPub federation server written in Rust. It uses MongoDB for storage and LavinMQ/RabbitMQ for inter-service messaging. The project is **pre-alpha and experimental** -- it is not production-ready. It started as an AI coding experiment and is under active development.

## Documentation Map

- **[README.md](README.md)** -- Project overview and quick start
- **[docs/ARCHITECTURE_DESIGN.md](docs/ARCHITECTURE_DESIGN.md)** -- Technical architecture (design document, not all features implemented)
- **[docs/DESIGN.md](docs/DESIGN.md)** -- Platform design goals and intended feature set
- **[docs/API_REFERENCE.md](docs/API_REFERENCE.md)** -- HTTP endpoints exposed by domainservd
- **[docs/HTTP_SIGNATURES.md](docs/HTTP_SIGNATURES.md)** -- HTTP signature implementation (RFC 9421)
- **[docs/KUBERNETES.md](docs/KUBERNETES.md)** -- Kubernetes deployment with FluxCD
- **[docs/CI_CD.md](docs/CI_CD.md)** -- CI/CD pipeline documentation
- **[docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md)** -- Known issues and limitations
- **[crates/oxiadm/README.md](crates/oxiadm/README.md)** -- Administration CLI tool
- **[e2e/README.md](e2e/README.md)** -- End-to-end federation tests

## Project Status

Pre-alpha. Experimental. Not production-ready.

### What Works

- domainservd serves ActivityPub endpoints: actor profiles, inboxes, outboxes, collections, shared inbox
- WebFinger discovery (RFC 7033) and NodeInfo 2.0
- publisherd delivers activities to remote inboxes with HTTP signatures (RFC 9421) and configurable retry logic
- oxiadm CLI manages domains, users, profiles, notes, and social interactions via AMQP messaging
- oxifed-operator manages Domain CRDs in Kubernetes and syncs to MongoDB
- Oxifed-to-Oxifed federation across multiple domains
- Cross-domain follow, like, announce, undo workflows
- RPC-based domain and user queries

### What Does Not Work Yet

- HTTP signature verification on incoming requests (placeholder that accepts all requests)
- PKI key generation returns mock PEM strings, not real cryptographic keys
- OAuth endpoints (`/oauth/authorize`, `/oauth/token`, `/oauth/revoke`) are stubs
- Media upload endpoint (`/users/{username}/media`) is a stub
- No metrics, tracing, or monitoring infrastructure
- oxiadm `keys import/verify/verify-complete/rotate/trust-chain/list` commands are stubs
- oxiadm `pki`, `system`, and `test` command groups are stubs
- Cavage-12 HTTP signature compatibility is not implemented
- No web interface or frontend application
- Pagination is incomplete in several collection endpoints

## Federation Compatibility

| Implementation | Status | Notes |
|----------------|--------|-------|
| Oxifed | E2E test suite in CI | 3-instance federation tests |
| snac2 | E2E test suite in CI | Interop tests in `e2e/` |
| Mitra | E2E test suite in CI | Interop tests in `e2e/` |
| Mastodon | Untested | Expected to work for basic ActivityPub operations but not verified |
| Pleroma/Akkoma | Untested | Expected to work for basic ActivityPub operations but not verified |
| GoToSocial | Untested | Expected to work for basic ActivityPub operations but not verified |
| Misskey/Calckey | Untested | Expected to work for basic ActivityPub operations but not verified |
| PeerTube | Untested | Expected to work for basic ActivityPub operations but not verified |
| Pixelfed | Untested | Expected to work for basic ActivityPub operations but not verified |

## Standards

| Standard | Status |
|----------|--------|
| RFC 9421 (HTTP Message Signatures) | Library implemented (`src/httpsignature.rs`). Used by publisherd for outgoing requests. Not wired into domainservd for incoming request verification. |
| RFC 7033 (WebFinger) | Implemented in domainservd |
| W3C ActivityPub | Partial -- server-to-server endpoints implemented, client-to-server is minimal |
| NodeInfo 2.0 | Implemented in domainservd |
| Cavage-12 (HTTP Signatures draft) | Not implemented. Planned. |

## Reference Materials

- [ActivityPub W3C Recommendation](https://www.w3.org/TR/activitypub/)
- [HTTP Signatures RFC 9421](https://datatracker.ietf.org/doc/html/rfc9421)
- [ActivityPub HTTP Signature Profile](https://swicg.github.io/activitypub-http-signature/)
- [RFC 7033 WebFinger](https://datatracker.ietf.org/doc/html/rfc7033)
