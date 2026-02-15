# HTTP Signatures in Oxifed

This document provides a comprehensive guide to HTTP signature implementation in Oxifed, following ActivityPub best practices and ensuring interoperability with the broader fediverse.

## Overview

HTTP signatures provide cryptographic authentication for ActivityPub server-to-server requests. Oxifed implements a robust signature system that prioritizes compatibility while providing strong security guarantees through our hierarchical PKI system.

## Standards Compliance

Oxifed follows these HTTP signature specifications:

- **Primary**: [ActivityPub HTTP Signature Profile](https://swicg.github.io/activitypub-http-signature/)
- **Compatibility**: [Cavage-12 Draft](https://datatracker.ietf.org/doc/html/draft-cavage-http-signatures-12)
- **Modern Support**: [RFC 9421](https://datatracker.ietf.org/doc/html/rfc9421) where supported
- **Security**: [W3C Security Vocabulary](https://w3c.github.io/vc-data-integrity/vocab/security/vocabulary.html)

## Signature Generation

### Outgoing Requests

When making ActivityPub requests to remote servers, Oxifed generates HTTP signatures as follows:

#### 1. Header Selection

```
GET requests:  (request-target) host date
POST requests: (request-target) host date digest
```

#### 2. Digest Generation (POST only)

```
SHA-256 hash of request body → Base64 encode → Prefix with "SHA-256="
```

Example:
```
Content: {"type": "Create", "actor": "https://example.com/users/alice"}
SHA-256:  f4d6c8...
Base64:   9NbI...
Digest:   SHA-256=9NbI...
```

#### 3. Signing String Construction

```
(request-target): post /users/bob/inbox
host: remote.example.com
date: Wed, 18 Dec 2024 10:08:46 GMT
digest: SHA-256=9NbI...
```

#### 4. Signature Header Format

```
Signature: keyId="https://example.com/users/alice#main-key",algorithm="hs2019",headers="(request-target) host date digest",signature="base64signature"
```

### Key Selection for Signing

Oxifed uses different keys based on the request context:

- **User Activities**: User's private key (domain-verified preferred)
- **System Operations**: Instance actor key
- **Anonymous Fetches**: Domain key for authorized fetch compliance

## Signature Verification

### Incoming Request Validation

#### 1. Signature Header Parsing

Extract components from the `Signature` header:
- `keyId`: URL pointing to the public key
- `algorithm`: Signature algorithm (typically "hs2019")
- `headers`: List of signed headers
- `signature`: Base64-encoded signature

#### 2. Public Key Retrieval

```rust
async fn get_public_key(key_id: &str) -> Result<CachedKey, SignatureError> {
    // 1. Check local cache first
    if let Some(cached) = self.key_cache.get(key_id) {
        if !cached.is_expired() {
            return Ok(cached);
        }
    }
    
    // 2. Fetch from remote server
    let key_object = self.fetch_remote_key(key_id).await?;
    
    // 3. Validate trust chain
    let trust_level = self.validate_trust_chain(&key_object).await?;
    
    // 4. Cache with appropriate TTL
    let cached_key = CachedKey {
        public_key: key_object.public_key,
        trust_level,
        cached_at: Utc::now(),
        ttl: self.get_cache_ttl(&trust_level),
    };
    
    self.key_cache.insert(key_id.to_string(), cached_key.clone());
    Ok(cached_key)
}
```

#### 3. Key Discovery Process

For `keyId` resolution, Oxifed follows this process:

1. **Fragment KeyId** (`actor#main-key`):
   - Fetch actor document
   - Extract `publicKey` property matching the fragment

2. **Standalone KeyId** (`/keys/key-uuid`):
   - Fetch key object directly
   - Verify `owner` or `controller` property
   - Confirm bidirectional reference with actor

3. **Trust Verification**:
   - Check for domain signature (if user key)
   - Validate signature chain to master key
   - Assign appropriate trust level

#### 4. Signature Verification with Double-Knocking

```rust
async fn verify_signature_with_fallback(
    &self,
    key_info: &CachedKey,
    signing_string: &str,
    signature_header: &SignatureHeader
) -> Result<(), SignatureError> {
    // Try cavage-12 with hs2019 first (most compatible)
    if signature_header.algorithm == "hs2019" {
        if self.verify_cavage12(key_info, signing_string, &signature_header.signature).is_ok() {
            return Ok(());
        }
    }
    
    // Try RFC 9421 if hs2019 fails
    if signature_header.has_signature_input_header() {
        if self.verify_rfc9421(key_info, signing_string, &signature_header.signature).is_ok() {
            return Ok(());
        }
    }
    
    // Try explicit algorithm
    match signature_header.algorithm.as_str() {
        "rsa-sha256" => self.verify_rsa_sha256(key_info, signing_string, &signature_header.signature),
        "rsa-sha512" => self.verify_rsa_sha512(key_info, signing_string, &signature_header.signature),
        "ed25519" => self.verify_ed25519(key_info, signing_string, &signature_header.signature),
        _ => Err(SignatureError::UnsupportedAlgorithm)
    }
}
```

#### 5. Timestamp Validation

```rust
fn validate_timestamp(&self, request: &HttpRequest) -> Result<(), SignatureError> {
    let date_header = request.headers().get("date")
        .ok_or(SignatureError::MissingDateHeader)?;
    
    let request_time = parse_http_date(date_header)?;
    let now = Utc::now();
    let age = (now - request_time).abs();
    
    // Allow 1 hour + 5 minutes buffer for clock skew
    if age > Duration::hours(1) + Duration::minutes(5) {
        return Err(SignatureError::TimestampTooOld);
    }
    
    Ok(())
}
```

## PKI Integration

### Trust Levels

Oxifed assigns trust levels to keys based on verification status:

```rust
pub enum TrustLevel {
    /// Self-signed user key without domain verification
    Unverified,
    /// Domain-signed user key (verified by domain authority)  
    DomainVerified,
    /// Master-signed domain key (root of trust)
    MasterSigned,
    /// Instance actor key (server-level authority)
    InstanceActor,
}
```

### Trust Level Impact

- **Rate Limiting**: Higher trust levels get increased rate limits
- **Content Visibility**: Verified users' content may be prioritized
- **Moderation**: Unverified keys face stricter content filtering
- **Federation**: Some servers may reject unverified signatures

### Domain Verification Process

1. **User Key Import**: User provides public key (and optionally private key)
2. **Challenge Generation**: System creates domain verification challenge
3. **Challenge Response**: User signs challenge with their private key
4. **Domain Signing**: Domain key signs user's public key upon verification
5. **Trust Upgrade**: User key trust level upgraded to DomainVerified

## Authorized Fetch Implementation

### Secure Mode Operation

When authorized fetch is enabled:

1. **GET Request Signing**: All object fetch requests require signatures
2. **Instance Actor Usage**: System uses instance actor to prevent deadlocks
3. **Access Control**: Check signature against block lists and access rules
4. **Cache Headers**: Include `Signature` in `Vary` header for cache safety

### Instance Actor Strategy

```rust
pub struct InstanceActor {
    pub actor_id: String,
    pub domain: String,
    pub public_key: PublicKey,
    pub private_key: EncryptedPrivateKey,
    pub purpose: InstanceActorPurpose,
}

pub enum InstanceActorPurpose {
    AuthorizedFetch,
    SystemOperations,
    EmergencyRecovery,
}
```

Instance actors are used for:
- Breaking signature verification deadlocks
- System-level ActivityPub operations
- Emergency key recovery procedures
- Relay and proxy operations

## Key Management

### User Key Import

Users can bring their own keys through the API:

```bash
# Import RSA key
curl -X POST https://example.com/api/v1/keys/import \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "public_key_pem": "-----BEGIN PUBLIC KEY-----\n...",
    "private_key_pem": "-----BEGIN PRIVATE KEY-----\n...",
    "algorithm": "RSA",
    "key_size": 2048
  }'

# Import Ed25519 key  
curl -X POST https://example.com/api/v1/keys/import \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "public_key_pem": "-----BEGIN PUBLIC KEY-----\n...",
    "private_key_pem": "-----BEGIN PRIVATE KEY-----\n...",
    "algorithm": "Ed25519"
  }'
```

### Key Rotation

#### Scheduled Rotation

```rust
pub async fn rotate_key_scheduled(&self, actor_id: &str) -> Result<(), RotationError> {
    // 1. Generate new keypair
    let new_keypair = self.generate_keypair().await?;
    
    // 2. Create Update activity
    let update_activity = Activity {
        id: format!("{}/activities/{}", actor_id, uuid::Uuid::new_v4()),
        activity_type: ActivityType::Update,
        actor: actor_id.to_string(),
        object: self.create_updated_actor(actor_id, &new_keypair.public_key).await?,
        published: Some(Utc::now()),
        ..Default::default()
    };
    
    // 3. Sign with old key and distribute
    self.sign_and_deliver(&update_activity, actor_id).await?;
    
    // 4. Update local storage
    self.store_new_key(actor_id, &new_keypair).await?;
    
    // 5. Schedule old key cleanup (7 days)
    self.schedule_cleanup(actor_id, Duration::days(7)).await?;
    
    Ok(())
}
```

#### Emergency Rotation

For compromised keys:

```rust
pub async fn rotate_key_emergency(&self, actor_id: &str) -> Result<(), RotationError> {
    // 1. Immediately revoke old key
    self.revoke_key(actor_id).await?;
    
    // 2. Generate new keypair
    let new_keypair = self.generate_keypair().await?;
    
    // 3. Sign with domain key (emergency authority)
    let emergency_update = self.create_emergency_update(actor_id, &new_keypair).await?;
    self.sign_with_domain_key(&emergency_update).await?;
    
    // 4. Broadcast emergency update
    self.broadcast_emergency_update(&emergency_update).await?;
    
    Ok(())
}
```

## Well-Known Endpoints

Oxifed publishes PKI information at standard endpoints:

### Master Key Endpoint

```
GET /.well-known/oxifed/master-key

Response:
{
  "keyId": "https://oxifed.example/.well-known/oxifed/master-key",
  "publicKeyPem": "-----BEGIN PUBLIC KEY-----\n...",
  "algorithm": "RSA",
  "keySize": 4096,
  "createdAt": "2024-01-01T00:00:00Z",
  "fingerprint": "sha256:abc123...",
  "usage": ["domain-signing"],
  "type": "MasterKey"
}
```

### Domain Key Endpoint

```
GET /.well-known/oxifed/domain-key

Response:
{
  "keyId": "https://example.com/.well-known/oxifed/domain-key",
  "domain": "example.com",
  "publicKeyPem": "-----BEGIN PUBLIC KEY-----\n...",
  "algorithm": "RSA", 
  "keySize": 4096,
  "createdAt": "2024-01-01T00:00:00Z",
  "expiresAt": "2025-01-01T00:00:00Z",
  "masterSignature": {
    "signature": "base64signature",
    "signedAt": "2024-01-01T00:00:00Z",
    "masterKeyId": "https://oxifed.example/.well-known/oxifed/master-key"
  },
  "fingerprint": "sha256:def456...",
  "usage": ["user-signing", "instance-actor"],
  "type": "DomainKey"
}
```

### Trust Chain Verification

```
GET /.well-known/oxifed/trust-chain?keyId=https://example.com/users/alice%23main-key

Response:
{
  "keyId": "https://example.com/users/alice#main-key",
  "trustLevel": "DomainVerified",
  "verificationChain": [
    {
      "level": "user",
      "keyId": "https://example.com/users/alice#main-key",
      "signedBy": "https://example.com/.well-known/oxifed/domain-key",
      "signedAt": "2024-01-01T00:00:00Z"
    },
    {
      "level": "domain", 
      "keyId": "https://example.com/.well-known/oxifed/domain-key",
      "signedBy": "https://oxifed.example/.well-known/oxifed/master-key",
      "signedAt": "2024-01-01T00:00:00Z"
    },
    {
      "level": "master",
      "keyId": "https://oxifed.example/.well-known/oxifed/master-key",
      "selfSigned": true,
      "createdAt": "2024-01-01T00:00:00Z"
    }
  ]
}
```

## Error Handling

### Signature Verification Errors

- `401 Unauthorized`: Invalid or missing signature
- `403 Forbidden`: Valid signature but insufficient authorization
- `429 Too Many Requests`: Rate limit exceeded
- `502 Bad Gateway`: Unable to fetch remote key

### Recovery Procedures

1. **Key Fetch Failure**: Retry with exponential backoff
2. **Signature Verification Failure**: Try alternative algorithms
3. **Timestamp Issues**: Log clock skew and adjust tolerance
4. **Cache Misses**: Implement warm-up strategies

## Performance Optimizations

### Caching Strategy

- **Key Cache**: LRU cache with trust-level based TTL
- **Signature Cache**: Short-term cache for repeated validations
- **Domain Cache**: Long-term cache for domain key information

### Cache TTL by Trust Level

```rust
fn get_cache_ttl(&self, trust_level: &TrustLevel) -> Duration {
    match trust_level {
        TrustLevel::Unverified => Duration::minutes(15),
        TrustLevel::DomainVerified => Duration::hours(4),  
        TrustLevel::MasterSigned => Duration::hours(24),
        TrustLevel::InstanceActor => Duration::hours(12),
    }
}
```

### Batch Verification

For high-throughput scenarios:

```rust
pub async fn verify_batch(&self, requests: Vec<SignedRequest>) -> Vec<VerificationResult> {
    // 1. Group by keyId to minimize fetches
    let grouped = group_by_key_id(requests);
    
    // 2. Batch fetch missing keys
    let keys = self.batch_fetch_keys(&grouped.keys()).await;
    
    // 3. Parallel verification
    let results = stream::iter(grouped)
        .map(|(key_id, requests)| {
            let key = keys.get(&key_id);
            async move {
                self.verify_requests_with_key(requests, key).await
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;
    
    results
}
```

## Security Considerations

### Attack Mitigation

- **Replay Attacks**: Timestamp validation and nonce tracking
- **Key Substitution**: Trust chain verification
- **Signature Stripping**: Mandatory signature requirements
- **Clock Skew Attacks**: Reasonable timestamp windows

### Monitoring and Alerting

- **Failed Verifications**: Alert on high failure rates
- **Key Rotation Events**: Monitor for suspicious rotations  
- **Trust Level Changes**: Track trust level modifications
- **Cache Performance**: Monitor hit rates and timing

## Testing and Validation

### Interoperability Testing

Oxifed is tested against these popular implementations:

- Mastodon 4.2+
- Pleroma/Akkoma
- GoToSocial
- PeerTube
- Misskey/Calckey
- Pixelfed

### Test Scenarios

1. **Basic Signature Verification**: Standard cavage-12 signatures
2. **Algorithm Fallback**: hs2019 → rsa-sha256 fallback
3. **Key Rotation**: Seamless key updates
4. **Emergency Recovery**: Compromise response procedures
5. **Cross-Domain Federation**: Multi-domain key management
6. **Performance Under Load**: High-throughput signature verification

This implementation ensures Oxifed can securely participate in the fediverse while providing advanced key management capabilities for users who want more control over their cryptographic identity.