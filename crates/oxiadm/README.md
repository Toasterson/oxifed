# Oxiadm Administration Tool

Oxiadm is the administration component for the Oxifed ActivityPub server. It provides comprehensive tools for managing ActivityPub entities, cryptographic keys, WebFinger profiles, and system configuration.

## Features

- **ActivityPub Object Management**:
  - Person actors and profiles
  - Notes and articles
  - Activities (Create, Follow, Like, Announce)
  - Custom ActivityPub object types
- **Cryptographic Key Management**:
  - RSA and Ed25519 key generation
  - User key import (Bring Your Own Key)
  - Domain verification and signing
  - Key rotation and recovery
- **PKI Operations**:
  - Master key management
  - Domain key generation and signing
  - Trust chain verification
  - Emergency key recovery
- **WebFinger Profile Management**
- **AMQP Messaging Integration**
- **MongoDB Persistence**
- **Federation Testing and Verification**

## Configuration

The tool requires the following environment variables:

- `MONGODB_URI`: MongoDB connection URI
- `MONGODB_DATABASE`: MongoDB database name
- `RABBITMQ_URI`: RabbitMQ connection URI
- `PKI_MASTER_KEY_PATH`: Path to encrypted master key file
- `PKI_PASSPHRASE`: Passphrase for encrypted keys (optional)

## Usage

### Key Management

#### Generating Keys

Generate a new RSA keypair for a user:

```bash
oxiadm keys generate --actor alice@example.com --algorithm rsa --key-size 2048
```

Generate an Ed25519 keypair:

```bash
oxiadm keys generate --actor alice@example.com --algorithm ed25519
```

#### Importing User Keys (BYOK)

Import an existing RSA key:

```bash
oxiadm keys import --actor alice@example.com \
  --public-key ./alice_public.pem \
  --private-key ./alice_private.pem \
  --algorithm rsa
```

Import an Ed25519 key:

```bash
oxiadm keys import --actor alice@example.com \
  --public-key ./alice_ed25519_public.pem \
  --private-key ./alice_ed25519_private.pem \
  --algorithm ed25519
```

#### Domain Verification

Initiate domain verification for a user key:

```bash
oxiadm keys verify --actor alice@example.com --domain example.com
```

Complete domain verification with challenge response:

```bash
oxiadm keys verify-complete --actor alice@example.com \
  --domain example.com \
  --challenge-response ./signed_challenge.txt
```

#### Key Rotation

Schedule a key rotation:

```bash
oxiadm keys rotate --actor alice@example.com --type scheduled
```

Emergency key rotation (immediate):

```bash
oxiadm keys rotate --actor alice@example.com --type emergency
```

#### Trust Chain Management

View trust chain for a key:

```bash
oxiadm keys trust-chain --key-id "https://example.com/users/alice#main-key"
```

List all keys by trust level:

```bash
oxiadm keys list --trust-level domain-verified
oxiadm keys list --trust-level unverified
```

### PKI Administration

#### Master Key Operations

Initialize a new master key (one-time setup):

```bash
oxiadm pki init-master --key-size 4096 --output ./master_key.pem
```

Backup master key:

```bash
oxiadm pki backup-master --output ./master_backup.pem --encrypt
```

#### Domain Key Management

Generate a domain key:

```bash
oxiadm pki generate-domain-key --domain example.com
```

Sign a domain key with master key:

```bash
oxiadm pki sign-domain-key --domain example.com --master-key ./master_key.pem
```

List all domain keys:

```bash
oxiadm pki list-domains
```

#### Emergency Recovery

Recover from master key compromise:

```bash
oxiadm pki recover-master --recovery-token ./recovery_token.json \
  --new-master-key ./new_master.pem
```

Recover user access with domain authority:

```bash
oxiadm pki recover-user --actor alice@example.com \
  --domain example.com \
  --method domain-admin
```

### Profile Management

Create a new actor profile:

```bash
oxiadm profile create alice@example.com \
  --summary "Software developer interested in federated networks" \
  --icon ./avatar.jpg
```

Update an existing profile:

```bash
oxiadm profile update alice@example.com \
  --summary "Updated bio" \
  --add-property "Website=https://alice.example.com"
```

### Content Publishing

Create a note:

```bash
oxiadm note create alice@example.com \
  "Hello, federated world! #ActivityPub #Federation"
```

Create an article:

```bash
oxiadm article create alice@example.com \
  --title "Getting Started with ActivityPub" \
  --content ./article.md \
  --tags "activitypub,federation,tutorial"
```

### Social Interactions

Follow another user:

```bash
oxiadm follow alice@example.com bob@remote.example
```

Like a post:

```bash
oxiadm like alice@example.com https://remote.example/posts/123
```

Boost (announce) a post:

```bash
oxiadm boost alice@example.com https://remote.example/posts/123
```

### Domain Management

Oxifed implements a hybrid messaging architecture for domain management:

- **Asynchronous Commands** (`create`, `update`, `delete`): Use RabbitMQ fanout exchanges for fire-and-forget messaging
- **Synchronous Queries** (`list`, `show`): Use RabbitMQ RPC with direct exchanges for real-time responses

Register a new domain with the system:

```bash
oxiadm domain create example.com \
  --name "Example Domain" \
  --description "A sample domain for testing" \
  --contact-email "admin@example.com" \
  --registration-mode approval \
  --authorized-fetch true \
  --max-note-length 500 \
  --max-file-size 10485760
```

Update domain configuration:

```bash
oxiadm domain update example.com \
  --authorized-fetch false \
  --max-note-length 1000
```

List all registered domains (real-time query):

```bash
oxiadm domain list
```

Show domain details (real-time query):

```bash
oxiadm domain show example.com
```

Delete a domain (with confirmation):

```bash
oxiadm domain delete example.com
```

Force delete a domain and all its users:

```bash
oxiadm domain delete example.com --force
```

#### RabbitMQ Architecture

The domain management system uses the following RabbitMQ exchanges:

- **oxifed.internal.publish** (fanout): For domain create/update/delete operations
- **oxifed.rpc.request** (direct): For domain query requests
- **oxifed.rpc.response** (direct): For domain query responses

Query commands include a 30-second timeout and use correlation IDs to match requests with responses.

### Federation Testing

Test HTTP signature generation and verification:

```bash
oxiadm test signatures --actor alice@example.com \
  --target https://remote.example/users/bob/inbox
```

Verify federation connectivity:

```bash
oxiadm test federation --actor alice@example.com \
  --remote-actor bob@remote.example
```

Test authorized fetch capability:

```bash
oxiadm test authorized-fetch --actor alice@example.com \
  --target https://remote.example/users/bob/outbox
```
</edits>

<edits>

<old_text>
### Complete Instance Setup from Scratch

```bash
# 1. Register your domain first
oxiadm domain create example.com \
  --name "Example Community" \
  --description "A federated community instance" \
  --contact-email "admin@example.com" \
  --registration-mode approval \
  --authorized-fetch true \
  --max-note-length 500 \
  --max-file-size 10485760

# 2. Generate keys locally (outside of Oxifed)
openssl genpkey -algorithm RSA -pkcs8 -out alice_private.pem -pkeyopt rsa_keygen_bits:2048
openssl pkey -in alice_private.pem -pubout -out alice_public.pem

# 3. Import keys into Oxifed
oxiadm keys import --actor alice@example.com \
  --public-key ./alice_public.pem \
  --private-key ./alice_private.pem \
  --algorithm rsa

# 4. Create actor profile
oxiadm profile create alice@example.com \
  --summary "Federated network enthusiast" \
  --icon ./avatar.jpg

# 5. Initiate domain verification
oxiadm keys verify --actor alice@example.com --domain example.com

# 6. Complete verification (after solving challenge)
oxiadm keys verify-complete --actor alice@example.com \
  --domain example.com \
  --challenge-response ./signed_challenge.txt

# 7. Test federation
oxiadm test federation --actor alice@example.com \
  --remote-actor bob@remote.example

# 8. Publish first note
oxiadm note create alice@example.com \
  "Hello fediverse! This is my first post with my own cryptographic keys! 🔐"
```

### Complete User Setup with BYOK (Domain Already Registered)

If your domain is already registered, you can skip the domain creation step:

```bash
# 1. Generate keys locally (outside of Oxifed)
openssl genpkey -algorithm RSA -pkcs8 -out alice_private.pem -pkeyopt rsa_keygen_bits:2048
openssl pkey -in alice_private.pem -pubout -out alice_public.pem

# 2. Import keys into Oxifed
oxiadm keys import --actor alice@example.com \
  --public-key ./alice_public.pem \
  --private-key ./alice_private.pem \
  --algorithm rsa

# 3. Create actor profile
oxiadm profile create alice@example.com \
  --summary "Federated network enthusiast" \
  --icon ./avatar.jpg

# 4. Initiate domain verification
oxiadm keys verify --actor alice@example.com --domain example.com

# 5. Complete verification (after solving challenge)
oxiadm keys verify-complete --actor alice@example.com \
  --domain example.com \
  --challenge-response ./signed_challenge.txt

# 6. Test federation
oxiadm test federation --actor alice@example.com \
  --remote-actor bob@remote.example

# 7. Publish first note
oxiadm note create alice@example.com \
  "Hello fediverse! This is my first post with my own cryptographic keys! 🔐"
```

### System Administration

Check system health:

```bash
oxiadm system health
```

View PKI status:

```bash
oxiadm system pki-status
```

Generate system report:

```bash
oxiadm system report --output ./system_report.json
```

### Configuration Management

Set domain configuration:

```bash
oxiadm config set-domain example.com \
  --authorized-fetch true \
  --registration-mode approval
```

Manage instance settings:

```bash
oxiadm config set-instance \
  --max-note-length 500 \
  --max-file-size 10MB
```

## Security Best Practices

### Key Management

1. **Use Strong Keys**: Minimum 2048-bit RSA or Ed25519 keys
2. **Secure Storage**: Encrypt private keys at rest
3. **Regular Rotation**: Schedule key rotations based on usage patterns
4. **Backup Strategy**: Maintain secure backups of critical keys
5. **Access Control**: Limit access to key management operations

### Domain Verification

1. **Prompt Verification**: Complete domain verification quickly after key import
2. **Challenge Security**: Use secure random challenges for domain verification
3. **Certificate Monitoring**: Monitor domain key certificates for changes
4. **Revocation Procedures**: Have clear procedures for key revocation

### Emergency Procedures

1. **Recovery Planning**: Document emergency recovery procedures
2. **Contact Lists**: Maintain updated emergency contact information
3. **Backup Access**: Ensure multiple people can perform emergency operations
4. **Communication**: Prepare communication templates for security incidents

## Examples

### Complete User Setup with BYOK

```bash
# 1. Generate keys locally (outside of Oxifed)
openssl genpkey -algorithm RSA -pkcs8 -out alice_private.pem -pkeyopt rsa_keygen_bits:2048
openssl pkey -in alice_private.pem -pubout -out alice_public.pem

# 2. Import keys into Oxifed
oxiadm keys import --actor alice@example.com \
  --public-key ./alice_public.pem \
  --private-key ./alice_private.pem \
  --algorithm rsa

# 3. Create actor profile
oxiadm profile create alice@example.com \
  --summary "Federated network enthusiast" \
  --icon ./avatar.jpg

# 4. Initiate domain verification
oxiadm keys verify --actor alice@example.com --domain example.com

# 5. Complete verification (after solving challenge)
oxiadm keys verify-complete --actor alice@example.com \
  --domain example.com \
  --challenge-response ./signed_challenge.txt

# 6. Test federation
oxiadm test federation --actor alice@example.com \
  --remote-actor bob@remote.example

# 7. Publish first note
oxiadm note create alice@example.com \
  "Hello fediverse! This is my first post with my own cryptographic keys! 🔐"
```

### Emergency Key Recovery

```bash
# 1. Detect compromise
oxiadm keys list --actor alice@example.com --show-suspicious

# 2. Emergency rotation
oxiadm keys rotate --actor alice@example.com --type emergency

# 3. Verify new key
oxiadm keys trust-chain --key-id "https://example.com/users/alice#main-key"

# 4. Test federation with new key
oxiadm test signatures --actor alice@example.com \
  --target https://remote.example/users/bob/inbox

# 5. Notify contacts
oxiadm note create alice@example.com \
  "I've rotated my cryptographic keys due to a security concern. Please update any cached keys."
```

## Managing ActivityPub Objects

Oxiadm supports various types of ActivityPub objects through a flexible system that can be extended to accommodate new object types in the future. The system uses the `ActivityObject` trait to standardize behavior across different object types.

## Advanced Usage

### Batch Operations

Process multiple keys:

```bash
oxiadm keys batch-process --input ./user_keys.json --operation verify
```

Bulk profile updates:

```bash
oxiadm profile batch-update --input ./profile_updates.csv
```

### Monitoring and Analytics

Monitor signature verification rates:

```bash
oxiadm monitor signatures --actor alice@example.com --duration 24h
```

Analyze federation health:

```bash
oxiadm analyze federation --domain example.com --output ./federation_report.json
```

### Integration with External Tools

Export keys for external use:

```bash
oxiadm keys export --actor alice@example.com --format pem --output ./alice_keys/
```

Import from external identity providers:

```bash
oxiadm keys import-from-provider --actor alice@example.com \
  --provider keybase \
  --username alice_keybase
```

Administration CLI tool for Oxifed, designed with a structure similar to Solaris commands and providing comprehensive cryptographic key management capabilities.

## Domain Registration

Before creating user profiles, you must first register a domain with the Oxifed system. This establishes the domain configuration, PKI settings, and federation policies.

### Registration Modes

- **open**: Anyone can register accounts on this domain
- **approval**: Account registration requires manual approval
- **invite**: Account registration is by invitation only
- **closed**: No new account registrations allowed

### Basic Domain Setup

```bash
# Register a new domain with basic settings
oxiadm domain create mydomain.com \
  --name "My Personal Domain" \
  --description "A personal ActivityPub instance" \
  --contact-email "admin@mydomain.com" \
  --registration-mode approval

# Enable authorized fetch for better security
oxiadm domain update mydomain.com --authorized-fetch true

# Set content limits
oxiadm domain update mydomain.com \
  --max-note-length 500 \
  --max-file-size 10485760 \
  --allowed-file-types image/jpeg \
  --allowed-file-types image/png
```

### Domain Configuration Options

The domain creation supports various configuration options:

- `--name`: Human-readable display name for the domain
- `--description`: Domain description shown to users
- `--contact-email`: Administrative contact email
- `--rules`: Domain rules (can be specified multiple times)
- `--registration-mode`: Registration policy (open/approval/invite/closed)
- `--authorized-fetch`: Enable authorized fetch mode for enhanced security
- `--max-note-length`: Maximum length for notes/posts
- `--max-file-size`: Maximum file upload size in bytes
- `--allowed-file-types`: Permitted file types (can be specified multiple times)
- `--properties`: Additional domain properties as JSON

### Managing Existing Domains

```bash
# List all registered domains
oxiadm domain list

# View detailed domain information
oxiadm domain show mydomain.com

# Update domain settings
oxiadm domain update mydomain.com \
  --description "Updated domain description" \
  --max-note-length 1000

# Delete a domain (requires confirmation unless --force is used)
oxiadm domain delete old-domain.com

# Force delete a domain and all associated users
oxiadm domain delete unwanted-domain.com --force
```

## Creating Profiles

After registering a domain, you can create user profiles. Create a new actor profile with automatic key generation:

```
</edits>