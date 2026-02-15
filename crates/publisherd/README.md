# publisherd -- Oxifed Activity Delivery Daemon

Worker daemon that consumes ActivityPub activities from the message queue and delivers them to remote inboxes with HTTP signatures.

## How It Works

1. Spawns N worker tasks (configurable via `PUBLISHER_WORKERS`)
2. Each worker binds to the `oxifed.activitypub.publish` fanout exchange with its own queue
3. When an activity is received, the worker resolves the recipient inbox URL
4. Signs the outgoing HTTP request using RFC 9421 HTTP Message Signatures
5. Delivers the activity via HTTP POST
6. Retries on failure with configurable attempts and delay

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AMQP_URI` / `AMQP_URL` | `amqp://guest:guest@localhost:5672` | AMQP broker connection |
| `PUBLISHER_WORKERS` | `4` | Number of concurrent delivery workers |
| `PUBLISHER_RETRY_ATTEMPTS` | `3` | Max retry attempts per delivery |
| `PUBLISHER_RETRY_DELAY_MS` | `1000` | Delay between retries in milliseconds |
| `RUST_LOG` | `info` | Log level |

## Running

```bash
# Requires MongoDB and LavinMQ/RabbitMQ running
cargo run -p publisherd
```

## Relationship to domainservd

domainservd publishes activities to the `oxifed.activitypub.publish` exchange. publisherd consumes from that exchange and handles delivery. They share MongoDB for looking up actor keys and recipient information.
