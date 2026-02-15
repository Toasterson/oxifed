# oxiadm -- Oxifed Administration CLI

Command-line tool for administering an Oxifed instance. Sends commands via AMQP messaging and uses RPC for queries.

## Command Status

| Command Group | Subcommands | Status |
|---------------|-------------|--------|
| `domain` | `create`, `update`, `delete` | Working (async AMQP) |
| `domain` | `list`, `show` | Working (RPC query) |
| `user` | `create` | Working (async AMQP) |
| `user` | `list`, `show` | Working (RPC query) |
| `person` | `create`, `update`, `delete` | Working (async AMQP) |
| `note` | `create`, `update`, `delete` | Working (async AMQP) |
| `activity` | `follow`, `like`, `announce` | Working (async AMQP) |
| `keys` | `generate` | Working (sends message, but PKI returns mock keys) |
| `keys` | `import`, `verify`, `verify-complete`, `rotate`, `trust-chain`, `list` | **Stub** -- prints message only |
| `pki` | all subcommands | **Stub** -- prints message only |
| `system` | all subcommands | **Stub** -- prints message only |
| `test` | all subcommands | **Stub** -- prints message only |

## Configuration

Environment variables:

| Variable | Description |
|----------|-------------|
| `AMQP_URI` or `AMQP_URL` | AMQP connection URI (default: `amqp://guest:guest@localhost:5672`) |

## Usage

### Domain Management

```bash
# Register a new domain
oxiadm domain create example.com \
  --name "Example Domain" \
  --description "A sample domain" \
  --contact-email "admin@example.com" \
  --registration-mode approval

# List all domains (RPC query, 30s timeout)
oxiadm domain list

# Show domain details
oxiadm domain show example.com

# Update domain configuration
oxiadm domain update example.com --max-note-length 1000

# Delete a domain
oxiadm domain delete example.com
oxiadm domain delete example.com --force
```

### Profile Management

```bash
# Create a new actor profile
oxiadm person create alice@example.com

# Update profile
oxiadm person update alice@example.com --summary "Updated bio"

# Delete profile
oxiadm person delete alice@example.com
```

### Content Publishing

```bash
# Create a note
oxiadm note create alice@example.com "Hello, fediverse!"

# Create an article
oxiadm article create alice@example.com \
  --title "Getting Started" \
  --content ./article.md
```

### Social Interactions

```bash
# Follow
oxiadm activity follow alice@example.com bob@remote.example

# Like
oxiadm activity like alice@example.com https://remote.example/posts/123

# Announce (boost)
oxiadm activity announce alice@example.com https://remote.example/posts/123
```

## Messaging Architecture

- **Async commands** (create, update, delete): Published to `oxifed.internal.publish` (fanout exchange). Fire-and-forget.
- **Sync queries** (list, show): Published to `oxifed.rpc.request` (direct exchange) with a reply-to queue. 30-second timeout with correlation ID matching.
