# End-to-End Federation Tests for Oxifed

This directory contains comprehensive end-to-end (E2E) tests for the Oxifed federation system. These tests validate the complete federation workflow including domain creation, WebFinger discovery, and ActivityPub message passing between multiple Oxifed instances.

## Overview

The E2E test suite spins up a complete Oxifed federation environment with:

- **3 Domain Instances:**
  - `social.solarm.org` (port 8081)
  - `solarm.space` (port 8082)
  - `social.aopc.cloud` (port 8083)

- **Infrastructure Services:**
  - MongoDB (for data persistence)
  - RabbitMQ/LavinMQ (for message queuing)
  - PostgreSQL (for Mitra instance in interop tests)

- **Publisher Services:**
  - One `publisherd` instance per domain for message federation

## Test Scenarios

The test suite covers the following scenarios:

### Core Federation Tests (`e2e_federation.rs`)

#### 1. Domain Management (`test_e2e_federation_workflow`)
- Creating multiple domains
- Verifying domain configuration
- Testing domain health endpoints

#### 2. WebFinger Discovery (`test_domain_resolution`)
- Testing WebFinger resolution for each domain
- Cross-domain actor discovery
- ActivityPub link verification

#### 3. Cross-Domain Messaging (`test_e2e_federation_workflow`)
- Sending notes between actors on different domains
- Verifying message delivery
- Testing broadcast messages (one-to-many)
- Full circle communication (A→B→C→A)

#### 4. Message Reliability (`test_message_federation_reliability`)
- Rapid message sending (10 messages)
- Delivery rate verification (expects ≥80% success)
- Message ordering and integrity

### ActivityPub Workflow Tests (`e2e_federation_activitypub.rs`)

#### 5. Follow/Accept Workflow (`test_follow_accept_workflow`)
- Sending Follow activities between actors
- Accepting follow requests
- Verifying follower collections
- Testing mutual follows

#### 6. Follow/Reject Workflow (`test_follow_reject_workflow`)
- Testing follow rejection scenarios
- Verifying rejected followers are not added
- Testing selective/private accounts

#### 7. Like Activity Workflow (`test_like_workflow`)
- Liking notes from different domains
- Multiple actors liking the same content
- Verifying Like activity delivery

#### 8. Announce (Boost) Workflow (`test_announce_workflow`)
- Boosting/reposting content to followers
- Testing announce propagation
- Verifying boosted content delivery

#### 9. Undo Workflow (`test_undo_workflow`)
- Unlike functionality
- Unfollow functionality
- Verifying state changes after undo

#### 10. Comprehensive ActivityPub Test (`test_comprehensive_activitypub_workflow`)
- Complete multi-actor interaction scenario
- Mixed Accept/Reject responses
- Reply threads across domains
- Complex activity chains (Follow→Like→Announce→Reply→Undo)

### Interoperability Tests (`e2e_interop.rs`)

#### 11. WebFinger Discovery Interop (`test_webfinger_discovery_interop`)
- Testing WebFinger across Oxifed, snac, and Mitra
- Verifying ActivityPub link compatibility
- Cross-implementation discovery

#### 12. Oxifed → snac Follow (`test_oxifed_to_snac_follow`)
- Testing follow activities from Oxifed to snac
- Verifying cross-implementation activity delivery
- Testing snac's acceptance of Oxifed activities

#### 13. Oxifed → Mitra Interaction (`test_oxifed_to_mitra_interaction`)
- Testing federation between Oxifed and Mitra
- Mastodon API compatibility testing
- Cross-implementation follow relationships

#### 14. Multi-Implementation Federation (`test_multi_implementation_note_federation`)
- Testing note/status propagation across all implementations
- Verifying content delivery between different software
- Testing federation resilience

#### 15. Comprehensive Interop Scenario (`test_comprehensive_interop_scenario`)
- Complex multi-implementation interaction chains
- Testing circular follow relationships across implementations
- Content creation and federation across all platforms

#### 16. Error Handling Interop (`test_error_handling_interop`)
- Testing error scenarios across implementations
- Invalid actor handling
- Rate limiting behavior

## Running Tests Locally

### Prerequisites

- Docker and Docker Compose installed
- Rust toolchain (for native testing)
- At least 4GB of available RAM
- Ports 8081-8083, 27017, 5672, and 15672 available

### Quick Start

```bash
# Run all E2E tests with Docker
cd e2e
./run-local.sh

# Run with verbose output
./run-local.sh --verbose

# Run a specific test
./run-local.sh --filter test_domain_resolution

# Keep containers running after tests
./run-local.sh --keep-running

# Skip building Docker images (use existing)
./run-local.sh --skip-build

# Run interoperability tests with other ActivityPub implementations
./run-interop.sh

# Run specific interop test
./run-interop.sh --filter test_oxifed_to_snac --verbose

# Keep interop services running for manual testing
./run-interop.sh --keep-running
```

### Interoperability Testing

Test Oxifed against other ActivityPub implementations:

```bash
# Run all interop tests (Oxifed, snac, Mitra)
cd oxifed/e2e
./run-interop.sh

# Test specific implementation interaction
./run-interop.sh --filter test_oxifed_to_mitra

# View implementation endpoints
# snac: http://localhost:8084
# Mitra: http://localhost:8085
```

### Manual Docker Compose

```bash
# Start all services
docker-compose -f docker-compose.e2e.yml up -d

# Run tests
docker-compose -f docker-compose.e2e.yml run test-runner

# View logs
docker-compose -f docker-compose.e2e.yml logs -f

# Stop and cleanup
docker-compose -f docker-compose.e2e.yml down -v
```

### Native Testing (without Docker)

```bash
# Start MongoDB
docker run -d -p 27017:27017 \
  -e MONGO_INITDB_ROOT_USERNAME=root \
  -e MONGO_INITDB_ROOT_PASSWORD=testpassword \
  mongo:8

# Start RabbitMQ
docker run -d -p 5672:5672 -p 15672:15672 \
  -e LAVINMQ_DEFAULT_USER=admin \
  -e LAVINMQ_DEFAULT_PASS=testpassword \
  cloudamqp/lavinmq:latest

# Set environment variables
export SOLARM_URL=http://localhost:8081
export SPACE_URL=http://localhost:8082
export AOPC_URL=http://localhost:8083
export MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin"
export AMQP_URI="amqp://admin:testpassword@localhost:5672"

# Run all E2E tests
cargo test --test e2e_federation -- --nocapture --test-threads=1

# Run ActivityPub workflow tests
cargo test --test e2e_federation_activitypub -- --nocapture --test-threads=1

# Run specific ActivityPub test
cargo test --test e2e_federation_activitypub test_follow_accept_workflow -- --nocapture

# Run interoperability tests
cargo test --test e2e_interop -- --nocapture --test-threads=1
```

## Implementations Tested

The E2E suite tests Oxifed against:

| Implementation | Description | Port | Domain |
|----------------|-------------|------|--------|
| **Oxifed** | Your federation server (3 instances) | 8081-8083 | social.solarm.org, solarm.space, social.aopc.cloud |
| **snac** | Simple ActivityPub server | 8084 | snac.aopc.cloud |
| **Mitra** | Federated social media server | 8085 | mitra.aopc.cloud |

### Interoperability Matrix

> **Note:** This matrix represents test definitions, not guaranteed passing status. Check CI results for current pass/fail status.

| From ↓ To → | Oxifed | snac | Mitra |
|-------------|---------|------|-------|
| **Oxifed** | E2E tests defined | E2E tests defined | E2E tests defined |
| **snac** | E2E tests defined | E2E tests defined | E2E tests defined |
| **Mitra** | E2E tests defined | E2E tests defined | E2E tests defined |

Test coverage includes: Follow/Accept/Reject, Create Note/Status, Like/Favorite, Announce/Boost, WebFinger Discovery.

No other ActivityPub implementations (Mastodon, Pleroma, GoToSocial, etc.) have been tested.

## GitHub CI Integration

The E2E tests run automatically in GitHub Actions:

- **On Push:** to `main` or `develop` branches
- **On Pull Request:** targeting `main` or `develop`
- **Manual Trigger:** via workflow dispatch

### CI Workflows

1. **Native Services:** Tests run with services started as GitHub Actions services
2. **Docker Compose:** Full Docker Compose environment (on PRs)

### Viewing CI Results

- Test logs are uploaded as artifacts
- Failed test logs include service outputs
- Test reports are generated with pass/fail status

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SOLARM_URL` | URL for social.solarm.org service | `http://localhost:8081` |
| `SPACE_URL` | URL for solarm.space service | `http://localhost:8082` |
| `AOPC_URL` | URL for social.aopc.cloud service | `http://localhost:8083` |
| `MONGODB_URI` | MongoDB connection string | `mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin` |
| `AMQP_URI` | RabbitMQ connection string | `amqp://admin:testpassword@localhost:5672` |
| `RUST_LOG` | Logging level | `debug` |
| `RUST_BACKTRACE` | Enable backtrace | `1` |

## Debugging Failed Tests

### Local Debugging

1. **Keep containers running after failure:**
   ```bash
   ./run-local.sh --keep-running --verbose
   ```

2. **Check service logs:**
   ```bash
   docker-compose -f docker-compose.e2e.yml logs domainservd-solarm
   docker-compose -f docker-compose.e2e.yml logs publisherd-solarm
   ```

3. **Access MongoDB:**
   ```bash
   docker exec -it mongodb-e2e mongosh \
     --username root --password testpassword
   ```

4. **Access RabbitMQ Management UI:**
   ```
   http://localhost:15672
   Username: admin
   Password: testpassword
   ```

5. **Test endpoints manually:**
   ```bash
   # Health check
   curl http://localhost:8081/health

   # WebFinger
   curl "http://localhost:8081/.well-known/webfinger?resource=acct:alice@social.solarm.org"

   # Actor endpoint
   curl -H "Accept: application/activity+json" \
     http://localhost:8081/users/alice
   ```

### CI Debugging

1. **Download test artifacts:**
   - Go to Actions → Select failed workflow → Download artifacts

2. **Re-run with debug logging:**
   - Use workflow dispatch with `debug_enabled` set to `true`

3. **Check service health in logs:**
   - Look for "Service X is healthy" messages
   - Check for connection errors to MongoDB/RabbitMQ

## Adding New Tests

1. **For basic federation tests, add to `tests/e2e_federation.rs`:**
   ```rust
   #[tokio::test]
   async fn test_new_feature() {
       let helper = E2ETestHelper::new();
       
       // Wait for services
       helper.wait_for_service(&helper.config.solarm_url, 30).await
           .expect("Service failed to start");
       
       // Your test logic here
       
       assert!(condition, "Test assertion message");
   }
   ```

2. **Use the helper methods:**
   - `wait_for_service()` - Wait for service health
   - `create_domain()` - Create a new domain
   - `test_webfinger()` - Test WebFinger discovery
   - `create_actor()` - Create a test actor
   - `send_note()` - Send a note between actors
   - `check_inbox()` - Verify message delivery

2. **For ActivityPub workflow tests, add to `tests/e2e_federation_activitypub.rs`:**
   ```rust
   #[tokio::test]
   async fn test_activitypub_feature() {
       let helper = ActivityPubTestHelper::new();
       
       // Wait for services
       helper.wait_for_services().await
           .expect("Services failed to start");
       
       // Create actors
       let actor_id = helper.create_test_actor(
           &helper.config.solarm_url, 
           "social.solarm.org", 
           "testactor"
       ).await.expect("Failed to create actor");
       
       // Test ActivityPub interactions
       
       assert!(condition, "Test assertion");
   }
   ```

3. **Additional ActivityPub helper methods:**
   - `send_follow()` - Send a Follow activity
   - `accept_follow()` - Accept a follow request
   - `reject_follow()` - Reject a follow request
   - `send_like()` - Like an object
   - `send_announce()` - Boost/repost content
   - `send_undo()` - Undo an activity
   - `get_followers_collection()` - Get an actor's followers

3. **For interoperability tests, add to `tests/e2e_interop.rs`:**
   ```rust
   #[tokio::test]
   async fn test_new_implementation() {
       let helper = InteropTestHelper::new();
       
       // Test cross-implementation federation
       helper.wait_for_all_services().await
           .expect("Services failed");
       
       // Test against snac, Mitra, or other implementations
       
       assert!(condition, "Cross-implementation test");
   }
   ```

4. **Run your new test:**
   ```bash
   # For federation tests
   ./run-local.sh --filter test_new_feature
   
   # For interoperability tests
   ./run-interop.sh --filter test_new_implementation
   ```

## Common Issues and Solutions

### Port Already in Use
```bash
# Find and stop conflicting processes
lsof -i :8081
kill <PID>
```

### Docker Build Failures
```bash
# Clean Docker cache
docker system prune -a
docker-compose -f docker-compose.e2e.yml build --no-cache
```

### MongoDB Connection Issues
```bash
# Check MongoDB is running
docker ps | grep mongodb
# Check MongoDB logs
docker logs mongodb-e2e
```

### RabbitMQ Connection Issues
```bash
# Check RabbitMQ is running
docker ps | grep rabbitmq
# Access management UI
open http://localhost:15672
```

### Test Timeouts
- Increase timeout in `run-local.sh`: `-t 600`
- Check service startup times
- Verify system resources (CPU/RAM)

## Performance Considerations

- Tests run sequentially (`--test-threads=1`) to avoid race conditions
- Each test waits for services to be fully ready
- Message delivery includes retry logic with exponential backoff
- Default timeout is 5 minutes (300 seconds)

## Contributing

When adding new E2E tests:

1. Ensure tests are idempotent
2. Clean up test data after execution
3. Use descriptive test names
4. Add appropriate logging for debugging
5. Document any new environment variables
6. Update this README with new test scenarios
7. Consider the test category:
   - Basic federation → `e2e_federation.rs`
   - ActivityPub workflows → `e2e_federation_activitypub.rs`
   - Interoperability → `e2e_interop.rs`
8. Test both success and failure scenarios
9. Verify state changes after activities
10. For interop tests, consider implementation-specific quirks

## Architecture

```
┌─────────────────────────────────────────┐
│           Test Runner Container         │
│         (cargo test e2e_federation)     │
└─────────────┬───────────────────────────┘
              │ HTTP/AMQP
              ▼
┌─────────────────────────────────────────┐
│         Docker Network (oxifed-e2e)     │
├─────────────────────────────────────────┤
│                                         │
│  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │domainservd│  │domainservd│  │domain  │ │
│  │ -solarm  │  │  -space  │  │ -aopc  │ │
│  └─────┬────┘  └─────┬────┘  └────┬───┘ │
│        │              │            │     │
│  ┌─────▼────┐  ┌─────▼────┐  ┌────▼───┐ │
│  │publisherd│  │publisherd│  │publish │ │
│  │ -solarm  │  │  -space  │  │ -aopc  │ │
│  └─────┬────┘  └─────┬────┘  └────┬───┘ │
│        │              │            │     │
│        └──────────┬───────────────┘     │
│                   │                     │
│           ┌───────▼────────┐            │
│           │   RabbitMQ     │            │
│           └───────┬────────┘            │
│                   │                     │
│           ┌───────▼────────┐            │
│           │    MongoDB     │            │
│           └────────────────┘            │
│                                         │
└─────────────────────────────────────────┘

                    +
           External Implementations
                    │
    ┌───────────────┼───────────────┐
    │               │               │
┌───▼────┐    ┌────▼────┐    ┌────▼───┐
│  snac  │    │  Mitra  │    │ Others │
│        │◄───►         │◄───►        │
└────────┘    └─────────┘    └────────┘
```

## License

These tests are part of the Oxifed project and follow the same license terms.