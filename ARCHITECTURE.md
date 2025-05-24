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

```rust
pub struct HttpSignatureValidator {
    pub key_cache: Arc<RwLock<HashMap<String, PublicKey>>>,
    pub signature_cache: Arc<RwLock<LruCache<String, bool>>>,
}

impl HttpSignatureValidator {
    pub async fn validate_signature(
        &self,
        request: &HttpRequest,
        body: &[u8]
    ) -> Result<String, SignatureError> {
        // 1. Parse signature header
        let signature_header = self.parse_signature_header(request)?;
        
        // 2. Retrieve public key (with caching)
        let public_key = self.get_public_key(&signature_header.key_id).await?;
        
        // 3. Reconstruct signing string
        let signing_string = self.build_signing_string(request, body, &signature_header.headers)?;
        
        // 4. Verify signature
        self.verify_signature(&public_key, &signing_string, &signature_header.signature)?;
        
        // 5. Return actor ID
        Ok(signature_header.key_id)
    }
}
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
│  ├── Per-actor rate limiting                               │
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

This technical architecture provides the detailed implementation specifications needed to build and deploy the Oxifed ActivityPub platform at scale.