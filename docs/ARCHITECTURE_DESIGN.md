# Oxifed Technical Architecture

> **This is a design document.** It describes the intended architecture of Oxifed. Not all features described here are implemented. Sections are marked `[IMPLEMENTED]`, `[PARTIAL]`, or `[PLANNED]` to indicate current status. See [DOCUMENTATION.md](../DOCUMENTATION.md) for what works today.

## System Overview [IMPLEMENTED]

```
Internet
    |
    v
+-------------------------------------------------------------+
|                    Load Balancer / Reverse Proxy              |
+------------------------------+------------------------------+
                               |
               +---------------+---------------+
               |               |               |
               v               v               v
       +---------------+ +---------------+ +---------------+
       |  domainservd  | |  domainservd  | |  domainservd  |
       |   Instance 1  | |   Instance 2  | |   Instance N  |
       +---------------+ +---------------+ +---------------+
               |               |               |
               +-------+-------+-------+-------+
                       |               |
                       v               v
              +----------------+ +----------------+
              |  Message Queue | |    MongoDB     |
              |  (LavinMQ/     | |                |
              |   RabbitMQ)    | |                |
              +-------+--------+ +----------------+
                      |
              +-------+-------+
              |               |
              v               v
       +---------------+ +---------------+
       |  publisherd   | |  publisherd   |
       |   Instance 1  | |   Instance N  |
       +---------------+ +---------------+
              |
              v
       Remote ActivityPub Servers
```

## Component Architecture

### domainservd [IMPLEMENTED]

Central Axum web server that handles HTTP requests. Binds to port 8080.

Responsibilities:
- Serves ActivityPub actor profiles, inboxes, outboxes, and collections
- WebFinger discovery (RFC 7033)
- NodeInfo 2.0 endpoint
- Shared inbox for server-to-server delivery
- Consumes AMQP messages for domain/user/profile management
- Publishes activities to AMQP for delivery by publisherd

### publisherd [IMPLEMENTED]

Worker daemon that consumes activities from the message queue and delivers them to remote inboxes.

- Configurable worker count (`PUBLISHER_WORKERS`, default: 4)
- HTTP signature signing on outgoing requests (RFC 9421)
- Retry with configurable attempts and delay (`PUBLISHER_RETRY_ATTEMPTS`, `PUBLISHER_RETRY_DELAY_MS`)
- Each worker binds to `EXCHANGE_ACTIVITYPUB_PUBLISH` with its own queue

### oxiadm [PARTIAL]

CLI administration tool. Uses AMQP messaging for commands and RPC for queries.

- Domain, user, profile, note, and activity management: working
- Keys generate: working (sends message, but PKI uses mock keys)
- Keys import/verify/rotate, PKI, system, test commands: stubs

### oxifed-operator [PARTIAL]

Kubernetes operator managing `Domain` CRDs (v1alpha1). Generates cryptographic keys, stores them in K8s Secrets, and syncs domain configuration to MongoDB. Note: key generation currently produces mock key material.

### oxifed (root crate) [IMPLEMENTED]

Shared library used by all other crates. Contains:
- ActivityPub/ActivityStreams types (`Object`, `Activity`, `Actor`, `Collection`)
- MongoDB `DatabaseManager` with collection management and index creation
- HTTP signature implementation (RFC 9421)
- PKI module (types defined, key generation is mock)
- WebFinger types
- `ActivityPubClient` for fetching remote actors and sending to inboxes
- Message types and AMQP exchange/queue constants

## Database Schema [IMPLEMENTED]

MongoDB collections:

| Collection | Description |
|------------|-------------|
| `actors` | Actor profiles. Indexed on `actor_id` (unique), `(domain, preferred_username)` (unique) |
| `objects` | ActivityPub objects. Indexed on `object_id` (unique), `(attributed_to, published)` |
| `activities` | Activity records. Indexed on `activity_id` (unique), `(actor, published)` |
| `keys` | Cryptographic keys. Indexed on `key_id` (unique), `actor_id` |
| `domains` | Domain configuration. Indexed on `domain` (unique) |
| `follows` | Follow relationships. Indexed on `(follower, following)` (unique) |
| `webfinger_profiles` | WebFinger JRD resources |

## Message Queue Architecture [IMPLEMENTED]

AMQP exchanges used for inter-service communication:

| Exchange | Type | Purpose |
|----------|------|---------|
| `oxifed.internal.publish` | fanout | Internal commands: profile/note/domain create/update/delete |
| `oxifed.activitypub.publish` | fanout | Outgoing ActivityPub activities for publisherd to deliver |
| `oxifed.incoming.process` | fanout | Incoming activities from remote servers |
| `oxifed.rpc.request` | direct | RPC requests (domain list/show, user list/show) |
| `oxifed.rpc.response` | direct | RPC responses routed by correlation ID |

Queue: `oxifed.rpc.domain` for domain RPC requests.

### Message Types

Commands (fire-and-forget via `oxifed.internal.publish`):
- `ProfileCreateMessage`, `ProfileUpdateMessage`, `ProfileDeleteMessage`
- `NoteCreateMessage`, `NoteUpdateMessage`, `NoteDeleteMessage`
- `DomainCreateMessage`, `DomainUpdateMessage`, `DomainDeleteMessage`
- `FollowActivityMessage`, `LikeActivityMessage`, `AnnounceActivityMessage`
- `AcceptActivityMessage`, `RejectActivityMessage`
- `KeyGenerateMessage`, `UserCreateMessage`

Queries (RPC via `oxifed.rpc.request`/`oxifed.rpc.response`):
- `DomainRpcRequest` / `DomainRpcResponse`
- `UserRpcRequest` / `UserRpcResponse`

## Security Architecture

### HTTP Signatures [PARTIAL]

Oxifed implements RFC 9421 HTTP Message Signatures in `src/httpsignature.rs`.

- **Signing outgoing requests**: Implemented. Used by publisherd when delivering activities.
- **Verifying incoming requests**: Not implemented. domainservd has a placeholder that accepts all requests.

Supported algorithms: `RsaSha256`, `RsaPssSha512`, `EcdsaP256Sha256`, `Ed25519`.

### PKI [PARTIAL]

Trust hierarchy types are defined (`Unverified`, `DomainVerified`, `MasterSigned`, `InstanceActor`). Key generation returns mock PEM strings. Signing returns a mock SHA256 hash.

### [PLANNED] Features

The following features are described in the design but not yet implemented:

- Cavage-12 HTTP signature compatibility and "double-knocking" fallback
- Rate limiting (per-actor, per-domain, trust-level aware)
- Prometheus metrics collection
- OpenTelemetry distributed tracing
- HSM support for master key storage
- Automated key rotation
- Emergency key recovery procedures
- Content moderation and spam detection
- Redis caching layer
