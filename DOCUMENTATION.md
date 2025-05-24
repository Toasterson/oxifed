# Oxifed Documentation Index

Welcome to the comprehensive documentation for the Oxifed ActivityPub platform. This document serves as your guide to understanding, deploying, and extending Oxifed's federated social networking capabilities.

## 📚 Core Documentation

### Platform Overview
- **[README.md](README.md)** - Quick start guide and platform overview
- **[DESIGN.md](DESIGN.md)** - Complete platform architecture and feature specifications
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Detailed technical implementation specifications

### Security and Cryptography
- **[HTTP_SIGNATURES.md](HTTP_SIGNATURES.md)** - Comprehensive HTTP signature implementation guide
- **Security Section in DESIGN.md** - PKI system, trust hierarchy, and key management
- **Security Architecture in ARCHITECTURE.md** - Implementation details for cryptographic systems

## 🔧 Component Documentation

### Core Daemons
- **[domainservd](crates/domainservd/)** - Central ActivityPub server daemon with domain management
- **[publisherd](crates/publisherd/)** - ActivityPub publishing and federation service
- **[oxiadm](crates/oxiadm/README.md)** - Administration, domain, and key management CLI tool

### Domain Management
Oxifed implements a comprehensive domain management system that allows hosting multiple domains on a single instance, each with their own configuration, PKI settings, and federation policies.

#### Domain Registration
Before users can create accounts, administrators must register domains in the system:

```bash
# Register a new domain with basic configuration
oxiadm domain create mydomain.com \
  --name "My Personal Domain" \
  --description "A personal ActivityPub instance" \
  --contact-email "admin@mydomain.com" \
  --registration-mode approval \
  --authorized-fetch true
```

#### Domain Configuration
Each domain supports comprehensive configuration options:

- **Registration Modes**: Open, approval-required, invite-only, or closed
- **Content Limits**: Maximum note length, file sizes, and allowed file types
- **Security Settings**: Authorized fetch mode, PKI configuration
- **Federation Policies**: Custom rules and moderation settings
- **Custom Properties**: JSON-based extensible configuration

#### Multi-Domain Architecture
The system supports multiple domains with isolated configurations:

```bash
# Register multiple domains
oxiadm domain create personal.example \
  --registration-mode closed \
  --max-note-length 280

oxiadm domain create community.example \
  --registration-mode open \
  --max-note-length 1000 \
  --authorized-fetch false

# Each domain operates independently
oxiadm profile create alice@personal.example
oxiadm profile create bob@community.example
```

## 🔐 Cryptographic Key Management

### Overview
Oxifed implements a hierarchical Public Key Infrastructure (PKI) that enables:
- User autonomy over cryptographic keys (Bring Your Own Key)
- Domain-level certificate authority
- Master key for instance-wide trust
- Emergency recovery procedures

### Key Concepts

#### Trust Hierarchy
```
Master Key (Root of Trust)
├── Domain Keys (Per-Domain Authority)
│   ├── User Keys (Individual Identity)
│   └── Instance Actor Keys (System Operations)
```

#### Trust Levels
- **Unverified**: Self-signed user keys without domain verification
- **Domain Verified**: User keys signed by domain authority
- **Master Signed**: Domain keys signed by master key
- **Instance Actor**: Server-level system keys

### Quick Start with Domain and Key Management

1. **Register Your Domain**:
   ```bash
   # Register domain first
   oxiadm domain create example.com \
     --name "Example Community" \
     --description "A federated social community" \
     --contact-email "admin@example.com" \
     --registration-mode approval
   ```

2. **Generate or Import Your Key**:
   ```bash
   # Generate new key
   oxiadm keys generate --actor alice@example.com --algorithm rsa --key-size 2048
   
   # Or import existing key
   oxiadm keys import --actor alice@example.com \
     --public-key ./alice_public.pem \
     --private-key ./alice_private.pem
   ```

3. **Complete Domain Verification**:
   ```bash
   oxiadm keys verify --actor alice@example.com --domain example.com
   oxiadm keys verify-complete --actor alice@example.com \
     --domain example.com --challenge-response ./signed_challenge.txt
   ```

4. **Start Federating**:
   ```bash
   oxiadm note create alice@example.com "Hello, federated world! 🌍"
   ```

## 📖 User Guides

### For End Users
- **Getting Started**: Setting up your account and importing keys
- **Publishing Content**: Creating notes, articles, and other content types
- **Social Features**: Following, liking, boosting, and interacting with others
- **Privacy Controls**: Managing visibility and content filtering

### For Administrators
- **Installation Guide**: Setting up your Oxifed instance
- **Domain Management**: Registering and configuring domains with PKI
- **User Management**: Account administration and moderation
- **Federation Setup**: Connecting with other ActivityPub servers

### For Developers
- **API Reference**: REST APIs for all platform features
- **Plugin Development**: Extending Oxifed with custom functionality
- **Integration Guide**: Connecting external applications
- **Testing Framework**: Automated testing and validation

## 🏗️ Application Development

### Built-in Applications

#### Microblogging (Notes-based)
- Twitter/Mastodon-style short-form content
- Real-time timelines and interactions
- Media attachments and hashtags
- Thread support and conversations

#### Blogging Platform (Article-based)
- Medium/Ghost-style long-form publishing
- Rich text editing with Markdown support
- Series and collaboration features
- SEO optimization and analytics

#### Portfolio Sites (Professional)
- Project showcases and skill matrices
- Experience timelines and certifications
- ActivityPub-based endorsements
- Custom domain support

### Custom Applications
Oxifed's modular architecture enables building custom applications:
- Extend core ActivityPub object types
- Implement custom activity workflows
- Create specialized user interfaces
- Integrate with external services

## 🔧 Technical Specifications

### Standards Compliance
- **ActivityPub**: Full W3C ActivityPub specification compliance
- **HTTP Signatures**: RFC 9421 with cavage-12 backward compatibility
- **Security Vocabulary**: W3C Security Vocabulary for cryptographic operations
- **WebFinger**: RFC 7033 for actor discovery

### Supported Cryptographic Algorithms
- **RSA**: 2048-bit minimum, 4096-bit recommended
- **Ed25519**: Modern elliptic curve cryptography (where supported)
- **Signature Formats**: Cavage-12 and RFC 9421 HTTP signatures
- **Hash Functions**: SHA-256, SHA-512

### Federation Compatibility
Tested and compatible with:
- Mastodon 4.2+
- Pleroma/Akkoma
- GoToSocial
- PeerTube
- Misskey/Calckey
- Pixelfed
- WordPress ActivityPub plugin

## 🛠️ Deployment and Operations

### System Requirements
- **Minimum**: 2 CPU cores, 4GB RAM, 20GB storage
- **Recommended**: 4 CPU cores, 8GB RAM, 100GB SSD
- **Enterprise**: 8+ CPU cores, 16GB+ RAM, 500GB+ SSD

### Dependencies
- **Runtime**: Rust 1.70+, tokio async runtime
- **Database**: MongoDB 6.0+
- **Message Queue**: RabbitMQ 3.11+ or compatible AMQP broker
- **Web Server**: Nginx or Apache for reverse proxy

### Deployment Options
- **Docker Compose**: Single-server deployment with multi-domain support
- **Kubernetes**: Scalable container orchestration with domain isolation
- **Traditional**: Direct installation on Linux servers
- **Cloud**: AWS, GCP, Azure deployment guides with domain management

### Domain Management in Production

#### Domain Lifecycle
```bash
# Create domain with production settings
oxiadm domain create production.social \
  --name "Production Social" \
  --contact-email "admin@production.social" \
  --registration-mode approval \
  --authorized-fetch true \
  --max-note-length 500 \
  --max-file-size 10485760

# Monitor domain status
oxiadm domain show production.social

# Update domain configuration
oxiadm domain update production.social \
  --max-note-length 1000 \
  --registration-mode invite

# List all registered domains
oxiadm domain list
```

#### Domain Migration and Backup
```bash
# Export domain configuration
oxiadm domain show mydomain.com --format json > domain-backup.json

# Domain deletion (with safety checks)
oxiadm domain delete old-domain.com

# Force deletion (removes all users and content)
oxiadm domain delete compromised-domain.com --force
```

## 🔍 Monitoring and Maintenance

### Health Monitoring
- **Health Checks**: `/health`, `/health/ready`, `/health/live` endpoints
- **Metrics**: Prometheus-compatible metrics collection
- **Logging**: Structured JSON logging with log levels
- **Tracing**: OpenTelemetry distributed tracing support

### Key Performance Indicators
- **Signature Verification Rate**: HTTP signature success/failure rates
- **Federation Health**: Connectivity with remote servers
- **Key Trust Distribution**: Percentage of verified vs unverified keys
- **Response Times**: API endpoint performance metrics

### Maintenance Tasks
- **Key Rotation**: Scheduled and emergency key rotation procedures
- **Database Maintenance**: Index optimization and cleanup
- **Cache Management**: Clearing expired keys and signatures
- **Backup Procedures**: Regular backups of keys and data

## 🆘 Troubleshooting

### Common Issues

#### HTTP Signature Failures
- Check timestamp synchronization (NTP)
- Verify key format and encoding
- Confirm trust chain integrity
- Review signature algorithm compatibility

#### Federation Problems
- Test DNS resolution and SSL certificates
- Verify ActivityPub endpoint accessibility
- Check HTTP signature configuration
- Review firewall and proxy settings

#### Key Management Issues
- Validate key format and strength
- Confirm domain verification status
- Check PKI endpoint accessibility
- Review trust chain signatures

### Recovery Procedures

#### Lost Private Keys
1. Use domain authority for emergency rotation
2. Generate new keypair with domain signing
3. Broadcast key update to followers
4. Update cached keys across the network

#### Compromised Master Key
1. Activate emergency recovery procedures
2. Generate new master key from backup
3. Re-sign all domain keys
4. Notify all domain administrators
5. Update well-known endpoints

## 📞 Support and Community

### Getting Help
- **Documentation**: This comprehensive guide
- **Issue Tracker**: GitHub issues for bugs and feature requests
- **Community Forum**: Discussion and support forum
- **Matrix Chat**: Real-time community chat

### Contributing
- **Code Contributions**: Pull requests welcome
- **Documentation**: Help improve these guides
- **Testing**: Report compatibility issues
- **Feedback**: Share your experience and suggestions

### Security Reporting
For security vulnerabilities:
- **Email**: security@oxifed.org
- **PGP Key**: Available at security.txt
- **Response Time**: 48 hours for initial response
- **Disclosure**: Coordinated disclosure process

## 📝 Reference Materials

### Specifications
- [ActivityPub W3C Recommendation](https://www.w3.org/TR/activitypub/)
- [HTTP Signatures RFC 9421](https://datatracker.ietf.org/doc/html/rfc9421)
- [ActivityPub HTTP Signature Profile](https://swicg.github.io/activitypub-http-signature/)
- [W3C Security Vocabulary](https://w3c.github.io/vc-data-integrity/vocab/security/vocabulary.html)

### External Resources
- [Mastodon Documentation](https://docs.joinmastodon.org/)
- [ActivityPub Rocks](https://activitypub.rocks/)
- [Fediverse Developer Resources](https://fedidevs.org/)
- [SocialCG Community Group](https://www.w3.org/community/socialcg/)

## 🔄 Version History

### Current Version: 0.1.0
- Initial release with core ActivityPub functionality
- HTTP signature implementation with PKI
- Basic microblogging and administration tools
- Multi-domain support

### Roadmap
- **0.2.0**: Enhanced microblogging features and web interface
- **0.3.0**: Article publishing platform
- **0.4.0**: Portfolio site functionality
- **1.0.0**: Production-ready release with full feature set

---

This documentation is actively maintained and updated. For the most current information, always refer to the latest version in the repository.

Last updated: December 2024