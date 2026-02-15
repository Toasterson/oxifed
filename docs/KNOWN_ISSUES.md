# Known Issues

Honest accounting of known limitations and incomplete implementations in Oxifed. This document is intended to help contributors understand the current state of the codebase.

## Mock / Placeholder Implementations

### PKI Key Generation Returns Mock Keys

`src/pki.rs` -- `KeyPair::generate_rsa()` and `KeyPair::generate_ed25519()` return mock PEM strings (e.g., `MOCK_RSA_PUBLIC_KEY_2048`) instead of real cryptographic keys. The `sign()` method returns a SHA256 hash instead of a real signature.

**Impact:** Any component that calls key generation (including oxifed-operator storing keys in K8s Secrets) gets non-functional key material.

### HTTP Signature Verification is a Placeholder

`crates/domainservd/src/activitypub.rs` -- `verify_http_signature()` always returns `Ok(())`. All incoming S2S requests are accepted without signature verification.

**Impact:** domainservd accepts activities from any source without authentication. This is a security gap that must be fixed before production use.

### OAuth Endpoints are Stubs

`/oauth/authorize`, `/oauth/token`, `/oauth/revoke` endpoints exist in the router but do not implement OAuth flows.

## `todo!()` Panics

`crates/domainservd/src/rabbitmq.rs` contains four `todo!()` panics in message handlers:

- `handle_like()` -- Like activity handling via AMQP
- `handle_announce()` -- Announce activity handling via AMQP
- `handle_accept()` -- Accept activity handling via AMQP
- `handle_reject()` -- Reject activity handling via AMQP

These will panic at runtime if the corresponding messages are received through the internal AMQP exchange. Note: Like/Announce/Accept/Reject are handled in the HTTP inbox handler -- these `todo!()` panics affect the AMQP consumer path only.

## Stub CLI Commands (oxiadm)

The following oxiadm command groups print informational messages but do not perform operations:

- `keys import`, `keys verify`, `keys verify-complete`, `keys rotate`, `keys trust-chain`, `keys list`
- `pki` (all subcommands: init-master, backup-master, generate-domain-key, sign-domain-key, list-domains, recover-master, recover-user)
- `system` (all subcommands: health, pki-status, report)
- `test` (all subcommands: signatures, federation, authorized-fetch)

## Behavioral Issues

### Follow Auto-Accept

Follow requests are auto-accepted without checking actor preferences (`manually_approves_followers`).

### Incomplete Pagination

Several collection endpoints (followers, following, outbox) return incomplete or unpaginated results for large collections.

### Media Upload Stub

`POST /users/{username}/media` is routed but does not process uploads.

## Operator Mock Keys

`crates/oxifed-operator/` generates key material using the mock PKI functions and stores it in Kubernetes Secrets. The stored keys are not real cryptographic keys.
