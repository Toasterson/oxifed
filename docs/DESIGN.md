# Oxifed ActivityPub Platform Design Document

## 1. Executive Summary

Oxifed is a comprehensive ActivityPub platform designed to enable federated social networking through a modular, microservices-based architecture. The platform provides the foundational infrastructure for building ActivityPub-compliant applications including microblogging, long-form blogging, and personal portfolio sites.

### Vision
To create a flexible, scalable, and extensible ActivityPub platform that enables developers and users to build federated social applications while maintaining full compatibility with the broader Fediverse ecosystem.

### Core Principles
- **Federation First**: Built from the ground up for ActivityPub federation
- **Modular Architecture**: Component-based design allowing selective deployment
- **Extensibility**: Plugin architecture for custom object types and activities
- **Scalability**: Designed to handle multiple domains and high throughput
- **Standards Compliance**: Full ActivityPub specification compliance

## 2. Architecture Overview

### 2.1 System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web Apps      │    │   Mobile Apps   │    │   CLI Tools     │
│                 │    │                 │    │                 │
│ • Portfolio     │    │ • iOS Client    │    │ • oxiadm        │
│ • Blog          │    │ • Android       │    │ • Admin Tools   │
│ • Microblog     │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                         ┌───────▼───────┐
                         │   domainservd │
                         │               │
                         │ • Inbox       │
                         │ • Outbox      │
                         │ • WebFinger   │
                         │ • Actor API   │
                         └───────┬───────┘
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
            ┌───────▼───────┐    │    ┌───────▼───────┐
            │   publisherd  │    │    │   Worker      │
            │               │    │    │   Daemons     │
            │ • AP Protocol │    │    │               │
            │ • Federation  │    │    │ • Content     │
            │ • Delivery    │    │    │ • Moderation  │
            └───────────────┘    │    │ • Analytics   │
                                 │    └───────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │     Message Queue       │
                    │      (RabbitMQ)         │
                    └─────────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │      Database           │
                    │      (MongoDB)          │
                    └─────────────────────────┘
```

### 2.2 Core Daemons

#### domainservd
The central ActivityPub server daemon that handles:
- **Inbox Management**: Receives and processes incoming ActivityPub messages
- **Outbox Management**: Manages outgoing activities for actors
- **WebFinger Protocol**: Actor discovery and domain verification
- **Actor Management**: CRUD operations for actors and their metadata
- **Domain Routing**: Multi-domain support with per-domain configuration
- **Authentication**: HTTP Signature verification and actor authentication

#### publisherd
Specialized daemon for ActivityPub protocol compliance:
- **Activity Processing**: Handles complex ActivityPub activity workflows
- **Federation Logic**: Manages interactions with remote ActivityPub servers
- **Delivery Management**: Ensures reliable message delivery across the network
- **Protocol Compliance**: Implements full ActivityPub specification requirements
- **Retry Logic**: Handles failed deliveries with exponential backoff

#### oxiadm
Command-line administration tool providing:
- **Actor Management**: Create, update, and delete actor profiles
- **Content Publishing**: Create notes, articles, and other objects
- **Social Interactions**: Follow, like, boost, and comment functionality
- **System Administration**: Domain management and configuration
- **Testing Interface**: Smoke testing and federation verification

### 2.3 Infrastructure Components

#### Message Queue (RabbitMQ)
- **Internal Communication**: Inter-service messaging using defined exchanges
- **Activity Processing**: Asynchronous activity handling and processing
- **Scalability**: Horizontal scaling through message distribution
- **Reliability**: Persistent messaging with acknowledgment patterns

**Key Exchanges:**
- `oxifed.internal.publish`: Internal service communication
- `oxifed.activitypub.publish`: ActivityPub activity publishing
- `INCOMMING_EXCHANGE`: Incoming message processing and filtering

#### Database (MongoDB)
- **Actor Storage**: Actor profiles, credentials, and metadata
- **Activity Storage**: Complete activity logs and object storage
- **Domain Configuration**: Multi-domain settings and routing rules
- **Relationship Management**: Follower/following graphs and social connections

## 3. Application Layer Design

### 3.1 Notes-Based Microblogging Application

#### Overview
A Twitter/Mastodon-style microblogging platform built on Oxifed infrastructure.

#### Features
- **Short-Form Content**: 500-character limit with rich text support
- **Real-Time Timeline**: Live updates using WebSocket connections
- **Social Interactions**: Like, boost, reply, and quote functionality
- **Media Support**: Image, video, and audio attachments
- **Hashtag System**: Topic discovery and trending analysis
- **Thread Support**: Conversation threading and context preservation

#### Technical Implementation
```rust
pub struct MicroblogNote {
    pub id: String,
    pub content: String,
    pub author: Actor,
    pub published: DateTime<Utc>,
    pub in_reply_to: Option<String>,
    pub attachments: Vec<Attachment>,
    pub tags: Vec<Tag>,
    pub mentions: Vec<Actor>,
    pub visibility: Visibility,
}

pub enum Visibility {
    Public,
    Unlisted,
    FollowersOnly,
    Direct,
}
```

#### API Endpoints
- `POST /api/v1/statuses` - Create new note
- `GET /api/v1/timelines/home` - User's home timeline
- `GET /api/v1/timelines/public` - Public timeline
- `POST /api/v1/statuses/:id/favourite` - Like a note
- `POST /api/v1/statuses/:id/reblog` - Boost a note

### 3.2 Article-Based Blogging Platform

#### Overview
A Medium/Ghost-style long-form publishing platform with ActivityPub federation.

#### Features
- **Rich Text Editor**: Markdown support with WYSIWYG editing
- **Publication Management**: Draft, review, and publishing workflows
- **Series Support**: Multi-part article collections
- **Reading Analytics**: View counts, reading time, and engagement metrics
- **Collaboration**: Multi-author support and editorial workflows
- **SEO Optimization**: Automatic meta tags and structured data

#### Technical Implementation
```rust
pub struct BlogArticle {
    pub id: String,
    pub title: String,
    pub content: String,
    pub summary: String,
    pub author: Actor,
    pub co_authors: Vec<Actor>,
    pub published: Option<DateTime<Utc>>,
    pub updated: Option<DateTime<Utc>>,
    pub tags: Vec<Tag>,
    pub featured_image: Option<ImageAttachment>,
    pub reading_time: u32,
    pub status: ArticleStatus,
    pub series: Option<String>,
}

pub enum ArticleStatus {
    Draft,
    InReview,
    Published,
    Archived,
}
```

#### Content Management
- **Revision History**: Full version control for articles
- **Editorial Workflow**: Review and approval processes
- **Content Scheduling**: Timed publication and social media integration
- **Import/Export**: Support for various content formats

### 3.3 Personal Portfolio Site

#### Overview
A professional portfolio platform showcasing work, skills, and achievements with social integration.

#### Features
- **Project Showcase**: Rich media project presentations
- **Skills Matrix**: Technology proficiency and endorsements
- **Experience Timeline**: Career history and achievements
- **Contact Integration**: Professional networking and inquiry handling
- **Blog Integration**: Seamless connection to article platform
- **Social Proof**: ActivityPub-based endorsements and recommendations

#### Technical Implementation
```rust
pub struct Portfolio {
    pub owner: Actor,
    pub projects: Vec<Project>,
    pub skills: Vec<Skill>,
    pub experience: Vec<Experience>,
    pub education: Vec<Education>,
    pub certifications: Vec<Certification>,
    pub theme: PortfolioTheme,
    pub custom_domain: Option<String>,
}

pub struct Project {
    pub id: String,
    pub title: String,
    pub description: String,
    pub technologies: Vec<String>,
    pub media: Vec<Attachment>,
    pub live_url: Option<String>,
    pub repository_url: Option<String>,
    pub featured: bool,
}
```

#### Professional Features
- **Skill Endorsements**: ActivityPub-based professional recommendations
- **Project Collaboration**: Multi-contributor project showcases
- **Achievement Verification**: Blockchain-based credential verification
- **Analytics Dashboard**: Visitor insights and engagement tracking

## 4. ActivityPub Implementation

### 4.1 Core Object Types

#### Supported ActivityPub Objects
- **Note**: Short-form content (microblog posts)
- **Article**: Long-form content (blog posts)
- **Person**: User profiles and actor information
- **Collection**: Timelines, followers, following lists
- **Activity**: All standard ActivityPub activities

#### Custom Extensions
```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://oxifed.org/ns/extensions"
  ],
  "type": "Article",
  "id": "https://example.com/articles/123",
  "oxifed:readingTime": 5,
  "oxifed:series": "https://example.com/series/web-development",
  "oxifed:collaborators": [
    "https://example.com/users/alice",
    "https://example.com/users/bob"
  ]
}
```

### 4.2 Federation Strategy

#### Multi-Domain Support
- **Domain Isolation**: Complete separation of domain data and configuration
- **Shared Infrastructure**: Common daemon infrastructure across domains
- **Custom Branding**: Per-domain theming and customization
- **Independent Moderation**: Domain-specific moderation policies

#### Cross-Platform Compatibility
- **Mastodon Integration**: Full compatibility with Mastodon servers
- **Pleroma Support**: Compatible with Pleroma and forks
- **PeerTube Integration**: Video content federation support
- **WordPress ActivityPub**: Blog post federation with WordPress sites

## 5. Data Architecture

### 5.1 Database Schema

#### Core Collections
```javascript
// actors collection
{
  _id: ObjectId,
  domain: String,
  username: String,
  display_name: String,
  summary: String,
  inbox_url: String,
  outbox_url: String,
  followers_url: String,
  following_url: String,
  public_key: String,
  private_key: String, // encrypted
  created_at: Date,
  updated_at: Date,
  actor_type: String, // Person, Service, Application
  metadata: Object
}

// objects collection
{
  _id: ObjectId,
  ap_id: String, // ActivityPub ID
  object_type: String,
  attributed_to: String,
  content: String,
  summary: String,
  published: Date,
  updated: Date,
  in_reply_to: String,
  tags: Array,
  attachments: Array,
  visibility: String,
  metadata: Object
}

// activities collection
{
  _id: ObjectId,
  ap_id: String,
  activity_type: String,
  actor: String,
  object: String,
  target: String,
  published: Date,
  delivered_to: Array,
  metadata: Object
}
```

### 5.2 Indexing Strategy
- **Actor Lookup**: Compound index on domain + username
- **Timeline Generation**: Index on published date and actor
- **Tag Discovery**: Text index on tags and content
- **Federation**: Index on AP IDs for efficient federation lookups

## 6. Security Considerations

### 6.1 HTTP Signature Authentication

Oxifed implements a robust HTTP signature system following ActivityPub best practices and RFC 9421, with backward compatibility for existing implementations.

#### Signature Profile and Standards Compliance

- **Primary Algorithm**: RSA-SHA256 (2048-bit minimum, 4096-bit recommended)
- **Modern Support**: Ed25519 for new installations where supported
- **Compatibility**: Cavage-12 draft with `hs2019` algorithm placeholder
- **Headers**: `(request-target)`, `host`, `date`, `digest` (for POST requests)
- **Timestamp Window**: 1 hour ± 5 minutes to account for clock skew

#### Double-Knocking Implementation

To ensure maximum compatibility across the fediverse, Oxifed implements "double-knocking":

1. **Primary Attempt**: Try cavage-12 with `hs2019` algorithm
2. **Fallback**: Attempt RFC 9421 if primary fails  
3. **Algorithm-Specific**: Try explicit algorithms (rsa-sha256, rsa-sha512, ed25519)
4. **Version Detection**: Use presence of `Signature-Input` header to detect newer versions

#### Authorized Fetch Support

- **Secure Mode**: Require HTTP signatures on all GET requests
- **Instance Actor**: Dedicated server-level actor to prevent signature deadlocks
- **Access Control**: Domain-level and user-level blocking enforcement
- **Caching Compatibility**: Proper `Vary` header usage for signature-dependent responses

### 6.2 Public Key Infrastructure (PKI)

Oxifed implements a hierarchical PKI system enabling user key autonomy while maintaining domain authority.

#### Trust Hierarchy

```
Master Key (Root Authority)
├── Signs domain keys during registration
├── Emergency recovery capabilities
└── HSM storage recommended

Domain Keys (Per-Domain Authority)  
├── Signed by master key
├── Signs user and instance actor keys
└── Domain-specific certificate authority

User Keys (Individual Identity)
├── User-provided or server-generated
├── Signed by domain key when verified
└── Used for all ActivityPub activities

Instance Actor Keys (System Operations)
├── Server-level authority
├── Signed by domain key
└── Handles authorized fetch and system tasks
```

#### Bring Your Own Key (BYOK) Support

Users can import their existing cryptographic keys:

- **Key Import**: Support for RSA (2048+ bit) and Ed25519 keys
- **Domain Verification**: Cryptographic proof of domain ownership via challenge-response
- **Trust Levels**: Clear distinction between unverified, domain-verified, and master-signed keys
- **Key Rotation**: User-controlled key rotation with ActivityPub Update activities
- **Recovery Options**: Multiple recovery mechanisms for lost private keys

#### PKI Endpoints

Well-known endpoints for key discovery and verification:

- `/.well-known/oxifed/master-key` - Root public key and metadata
- `/.well-known/oxifed/domain-key` - Domain authority key with master signature
- `/.well-known/oxifed/pki-info` - Complete trust hierarchy information
- `/.well-known/oxifed/trust-chain` - Verification chain for any key

### 6.3 Authentication & Authorization

- **Multi-Level Trust**: PKI-based trust levels affect authorization decisions
- **Domain Verification**: Cryptographic proof of domain ownership
- **Rate Limiting**: Trust-level aware rate limiting (verified users get higher limits)
- **Content Validation**: Strict input validation and sanitization
- **Signature Caching**: LRU cache for signature verification results

### 6.4 Content Safety
- **Moderation Tools**: Automated and manual content moderation
- **Spam Prevention**: Machine learning-based spam detection with signature trust factors
- **Content Filtering**: User-configurable content filtering
- **Report System**: Community-driven content reporting
- **Reputation System**: Trust-level integration with content visibility

### 6.5 Privacy Protection
- **Data Minimization**: Collect only necessary data
- **User Control**: Granular privacy settings and data export
- **Encryption**: At-rest and in-transit data encryption
- **Key Escrow**: Optional user key backup with domain authority
- **Compliance**: GDPR and other privacy regulation compliance

### 6.6 Security Monitoring and Incident Response

- **Signature Analytics**: Monitor signature verification patterns for anomalies
- **Key Rotation Tracking**: Automated detection of suspicious key changes
- **Compromise Detection**: Machine learning models for detecting compromised accounts
- **Emergency Procedures**: Rapid key revocation and recovery protocols
- **Audit Logging**: Comprehensive logging of all cryptographic operations

## 7. Scalability & Performance

### 7.1 Horizontal Scaling
- **Stateless Services**: All daemons designed for horizontal scaling
- **Database Sharding**: MongoDB sharding for large datasets
- **CDN Integration**: Asset delivery via content distribution networks
- **Caching Strategy**: Multi-layer caching with Redis integration

### 7.2 Performance Optimization
- **Async Processing**: Non-blocking I/O throughout the stack
- **Batch Operations**: Efficient bulk operations for federation
- **Connection Pooling**: Database and HTTP connection reuse
- **Compression**: Content compression for bandwidth optimization

## 8. Deployment & Operations

### 8.1 Container Architecture
```yaml
# Production deployment example
services:
  domainservd:
    image: oxifed/domainservd:latest
    replicas: 3
    environment:
      - MONGODB_URL=mongodb://mongodb:27017
      - RABBITMQ_URL=amqp://rabbitmq:5672
    
  publisherd:
    image: oxifed/publisherd:latest
    replicas: 2
    
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
```

### 8.2 Monitoring & Observability
- **Metrics Collection**: Prometheus-based metrics gathering
- **Distributed Tracing**: OpenTelemetry integration
- **Log Aggregation**: Centralized logging with structured data
- **Health Checks**: Comprehensive service health monitoring

### 8.3 Backup & Recovery
- **Database Backups**: Automated MongoDB backup strategy
- **Media Storage**: Distributed file storage with redundancy
- **Configuration Management**: Infrastructure as code with version control
- **Disaster Recovery**: Multi-region deployment capabilities

## 9. Development Roadmap

### Phase 1: Core Infrastructure (Completed)
- ✅ Basic ActivityPub server implementation
- ✅ Message queue integration
- ✅ Database abstraction layer
- ✅ CLI administration tools

### Phase 2: Microblogging Platform (In Progress)
- 🔄 Web interface development
- 🔄 Real-time timeline updates
- 🔄 Media attachment handling
- 📋 Mobile application development

### Phase 3: Blogging Platform
- 📋 Rich text editor implementation
- 📋 Editorial workflow system
- 📋 SEO optimization features
- 📋 Analytics dashboard

### Phase 4: Portfolio Platform
- 📋 Portfolio builder interface
- 📋 Professional networking features
- 📋 Skill endorsement system
- 📋 Custom domain support

### Phase 5: Advanced Features
- 📋 Plugin architecture
- 📋 Advanced moderation tools
- 📋 AI-powered content recommendations
- 📋 Enterprise features

## 10. Community & Ecosystem

### 10.1 Developer Ecosystem
- **Plugin API**: Extensible architecture for third-party developers
- **Theme System**: Customizable UI themes and branding
- **Integration Framework**: Easy integration with external services
- **Documentation**: Comprehensive API and developer documentation

### 10.2 User Experience
- **Progressive Web App**: Mobile-first web application
- **Accessibility**: WCAG 2.1 AA compliance
- **Internationalization**: Multi-language support
- **Onboarding**: Streamlined user registration and setup

### 10.3 Federation Network
- **Instance Directory**: Discoverable instance registry
- **Relay Support**: Content relay for improved discovery
- **Migration Tools**: Easy account and data migration
- **Compatibility Testing**: Automated federation testing suite

## 11. Technical Specifications

### 11.1 System Requirements
- **Minimum**: 2 CPU cores, 4GB RAM, 20GB storage
- **Recommended**: 4 CPU cores, 8GB RAM, 100GB SSD
- **Enterprise**: 8+ CPU cores, 16GB+ RAM, 500GB+ SSD

### 11.2 Dependencies
- **Runtime**: Rust 1.70+, tokio async runtime
- **Database**: MongoDB 6.0+
- **Message Queue**: RabbitMQ 3.11+ or compatible AMQP broker
- **Web Server**: Nginx or Apache for reverse proxy

### 11.3 API Versioning
- **Semantic Versioning**: Major.Minor.Patch version scheme
- **Backward Compatibility**: Minimum 1-year deprecation cycle
- **API Documentation**: OpenAPI 3.0 specification
- **Client Libraries**: Official SDKs for major programming languages

This design document provides a comprehensive foundation for the Oxifed ActivityPub platform, enabling the development of federated social applications while maintaining flexibility and standards compliance.