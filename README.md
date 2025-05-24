# Oxifed ActivityPub Platform

A comprehensive, modular ActivityPub platform for building federated social applications including microblogging, long-form blogging, and personal portfolio sites.

## 📚 Documentation

- **[Design Document](DESIGN.md)** - Complete platform architecture and feature specifications
- **[Technical Architecture](ARCHITECTURE.md)** - Detailed implementation specifications and system design

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+
- Docker & Docker Compose
- MongoDB 6.0+
- RabbitMQ 3.11+

### Running the Platform

1. **Start infrastructure services:**
   ```bash
   docker-compose up -d mongodb lavinmq
   ```

2. **Build and run the core daemons:**
   ```bash
   # Build all components
   cargo build --release
   
   # Run domainservd (in one terminal)
   cargo run --bin domainservd
   
   # Run publisherd (in another terminal)  
   cargo run --bin publisherd
   ```

3. **Test with CLI tool:**
   ```bash
   # Register a domain first
   cargo run --bin oxiadm -- domain create example.com \
     --name "Example Domain" \
     --description "A test domain" \
     --contact-email "admin@example.com"
   
   # Create a user profile
   cargo run --bin oxiadm -- profile create alice@example.com --summary "Hello ActivityPub!"
   
   # Publish a note
   cargo run --bin oxiadm -- note create alice@example.com "Hello, federated world!"
   ```

## 🏗️ Core Components

The platform consists of three main daemons that work together to provide ActivityPub functionality:

### domainservd
The central ActivityPub server daemon that handles:
- **Inbox/Outbox APIs**: Serves ActivityPub endpoints for receiving and sending activities
- **WebFinger Protocol**: Enables actor discovery across the federation
- **Actor Management**: CRUD operations for user profiles and actor metadata
- **Multi-domain Support**: Hosts multiple domains with isolated configurations
- **Message Routing**: Distributes incoming activities to worker daemons via RabbitMQ
- **RPC Services**: Handles real-time domain queries via RabbitMQ RPC pattern

All external ActivityPub servers connect to domainservd, and it serves as the main entry point for internal applications. When messages are received at actor inboxes or the shared inbox, they are routed to the `INCOMING_EXCHANGE` for processing by specialized worker daemons. Domain queries use RPC pattern for real-time responses.

### publisherd
Specialized daemon for ActivityPub protocol compliance:
- **Activity Processing**: Listens on `EXCHANGE_ACTIVITYPUB_PUBLISH` for outgoing activities
- **Federation Logic**: Implements the complete ActivityPub specification from [W3C ActivityPub](https://www.w3.org/TR/activitypub/)
- **Delivery Management**: Handles reliable message delivery to remote ActivityPub servers
- **Protocol Compliance**: Ensures all outgoing activities meet ActivityPub standards

### oxiadm
Command-line administration and testing tool:
- **Domain Management**: Register and configure domains in the system
  - Asynchronous commands (create/update/delete) via fanout messaging
  - Synchronous queries (list/show) via RPC pattern with 30-second timeout
- **Profile Management**: Create and manage actor profiles and metadata
- **Content Publishing**: Publish notes, articles, and other ActivityPub objects
- **Social Interactions**: Follow accounts, like posts, and boost content across the federation
- **System Testing**: Provides smoke testing capabilities for federation connectivity
- **Administration**: Domain configuration and system management utilities

*Note: oxiadm is designed for administration and testing - it does not include content viewing capabilities.*

## 🛠️ Applications Built on Oxifed

The platform supports multiple application types:

- **📱 Microblogging App**: Twitter/Mastodon-style short-form content sharing
- **📝 Blog Platform**: Medium/Ghost-style long-form article publishing  
- **💼 Portfolio Sites**: Professional portfolio and networking platform
- **🔧 Custom Apps**: Extensible architecture for custom ActivityPub applications

## 🗄️ Infrastructure

- **Database**: MongoDB for actor profiles, activities, and domain configuration
- **Message Queue**: RabbitMQ with hybrid architecture:
  - Fanout exchanges for asynchronous command processing
  - Direct exchanges for synchronous RPC queries
  - Request-response pattern with correlation IDs and timeouts
- **Federation**: Full ActivityPub protocol support for interoperability with Mastodon, Pleroma, PeerTube, and other platforms

## 📖 Getting Started

1. Read the [Design Document](DESIGN.md) for a comprehensive overview
2. Check the [Technical Architecture](ARCHITECTURE.md) for implementation details
3. Follow the Quick Start guide above to run your first instance
4. Register your domain using `oxiadm domain create` before creating user profiles
5. Use `oxiadm` to create profiles and test federation with existing ActivityPub servers
