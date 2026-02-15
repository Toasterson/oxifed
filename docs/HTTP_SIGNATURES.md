# HTTP Signatures in Oxifed

Oxifed implements RFC 9421 HTTP Message Signatures. The implementation lives in `src/httpsignature.rs`.

> **Important:** domainservd does not currently use this module for incoming request verification. The verification function in domainservd is a placeholder that accepts all requests (`Ok(())`). Signature signing is used by publisherd for outgoing activity delivery.

> **Cavage-12 compatibility is planned but not yet implemented.** The "double-knocking" fallback strategy described in some ActivityPub documentation is not coded.

## Standards

- **Implemented**: [RFC 9421 HTTP Message Signatures](https://datatracker.ietf.org/doc/html/rfc9421)
- **Reference**: [ActivityPub HTTP Signature Profile](https://swicg.github.io/activitypub-http-signature/)
- **Planned**: [Cavage-12 Draft](https://datatracker.ietf.org/doc/html/draft-cavage-http-signatures-12) (not yet implemented)

## Supported Algorithms

The `SignatureAlgorithm` enum defines four algorithms:

| Variant | RFC 9421 identifier | Description |
|---------|---------------------|-------------|
| `RsaSha256` | `rsa-v1_5-sha256` | RSASSA-PKCS1-v1_5 using SHA-256 |
| `RsaPssSha512` | `rsa-pss-sha512` | RSASSA-PSS using SHA-512 |
| `EcdsaP256Sha256` | `ecdsa-p256-sha256` | ECDSA using curve P-256 and SHA-256 |
| `Ed25519` | `ed25519` | EdDSA using curve Ed25519 |

## Core Types

### `HttpSignature`

The main struct. All methods are associated functions (no instance state).

### `SignatureConfig`

Configuration for signing a request:

```rust
pub struct SignatureConfig {
    pub algorithm: SignatureAlgorithm,
    pub parameters: SignatureParameters,
    pub key_id: String,
    pub components: Vec<ComponentIdentifier>,
    pub private_key: Vec<u8>,
}
```

### `VerificationConfig`

Configuration for verifying a signed request:

```rust
pub struct VerificationConfig {
    pub public_key: Vec<u8>,
    pub algorithm: SignatureAlgorithm,
    pub max_age: Option<i64>,
    pub required_components: Option<Vec<ComponentIdentifier>>,
    pub expected_key_id: Option<String>,
}
```

Builder methods: `new()`, `with_max_age()`, `without_max_age()`, `with_required_components()`, `with_expected_key_id()`.

### `SignatureParameters`

```rust
pub struct SignatureParameters {
    pub created: Option<DateTime<Utc>>,
    pub expires: Option<DateTime<Utc>>,
    pub nonce: Option<String>,
    pub key_id: Option<String>,
    pub tag: Option<String>,
    pub algorithm: Option<SignatureAlgorithm>,
}
```

### `ComponentIdentifier`

Specifies which parts of the HTTP message are included in the signature base:

```rust
pub enum ComponentIdentifier {
    Header(String),
    Method,
    TargetUri,
    RequestTarget,
    Path,
    Query,
    Status,
    Digest,
}
```

## Functions

### `HttpSignature::sign_request(req, config) -> Result<(), SignatureError>`

Signs an outgoing HTTP request by:
1. Building a signature base from the specified components
2. Signing the base with the private key using the configured algorithm
3. Adding `Signature` and `Signature-Input` headers to the request

### `HttpSignature::verify_request(req, config) -> Result<(), SignatureError>`

Verifies an incoming HTTP request by:
1. Extracting `Signature` and `Signature-Input` headers
2. Reconstructing the signature base from the specified components
3. Verifying the signature against the public key

### `HttpSignature::create_signature_base(req, components, params) -> Result<String, SignatureError>`

Builds the signature base string from request components and parameters according to RFC 9421.

## How publisherd Uses Signatures

When delivering an activity to a remote inbox, publisherd:
1. Constructs the HTTP POST request with the activity JSON body
2. Calls `HttpSignature::sign_request()` with the actor's key
3. Sends the signed request to the remote inbox

## Error Types

`SignatureError` covers: invalid parameters, missing parameters, unsupported algorithms, invalid key format, verification failure, expiry, crypto errors, base64 errors, invalid headers, and missing signatures.

## PKI Integration

The `src/pki.rs` module defines trust levels used alongside signatures:

- `Unverified` -- self-signed user key without domain verification
- `DomainVerified` -- user key signed by domain authority
- `MasterSigned` -- domain key signed by master key
- `InstanceActor` -- server-level system key

Note: PKI key generation currently returns mock PEM strings. The trust hierarchy types are defined but the full verification chain is not implemented.
