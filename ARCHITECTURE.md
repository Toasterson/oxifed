# Oxifed Technical Architecture

## System Overview

```
Internet
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│                    Load Balancer / Reverse Proxy            │
│                         (Nginx/HAProxy)                     │
└─────────────────────────┬───────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   domainservd   │ │   domainservd   │ │   domainservd   │
│    Instance 1   │ │    Instance 2   │ │    Instance 3   │
└─────────────────┘ └─────────────────┘ └─────────────────┘
          │               │               │
          └───────────────┼───────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Message Queue                            │
│                     (RabbitMQ)                              │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │ Internal Queue  │  │ ActivityPub     │  │ Incoming     │ │
│  │                 │  │ Publish Queue   │  │ Exchange     │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   publisherd    │ │   publisherd    │ │ Worker Daemons  │
│    Instance 1   │ │    Instance 2   │ │                 │
└─────────────────┘ └─────────────────┘ └─────────────────┘
          │               │               │
          └───────────────┼───────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                     Database Cluster                        │
│                       (MongoDB)                             │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │ Primary Replica │  │Secondary Replica│  │Arbiter Node  │ │
│  │                 │  │                 │  │              │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Component Architecture

### domainservd - Domain Service Daemon

```
┌─────────────────────────────────────────────────────────────┐
│                      domainservd                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   HTTP Server   │  │  WebFinger      │  │   Actor      │ │
│  │                 │  │  Handler        │  │  Manager     │ │
│  │ • Inbox API     │  │                 │  │              │ │
│  │ • Outbox API    │  │ • Discovery     │  │ • CRUD Ops   │ │
│  │ • Actor API     │  │ • Verification  │  │ • Auth       │ │
│  │ • Health Check  │  │                 │  │ • Metadata   │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
│           │                     │                 │         │
│           └─────────────────────┼─────────────────┘         │
│                                 │                           │
│  ┌─────────────────┐  ┌─────────▼─────────┐  ┌──────────────┐ │
│  │   Message       │  │   HTTP Signature  │  │   Domain     │ │
│  │   Publisher     │  │   Verification    │  │   Router     │ │
│  │                 │  │                   │  │              │ │
│  │ • Queue Publish │  │ • Key Management  │  │ • Multi-     │ │
│  │ • Routing       │  │ • Signature Check │  │   Domain     │ │
│  │ • Batching      │  │ • Auth Context    │  │ • Config     │ │
│  └─────────────────┘  └───────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### publisherd - Publishing Service Daemon

```
┌─────────────────────────────────────────────────────────────┐
│                      publisherd                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   Message       │  │   Activity      │  │   Federation │ │
│  │   Consumer      │  │   Processor     │  │   Manager    │ │
│  │                 │  │                 │  │              │ │
│  │ • Queue Listen  │  │ • Activity      │  │ • Remote     │ │
│  │ • Deserialization│  │   Validation    │  │   Discovery  │ │
│  │ • Error Handling│  │ • State Machine │  │ • Protocol   │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
│           │                     │                 │         │
│           └─────────────────────┼─────────────────┘         │
│                                 │                           │
│  ┌─────────────────┐  ┌─────────▼─────────┐  ┌──────────────┐ │
│  │   Delivery      │  │   Retry Engine    │  │   Analytics  │ │
│  │   Engine        │  │                   │  │   Collector  │ │
│  │                 │  │ • Exponential     │  │              │ │
│  │ • HTTP Client   │  │   Backoff         │  │ • Metrics    │ │
│  │ • Batch Send    │  │ • Dead Letter     │  │ • Logging    │ │
│  │ • Rate Limiting │  │ • Success Track   │  │ • Tracing    │ │
│  └─────────────────┘  └───────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow Architecture

### Incoming Activity Processing

```
External ActivityPub Server
            │
            ▼
┌─────────────────────────┐
│     Load Balancer       │
└─────────────────────────┘
            │
            ▼
┌─────────────────────────┐
│     domainservd         │
│   ┌─────────────────┐   │
│   │   Inbox API     │   │
│   └─────────────────┘   │
└─────────────────────────┘
            │
            ▼
┌─────────────────────────┐
│  HTTP Signature         │
│  Verification           │
└─────────────────────────┘
            │
            ▼
┌─────────────────────────┐
│   Message Queue         │
│   INCOMING_EXCHANGE     │
└─────────────────────────┘
            │
      ┌─────┴─────┐
      │           │
      ▼           ▼
┌──────────┐ ┌──────────┐
│ Filter   │ │ Process  │
│ Worker   │ │ Worker   │
└──────────┘ └──────────┘
      │           │
      └─────┬─────┘
            ▼
┌─────────────────────────┐
│     Database            │
│     (MongoDB)           │
└─────────────────────────┘
```

### Outgoing Activity Publishing

```
Application Layer
    │
    ▼
┌─────────────────────────┐
│   domainservd API       │
└─────────────────────────┘
    │
    ▼
┌─────────────────────────┐
│   Activity Creation     │
│   & Validation          │
└─────────────────────────┘
    │
    ▼
┌─────────────────────────┐
│   Message Queue         │
│   ACTIVITYPUB_PUBLISH   │
└─────────────────────────┘
    │
    ▼
┌─────────────────────────┐
│     publisherd          │
│   ┌─────────────────┐   │
│   │   Processor     │   │
│   └─────────────────┘   │
└─────────────────────────┘
    │
    ▼
┌─────────────────────────┐
│   Delivery Engine       │
│   • Target Discovery    │
│   • HTTP Signature      │
│   • Retry Logic         │
└─────────────────────────┘
    │
    ▼
External ActivityPub Servers
```

## Database Schema Design

### Actor Collection

```javascript
{
  _id: ObjectId("507f1f77bcf86cd799439011"),
  domain: "example.com",
  username: "alice",
  display_name: "Alice Johnson",
  summary: "Software engineer passionate about federated networks",
  
  // ActivityPub URLs
  ap_id: "https://example.com/users/alice",
  inbox_url: "https://example.com/users/alice/inbox",
  outbox_url: "https://example.com/users/alice/outbox",
  followers_url: "https://example.com/users/alice/followers",
  following_url: "https://example.com/users/alice/following",
  featured_url: "https://example.com/users/alice/collections/featured",
  
  // Cryptographic keys
  public_key: {
    id: "https://example.com/users/alice#main-key",
    owner: "https://example.com/users/alice",
    public_key_pem: "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG..."
  },
  private_key_encrypted: "encrypted_private_key_data",
  
  // Profile data
  icon: {
    type: "Image",
    url: "https://example.com/media/avatars/alice.jpg",
    media_type: "image/jpeg"
  },
  header: {
    type: "Image", 
    url: "https://example.com/media/headers/alice.jpg",
    media_type: "image/jpeg"
  },
  
  // Metadata
  actor_type: "Person", // Person, Service, Application, Group, Organization
  preferred_username: "alice",
  manually_approves_followers: false,
  discoverable: true,
  indexable: true,
  
  // Attachment fields
  attachment: [
    {
      type: "PropertyValue",
      name: "Website",
      value: "https://alice.example.com"
    },
    {
      type: "PropertyValue", 
      name: "Location",
      value: "San Francisco, CA"
    }
  ],
  
  // System fields
  created_at: ISODate("2023-01-01T00:00:00.000Z"),
  updated_at: ISODate("2023-12-01T12:34:56.789Z"),
  last_activity_at: ISODate("2023-12-01T12:34:56.789Z"),
  
  // Configuration
  settings: {
    privacy: {
      require_follow_approval: false,
      hide_followers: false,
      hide_following: false
    },
    notifications: {
      email_enabled: true,
      push_enabled: false
    }
  }
}
```

### Objects Collection

```javascript
{
  _id: ObjectId("507f1f77bcf86cd799439012"),
  
  // ActivityPub identification
  ap_id: "https://example.com/notes/123",
  ap_type: "Note",
  
  // Content
  content: "Hello, federated world! 🌍",
  content_html: "<p>Hello, federated world! 🌍</p>",
  summary: null,
  sensitive: false,
  
  // Authorship
  attributed_to: "https://example.com/users/alice",
  attributed_to_actor_id: ObjectId("507f1f77bcf86cd799439011"),
  
  // Timestamps
  published: ISODate("2023-12-01T12:34:56.789Z"),
  updated: null,
  
  // Threading
  in_reply_to: null,
  context: "https://example.com/contexts/conversation-456",
  conversation: "https://example.com/contexts/conversation-456",
  
  // Audience
  to: ["https://www.w3.org/ns/activitystreams#Public"],
  cc: ["https://example.com/users/alice/followers"],
  bto: [],
  bcc: [],
  
  // Attachments and media
  attachment: [
    {
      type: "Document",
      media_type: "image/jpeg",
      url: "https://example.com/media/uploads/photo123.jpg",
      name: "A beautiful sunset",
      width: 1920,
      height: 1080,
      blurhash: "LEHV6nWB2yk8pyo0adR*.7kCMdnj"
    }
  ],
  
  // Tags and mentions
  tag: [
    {
      type: "Hashtag",
      href: "https://example.com/tags/federation",
      name: "#federation"
    },
    {
      type: "Mention",
      href: "https://remote.example/users/bob",
      name: "@bob@remote.example"
    }
  ],
  
  // Content metadata
  language: "en",
  content_warning: null,
  
  // Engagement metrics (local only)
  local_metrics: {
    replies_count: 3,
    reblogs_count: 12,
    favourites_count: 25,
    last_engagement_at: ISODate("2023-12-01T15:22:10.123Z")
  },
  
  // System fields
  created_at: ISODate("2023-12-01T12:34:56.789Z"),
  updated_at: ISODate("2023-12-01T12:34:56.789Z"),
  deleted_at: null,
  
  // Processing status
  federated: true,
  local: true,
  processing_state: "completed"
}
```

### Activities Collection

```javascript
{
  _id: ObjectId("507f1f77bcf86cd799439013"),
  
  // ActivityPub identification
  ap_id: "https://example.com/activities/create-note-123",
  ap_type: "Create",
  
  // Activity components
  actor: "https://example.com/users/alice",
  actor_id: ObjectId("507f1f77bcf86cd799439011"),
  object: "https://example.com/notes/123",
  object_id: ObjectId("507f1f77bcf86cd799439012"),
  target: null,
  
  // Timestamps
  published: ISODate("2023-12-01T12:34:56.789Z"),
  
  // Audience (inherited from object if not specified)
  to: ["https://www.w3.org/ns/activitystreams#Public"],
  cc: ["https://example.com/users/alice/followers"],
  
  // Delivery tracking
  delivery_status: {
    total_recipients: 150,
    successful_deliveries: 147,
    failed_deliveries: 3,
    pending_deliveries: 0,
    last_delivery_attempt: ISODate("2023-12-01T12:45:30.456Z")
  },
  
  delivery_log: [
    {
      target_inbox: "https://remote1.example/users/bob/inbox",
      status: "success",
      attempts: 1,
      last_attempt: ISODate("2023-12-01T12:35:15.123Z"),
      response_code: 202
    },
    {
      target_inbox: "https://remote2.example/shared/inbox", 
      status: "failed",
      attempts: 3,
      last_attempt: ISODate("2023-12-01T12:45:30.456Z"),
      response_code: 500,
      error: "Internal server error"
    }
  ],
  
  // System fields
  created_at: ISODate("2023-12-01T12:34:56.789Z"),
  updated_at: ISODate("2023-12-01T12:45:30.456Z"),
  
  // Processing metadata
  local: true,
  processing_state: "delivered"
}
```

## Message Queue Architecture

### Exchange Configuration

```
┌─────────────────────────────────────────────────────────────┐
│                    RabbitMQ Exchanges                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  oxifed.internal.publish (topic)                           │
│  ├── routing keys:                                          │
│  │   ├── profile.create                                     │
│  │   ├── profile.update                                     │
│  │   ├── profile.delete                                     │
│  │   ├── note.create                                        │
│  │   ├── note.update                                        │
│  │   ├── note.delete                                        │
│  │   └── activity.*                                         │
│  │                                                          │
│  oxifed.activitypub.publish (topic)                        │
│  ├── routing keys:                                          │
│  │   ├── activity.create                                    │
│  │   ├── activity.update                                    │
│  │   ├── activity.delete                                    │
│  │   ├── activity.follow                                    │
│  │   ├── activity.like                                      │
│  │   ├── activity.announce                                  │
│  │   ├── activity.accept                                    │
│  │   └── activity.reject                                    │
│  │                                                          │
│  oxifed.incoming (fanout)                                  │
│  ├── queues:                                               │
│  │   ├── incoming.filter                                    │
│  │   ├── incoming.moderation                               │
│  │   ├── incoming.process                                   │
│  │   └── incoming.analytics                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Message Flow Patterns

#### Internal Message Publishing

```rust
// Message structure
#[derive(Serialize, Deserialize)]
pub struct InternalMessage {
    pub message_id: String,
    pub timestamp: DateTime<Utc>,
    pub source_service: String,
    pub domain: String,
    pub payload: MessagePayload,
    pub metadata: HashMap<String, Value>,
}

// Publishing pattern
async fn publish_internal_message(
    channel: &Channel,
    routing_key: &str,
    message: &InternalMessage
) -> Result<(), Error> {
    let payload = serde_json::to_vec(message)?;
    
    channel.basic_publish(
        "oxifed.internal.publish",
        routing_key,
        BasicPublishOptions::default(),
        &payload,
        BasicProperties::default()
            .with_content_type("application/json".into())
            .with_delivery_mode(2) // Persistent
            .with_timestamp(Utc::now().timestamp() as u64)
    ).await?;
    
    Ok(())
}
```

## Security Architecture

### HTTP Signature Implementation

Oxifed implements HTTP signatures following the ActivityPub HTTP Signature specification and RFC 9421, with backward compatibility for the widely-used cavage-12 draft. The implementation prioritizes interoperability with existing ActivityPub servers while providing robust security.

#### Signature Profile

Oxifed follows these requirements:
- **Algorithm Support**: RSA-SHA256 (primary), RSA-SHA512, Ed25519 (when supported)
- **Key Size**: Minimum 2048-bit RSA keys, Ed25519 recommended for new actors
- **Headers**: `(request-target)`, `host`, `date`, `digest` (for POST requests)
- **Signature Format**: Cavage-12 with `hs2019` algorithm placeholder for compatibility

#### Core Implementation

```rust
pub struct HttpSignatureValidator {
    pub key_cache: Arc<RwLock<HashMap<String, CachedKey>>>,
    pub signature_cache: Arc<RwLock<LruCache<String, bool>>>,
    pub pki_validator: Arc<PkiValidator>,
}

#[derive(Clone)]
pub struct CachedKey {
    pub public_key: PublicKey,
    pub actor_id: String,
    pub key_id: String,
    pub algorithm: SignatureAlgorithm,
    pub cached_at: DateTime<Utc>,
    pub ttl: Duration,
    pub trust_level: TrustLevel,
}

#[derive(Clone, Debug)]
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

impl HttpSignatureValidator {
    pub async fn validate_signature(
        &self,
        request: &HttpRequest,
        body: &[u8]
    ) -> Result<ValidationResult, SignatureError> {
        // 1. Parse signature header
        let signature_header = self.parse_signature_header(request)?;
        
        // 2. Retrieve and validate key chain
        let key_info = self.get_verified_key(&signature_header.key_id).await?;
        
        // 3. Check timestamp freshness (within 1 hour + 5 minutes buffer)
        self.validate_timestamp(request, &signature_header)?;
        
        // 4. Reconstruct signing string
        let signing_string = self.build_signing_string(request, body, &signature_header.headers)?;
        
        // 5. Verify signature with double-knocking for compatibility
        self.verify_signature_with_fallback(&key_info, &signing_string, &signature_header).await?;
        
        Ok(ValidationResult {
            actor_id: key_info.actor_id,
            key_id: key_info.key_id,
            trust_level: key_info.trust_level,
            algorithm: key_info.algorithm,
        })
    }
    
    /// Implements "double-knocking" for version compatibility
    async fn verify_signature_with_fallback(
        &self,
        key_info: &CachedKey,
        signing_string: &str,
        signature_header: &SignatureHeader
    ) -> Result<(), SignatureError> {
        // Try cavage-12 with hs2019 first (most compatible)
        if signature_header.algorithm == "hs2019" {
            if let Ok(()) = self.verify_cavage12(&key_info.public_key, signing_string, &signature_header.signature) {
                return Ok(());
            }
        }
        
        // Try RFC 9421 if hs2019 fails
        if let Ok(()) = self.verify_rfc9421(&key_info.public_key, signing_string, &signature_header.signature) {
            return Ok(());
        }
        
        // Try explicit algorithm if specified
        match signature_header.algorithm.as_str() {
            "rsa-sha256" => self.verify_rsa_sha256(&key_info.public_key, signing_string, &signature_header.signature),
            "rsa-sha512" => self.verify_rsa_sha512(&key_info.public_key, signing_string, &signature_header.signature),
            "ed25519" => self.verify_ed25519(&key_info.public_key, signing_string, &signature_header.signature),
            _ => Err(SignatureError::UnsupportedAlgorithm(signature_header.algorithm.clone()))
        }
    }
}

#[derive(Debug)]
pub struct ValidationResult {
    pub actor_id: String,
    pub key_id: String,
    pub trust_level: TrustLevel,
    pub algorithm: SignatureAlgorithm,
}
```

#### Signature Generation

```rust
pub struct HttpSignatureSigner {
    pub private_keys: Arc<RwLock<HashMap<String, PrivateKey>>>,
    pub domain_key: Arc<RwLock<Option<PrivateKey>>>,
    pub master_key: Arc<RwLock<Option<PrivateKey>>>,
}

impl HttpSignatureSigner {
    /// Signs an outgoing HTTP request
    pub async fn sign_request(
        &self,
        request: &mut HttpRequest,
        actor_id: &str,
        body: Option<&[u8]>
    ) -> Result<(), SignatureError> {
        let key_info = self.get_signing_key(actor_id).await?;
        
        // Add required headers
        if !request.headers().contains_key("date") {
            request.headers_mut().insert("date", self.format_http_date().parse()?);
        }
        
        if let Some(body) = body {
            let digest = self.generate_digest(body);
            request.headers_mut().insert("digest", digest.parse()?);
        }
        
        // Build signing string
        let headers = self.select_headers(request, body.is_some());
        let signing_string = self.build_signing_string(request, &headers)?;
        
        // Generate signature
        let signature = self.sign_string(&signing_string, &key_info.private_key, &key_info.algorithm)?;
        
        // Add signature header
        let signature_header = self.format_signature_header(&key_info.key_id, &headers, &signature);
        request.headers_mut().insert("signature", signature_header.parse()?);
        
        Ok(())
    }
    
    /// Generates SHA-256 digest for POST requests
    fn generate_digest(&self, body: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        let hash = Sha256::digest(body);
        format!("SHA-256={}", base64::encode(hash))
    }
    
    /// Formats signature header according to cavage-12
    fn format_signature_header(&self, key_id: &str, headers: &[&str], signature: &str) -> String {
        format!(
            r#"keyId="{}",algorithm="hs2019",headers="{}",signature="{}""#,
            key_id,
            headers.join(" "),
            signature
        )
    }
}
```

### Public Key Infrastructure (PKI)

Oxifed implements a hierarchical PKI system that enables users to bring their own keys while maintaining domain authority and trust chains.

#### Trust Hierarchy

```
┌─────────────────────────────────────────────────────────────┐
│                 Oxifed PKI Trust Chain                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Level 1: Master Key (Root of Trust)                       │
│  ├── Global authority for the Oxifed instance              │
│  ├── Signs domain keys during domain registration          │
│  ├── Emergency key rotation capabilities                   │
│  └── Hardware Security Module (HSM) recommended            │
│                                                             │
│  Level 2: Domain Keys                                      │
│  ├── One per hosted domain                                 │
│  ├── Signed by master key                                  │
│  ├── Signs user keys and instance actor keys              │
│  └── Domain-specific authority                             │
│                                                             │
│  Level 3: User Keys                                        │
│  ├── Brought by users or generated by server               │
│  ├── Signed by domain key when verified                   │
│  ├── Used for ActivityPub activities                      │
│  └── Can be rotated by user                               │
│                                                             │
│  Level 4: Instance Actor Keys                              │
│  ├── Server-level actors for system operations            │
│  ├── Signed by domain key                                 │
│  ├── Used for authorized fetch and system activities      │
│  └── Automatic rotation supported                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### Key Management Implementation

```rust
pub struct PkiManager {
    pub master_key: Arc<RwLock<MasterKey>>,
    pub domain_keys: Arc<RwLock<HashMap<String, DomainKey>>>,
    pub user_keys: Arc<RwLock<HashMap<String, UserKeyInfo>>>,
    pub instance_keys: Arc<RwLock<HashMap<String, InstanceKey>>>,
    pub key_store: Arc<dyn KeyStore>,
    pub hsm: Option<Arc<dyn HsmProvider>>,
}

#[derive(Debug, Clone)]
pub struct UserKeyInfo {
    pub actor_id: String,
    pub key_id: String,
    pub public_key: PublicKey,
    pub private_key: Option<EncryptedPrivateKey>,
    pub domain_signature: Option<DomainSignature>,
    pub trust_level: TrustLevel,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rotation_policy: KeyRotationPolicy,
}

#[derive(Debug, Clone)]
pub struct DomainSignature {
    pub domain: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
    pub domain_key_id: String,
    pub verification_chain: Vec<String>,
}

impl PkiManager {
    /// Allows users to bring their own keys
    pub async fn import_user_key(
        &self,
        actor_id: &str,
        public_key_pem: &str,
        private_key_pem: Option<&str>,
        domain: &str
    ) -> Result<String, PkiError> {
        // 1. Validate key format and strength
        let public_key = self.validate_public_key(public_key_pem)?;
        let private_key = if let Some(pem) = private_key_pem {
            Some(self.encrypt_private_key(pem, actor_id)?)
        } else {
            None
        };
        
        // 2. Generate key ID
        let key_id = format!("{}#main-key", actor_id);
        
        // 3. Create user key info
        let user_key = UserKeyInfo {
            actor_id: actor_id.to_string(),
            key_id: key_id.clone(),
            public_key,
            private_key,
            domain_signature: None,
            trust_level: TrustLevel::Unverified,
            created_at: Utc::now(),
            expires_at: None,
            rotation_policy: KeyRotationPolicy::Manual,
        };
        
        // 4. Store key
        self.key_store.store_user_key(&user_key).await?;
        
        // 5. Initiate domain verification
        self.initiate_domain_verification(actor_id, domain, &key_id).await?;
        
        Ok(key_id)
    }
    
    /// Signs a user key with domain authority
    pub async fn verify_and_sign_user_key(
        &self,
        actor_id: &str,
        domain: &str,
        verification_token: &str
    ) -> Result<(), PkiError> {
        // 1. Verify domain ownership
        self.verify_domain_ownership(actor_id, domain, verification_token).await?;
        
        // 2. Get domain key
        let domain_key = self.domain_keys.read().await
            .get(domain)
            .ok_or(PkiError::DomainKeyNotFound(domain.to_string()))?
            .clone();
        
        // 3. Sign user's public key
        let mut user_key = self.key_store.get_user_key(actor_id).await?;
        let signature = self.sign_user_key(&user_key.public_key, &domain_key).await?;
        
        user_key.domain_signature = Some(DomainSignature {
            domain: domain.to_string(),
            signature,
            signed_at: Utc::now(),
            domain_key_id: domain_key.key_id.clone(),
            verification_chain: vec![domain_key.master_signature.signature.clone()],
        });
        user_key.trust_level = TrustLevel::DomainVerified;
        
        // 4. Update stored key
        self.key_store.update_user_key(&user_key).await?;
        
        Ok(())
    }
    
    /// Generates a new domain key signed by master key
    pub async fn generate_domain_key(&self, domain: &str) -> Result<DomainKey, PkiError> {
        // 1. Generate RSA 4096-bit keypair for domain
        let keypair = self.generate_rsa_keypair(4096)?;
        
        // 2. Create domain key structure
        let key_id = format!("https://{}/.well-known/oxifed/domain-key", domain);
        let domain_key = DomainKey {
            domain: domain.to_string(),
            key_id: key_id.clone(),
            public_key: keypair.public_key,
            private_key: self.encrypt_private_key(&keypair.private_key_pem, &key_id)?,
            master_signature: MasterSignature::default(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(365),
        };
        
        // 3. Sign with master key
        let master_key = self.master_key.read().await;
        let signature = master_key.sign_domain_key(&domain_key)?;
        drop(master_key);
        
        let mut signed_domain_key = domain_key;
        signed_domain_key.master_signature = signature;
        
        // 4. Store domain key
        self.key_store.store_domain_key(&signed_domain_key).await?;
        self.domain_keys.write().await.insert(domain.to_string(), signed_domain_key.clone());
        
        Ok(signed_domain_key)
    }
}
```

#### Well-Known Endpoints for PKI

Oxifed publishes PKI information at standardized endpoints:

```
GET /.well-known/oxifed/master-key
├── Returns master public key and metadata
├── Used for verifying domain key signatures
└── Cached with long TTL

GET /.well-known/oxifed/domain-key
├── Returns domain public key for the current domain
├── Includes master key signature for verification
└── Used for verifying user key signatures

GET /.well-known/oxifed/pki-info
├── Returns complete PKI hierarchy information
├── Trust chain verification endpoints
└── Key rotation notifications

GET /users/{username}#main-key
├── Returns user's public key with domain signature
├── Standard ActivityPub key location
└── Includes trust level and verification chain
```

### Rate Limiting Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                   Rate Limiting Layers                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Layer 1: Load Balancer (Nginx)                            │
│  ├── IP-based rate limiting                                 │
│  ├── Connection limits                                      │
│  └── Request size limits                                    │
│                                                             │
│  Layer 2: domainservd Application                          │
│  ├── Per-actor rate limiting (trust-level aware)           │
│  ├── Per-domain rate limiting                              │
│  ├── Endpoint-specific limits                              │
│  └── Authenticated vs anonymous limits                     │
│                                                             │
│  Layer 3: Database Protection                              │
│  ├── Connection pool limits                                │
│  ├── Query timeout enforcement                             │
│  └── Resource usage monitoring                             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Monitoring & Observability

### Metrics Collection

```rust
// Prometheus metrics definitions
pub struct Metrics {
    pub http_requests_total: IntCounterVec,
    pub http_request_duration: HistogramVec,
    pub activitypub_deliveries_total: IntCounterVec,
    pub activitypub_delivery_duration: HistogramVec,
    pub database_connections_active: IntGauge,
    pub message_queue_messages_total: IntCounterVec,
    pub federation_peers_connected: IntGauge,
}

// Metric collection points
impl Metrics {
    pub fn record_http_request(&self, method: &str, status: u16, duration: Duration) {
        self.http_requests_total
            .with_label_values(&[method, &status.to_string()])
            .inc();
            
        self.http_request_duration
            .with_label_values(&[method])
            .observe(duration.as_secs_f64());
    }
    
    pub fn record_delivery(&self, target_domain: &str, success: bool, duration: Duration) {
        let status = if success { "success" } else { "failure" };
        
        self.activitypub_deliveries_total
            .with_label_values(&[target_domain, status])
            .inc();
            
        self.activitypub_delivery_duration
            .with_label_values(&[target_domain])
            .observe(duration.as_secs_f64());
    }
}
```

### Health Check Endpoints

```
GET /health
├── Component checks:
│   ├── Database connectivity
│   ├── Message queue connectivity  
│   ├── External service dependencies
│   └── Resource utilization
│
GET /health/ready
├── Readiness probe for Kubernetes
└── Returns 200 when ready to serve traffic

GET /health/live
├── Liveness probe for Kubernetes
└── Returns 200 when process is healthy

GET /metrics
├── Prometheus metrics endpoint
└── All application and system metrics
```

### Key Rotation and Recovery

#### Automated Key Rotation

```rust
pub struct KeyRotationManager {
    pub rotation_scheduler: Arc<dyn SchedulerService>,
    pub notification_service: Arc<dyn NotificationService>,
    pub pki_manager: Arc<PkiManager>,
}

impl KeyRotationManager {
    /// Initiates key rotation for an actor
    pub async fn rotate_actor_key(
        &self,
        actor_id: &str,
        rotation_type: RotationType
    ) -> Result<KeyRotationResult, RotationError> {
        match rotation_type {
            RotationType::Scheduled => self.perform_scheduled_rotation(actor_id).await,
            RotationType::Emergency => self.perform_emergency_rotation(actor_id).await,
            RotationType::UserRequested => self.perform_user_rotation(actor_id).await,
        }
    }
    
    async fn perform_scheduled_rotation(&self, actor_id: &str) -> Result<KeyRotationResult, RotationError> {
        // 1. Generate new keypair
        let new_keypair = self.generate_new_keypair(actor_id).await?;
        
        // 2. Create Update activity with new key
        let update_activity = self.create_key_update_activity(actor_id, &new_keypair).await?;
        
        // 3. Sign with old key and publish
        self.publish_signed_update(actor_id, &update_activity).await?;
        
        // 4. Update local storage
        self.update_stored_key(actor_id, &new_keypair).await?;
        
        // 5. Schedule old key cleanup
        self.schedule_old_key_cleanup(actor_id, Duration::days(7)).await?;
        
        Ok(KeyRotationResult::Success {
            old_key_id: format!("{}#main-key", actor_id),
            new_key_id: format!("{}#main-key-{}", actor_id, Utc::now().timestamp()),
            rotation_time: Utc::now(),
        })
    }
}
```

#### Emergency Recovery

```rust
pub struct EmergencyRecovery {
    pub master_key_backup: Arc<dyn SecureBackupService>,
    pub domain_key_escrow: Arc<dyn KeyEscrowService>,
    pub recovery_contacts: Arc<dyn ContactService>,
}

impl EmergencyRecovery {
    /// Recovers from master key compromise
    pub async fn recover_master_key(&self, recovery_token: &str) -> Result<(), RecoveryError> {
        // 1. Validate recovery authorization
        self.validate_recovery_authorization(recovery_token).await?;
        
        // 2. Generate new master key
        let new_master_key = self.generate_new_master_key().await?;
        
        // 3. Re-sign all domain keys
        self.resign_all_domain_keys(&new_master_key).await?;
        
        // 4. Publish master key update
        self.publish_master_key_update(&new_master_key).await?;
        
        // 5. Notify all domains
        self.notify_domain_administrators().await?;
        
        Ok(())
    }
    
    /// Recovers user access when private key is lost
    pub async fn recover_user_access(
        &self,
        actor_id: &str,
        recovery_method: RecoveryMethod
    ) -> Result<String, RecoveryError> {
        match recovery_method {
            RecoveryMethod::DomainAdmin => self.domain_admin_recovery(actor_id).await,
            RecoveryMethod::BackupCodes => self.backup_code_recovery(actor_id).await,
            RecoveryMethod::TrustedContacts => self.trusted_contact_recovery(actor_id).await,
        }
    }
}
```

#### PKI Endpoints Implementation

```rust
pub async fn handle_pki_endpoints(req: HttpRequest) -> Result<HttpResponse, ApiError> {
    match req.path() {
        "/.well-known/oxifed/master-key" => serve_master_key().await,
        "/.well-known/oxifed/domain-key" => serve_domain_key(&req).await,
        "/.well-known/oxifed/pki-info" => serve_pki_info(&req).await,
        "/.well-known/oxifed/trust-chain" => serve_trust_chain(&req).await,
        _ => Err(ApiError::NotFound),
    }
}

async fn serve_master_key() -> Result<HttpResponse, ApiError> {
    let master_key_info = MasterKeyInfo {
        key_id: "https://oxifed.example/.well-known/oxifed/master-key".to_string(),
        public_key_pem: get_master_public_key().await?,
        algorithm: "RSA".to_string(),
        key_size: 4096,
        created_at: get_master_key_creation_time().await?,
        fingerprint: calculate_key_fingerprint(&get_master_public_key().await?),
        usage: vec!["domain-signing".to_string()],
    };
    
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .insert_header(("cache-control", "public, max-age=86400"))
        .json(master_key_info))
}

async fn serve_domain_key(req: &HttpRequest) -> Result<HttpResponse, ApiError> {
    let domain = extract_domain_from_host(req)?;
    let domain_key = get_domain_key(&domain).await?;
    
    let domain_key_info = DomainKeyInfo {
        key_id: format!("https://{}/.well-known/oxifed/domain-key", domain),
        domain: domain.clone(),
        public_key_pem: domain_key.public_key_pem,
        algorithm: "RSA".to_string(),
        key_size: 4096,
        created_at: domain_key.created_at,
        expires_at: domain_key.expires_at,
        master_signature: domain_key.master_signature,
        fingerprint: calculate_key_fingerprint(&domain_key.public_key_pem),
        usage: vec!["user-signing".to_string(), "instance-actor".to_string()],
    };
    
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .insert_header(("cache-control", "public, max-age=3600"))
        .json(domain_key_info))
}
```

This comprehensive PKI implementation provides:

1. **Hierarchical Trust**: Master key → Domain keys → User keys
2. **User Key Import**: Users can bring their own RSA or Ed25519 keys
3. **Domain Verification**: Cryptographic proof of domain authority
4. **Key Rotation**: Automated and emergency rotation capabilities
5. **Recovery Mechanisms**: Multiple recovery options for lost keys
6. **Public Endpoints**: Well-known URLs for key discovery and verification
7. **Trust Levels**: Clear distinction between verified and unverified keys
8. **Interoperability**: Compatible with existing ActivityPub implementations

This architecture ensures strong security while maintaining the flexibility for users to manage their own cryptographic identity within the federated network.

This technical architecture provides the detailed implementation specifications needed to build and deploy the Oxifed ActivityPub platform at scale.