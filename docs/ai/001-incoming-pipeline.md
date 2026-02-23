# 001: Incoming Activity Processing Pipeline

**Status:** Planning
**Created:** 2026-02-23
**Last Updated:** 2026-02-23

## Summary

Implement the chained AMQP filter pipeline for incoming ActivityPub activities.
The queue topology is already declared in `domainservd/rabbitmq.rs` (quorum queues with DLX).
Each pipeline stage is a standalone daemon, pluggable at deployment time via K8s.

## Existing Infrastructure

### Declared Queues (domainservd/rabbitmq.rs:180-219)

```
oxifed.incoming.validation          -- Stage 1
oxifed.incoming.spam_filter         -- Stage 2
oxifed.incoming.moderation          -- Stage 3
oxifed.incoming.relationship_verify -- Stage 4
oxifed.incoming.storage             -- Stage 5
```

All queues are quorum queues with:
- 30-minute message TTL
- Dead letter exchange: `oxifed.dlx` -> `oxifed.dlq`

### Current Flow

```
Remote POST /inbox -> domainservd -> EXCHANGE_INCOMING_PROCESS (fanout)
                                          |
                                     (not yet bound to pipeline queues)
```

domainservd publishes `IncomingObjectMessage` and `IncomingActivityMessage`
to `EXCHANGE_INCOMING_PROCESS` but nothing consumes them yet.

## Research: Filtering Approaches

### Pleroma/Akkoma MRF

The gold standard. Sequential policy pipeline where each policy receives the
(possibly modified) message and returns accept/reject/rewrite. Policies are
Elixir behaviours configured per-instance. Simple interface:

```elixir
@callback filter(message) :: {:ok, message} | {:reject, reason}
```

Built-in policies: SimplePolicy (domain blocks), KeywordPolicy, HellthreadPolicy,
TagPolicy, SubchainPolicy, RejectNonPublic, ActivityExpirationPolicy.

### GoToSocial

Sequential access control: HTTP signature -> domain block -> user block -> process.
Adds per-post interaction policies (FEP-7628). Federation modes: blocklist (default)
or allowlist (experimental). HTTP header filtering.

### Mastodon

Hardcoded filtering. No plugin/extension system. Domain suspend/limit/media-reject.
Rejects Create from accounts with no local followers. HTML sanitization allowlist.

## Architecture: AMQP-Based Pipeline

### Why Per-Daemon Stages (Not In-Process Pipeline)

The MRF approach (in-process trait chain) is simpler but oxifed's architecture
is better served by per-daemon stages because:

1. **Deployment flexibility**: Hosters can skip/replace stages via K8s manifests
2. **Independent scaling**: Spam filtering may need more resources than validation
3. **Language agnostic**: A future ML-based spam filter could be Python
4. **Zero-downtime updates**: Update one stage without restarting others
5. **Per-domain routing**: Some stages only apply to certain domains

### Pipeline Topology

Each stage daemon:
1. Binds its input queue to the previous stage's output exchange
2. Processes the message (accept/reject/rewrite)
3. Publishes to its own output exchange (input for next stage)
4. Rejected messages go to DLQ with rejection metadata

```
EXCHANGE_INCOMING_PROCESS (fanout, existing)
    |
    v
[oxifed.incoming.validation] (queue, existing)
    consumed by: validationd
    publishes to: oxifed.pipeline.validated (exchange)
        |
        v
    [oxifed.incoming.spam_filter] (queue, existing)
        consumed by: spamfilterd
        publishes to: oxifed.pipeline.spam_checked (exchange)
            |
            v
        [oxifed.incoming.moderation] (queue, existing)
            consumed by: moderationd
            publishes to: oxifed.pipeline.moderated (exchange)
                |
                v
            [oxifed.incoming.relationship_verify] (queue, existing)
                consumed by: relationshipd
                publishes to: oxifed.pipeline.verified (exchange)
                    |
                    v
                [oxifed.incoming.storage] (queue, existing)
                    consumed by: storaged (or domainservd)
                    final: writes to MongoDB
```

### Intermediate Exchanges

New exchanges to declare (by the stage daemons themselves):

| Exchange | Type | Declared By |
|----------|------|-------------|
| `oxifed.pipeline.validated` | Fanout | validationd |
| `oxifed.pipeline.spam_checked` | Fanout | spamfilterd |
| `oxifed.pipeline.moderated` | Fanout | moderationd |
| `oxifed.pipeline.verified` | Fanout | relationshipd |

### Pipeline Message Envelope

All pipeline messages use a standard envelope that carries metadata through stages:

```rust
/// Pipeline message envelope wrapping the raw ActivityPub activity
pub struct PipelineMessage {
    /// Unique message ID for deduplication
    pub message_id: String,
    /// The raw ActivityPub JSON (may be modified by rewrite stages)
    pub activity: serde_json::Value,
    /// Source domain of the sender
    pub source_domain: String,
    /// Actor ID that sent the activity
    pub actor_id: String,
    /// Target domain (which of our domains received this)
    pub target_domain: String,
    /// Activity type string (Create, Follow, Announce, etc.)
    pub activity_type: String,
    /// Whether HTTP signature was verified
    pub signature_verified: bool,
    /// Source IP address
    pub source_ip: Option<String>,
    /// Audit trail from previous pipeline stages
    pub audit: Vec<AuditEntry>,
    /// Timestamp of original receipt
    pub received_at: String,
}

pub struct AuditEntry {
    pub stage: String,
    pub decision: String,       // "accept", "rewrite", "pass"
    pub reason: Option<String>,
    pub timestamp: String,
}
```

### Rejection Handling

When a stage rejects a message, it does NOT publish to the next exchange.
Instead it publishes to `oxifed.dlx` with headers:

```
x-rejected-by: spamfilterd
x-rejection-reason: "Rate limit exceeded for domain bad.example"
x-rejection-stage: spam_filter
x-original-message-id: <uuid>
```

The DLQ consumer (in domainservd or a dedicated daemon) can log rejections
and make them queryable for admin review.

## Two Levels of Configuration

### Hoster-Level (Instance-Wide)

Configured via environment variables or a TOML config file mounted into the
daemon container. Applies to ALL domains on the instance.

Examples:
- Global domain blocklist
- Global rate limits
- Spam detection thresholds
- Required HTTP signature verification

```toml
# /etc/oxifed/spamfilterd.toml
[rate_limit]
per_domain_per_minute = 120
per_actor_per_minute = 30

[domain_block]
blocked = ["known-spam.example", "another-bad.example"]
```

### Domain-Level (Per-Domain, Operator-Configured)

Stored in MongoDB `DomainDocument` config and managed via the Domain CRD.
Domain operators can request additional filtering when the hoster has enabled
the capability. The operator reconciles CRD changes into MongoDB.

```yaml
apiVersion: oxifed.io/v1alpha1
kind: Domain
metadata:
  name: example-com
spec:
  hostname: example.com
  description: "My domain"
  admin_email: admin@example.com
  filters:
    domain_blocks:
      - domain: "noisy.instance"
        action: silence    # hide from public timelines
      - domain: "bad.actor"
        action: reject
    keywords:
      - pattern: "crypto.*investment"
        action: reject
    hellthread_threshold: 15
    force_sensitive_domains:
      - "questionable.social"
    interaction_policy: followers_only  # only followers can reply
```

The CRD change triggers the operator to update MongoDB. Pipeline daemons
query `DomainDocument` for the target domain's config on each message.

### Config Precedence

1. Hoster-level block -> reject (cannot be overridden by domain)
2. Domain-level block -> reject
3. Hoster-level silence -> hide from public timelines
4. Domain-level silence -> hide from public timelines
5. Domain-level custom rules

## Shared Pipeline Crate

Create `crates/oxifed-pipeline` (or a module in the root crate) with:

```rust
/// Trait for pipeline stage implementations
#[async_trait]
pub trait PipelineStage: Send + Sync {
    /// Stage name (used in audit trail and metrics)
    fn name(&self) -> &str;

    /// Process a single message through this stage
    async fn process(&self, message: PipelineMessage) -> StageResult;
}

pub enum StageResult {
    /// Pass message to next stage (possibly modified)
    Accept(PipelineMessage),
    /// Reject message with reason
    Reject { reason: String, message: PipelineMessage },
    /// Rewrite message and pass to next stage
    Rewrite { reason: String, message: PipelineMessage },
}
```

Plus boilerplate for:
- AMQP connection setup
- Input queue binding
- Output exchange declaration
- Consumer loop with ack/nack
- Graceful shutdown

Each stage daemon is then a thin binary that instantiates the stage trait
and calls the boilerplate runner.

## Implementation Phases

### Phase 1: Pipeline Infrastructure

**Crate:** `oxifed-pipeline` (new, or module in root crate)

1. Define `PipelineMessage` and `AuditEntry` types in `messaging.rs`
2. Define `PipelineStage` trait and `StageResult` enum
3. Implement `PipelineRunner` — generic daemon that:
   - Connects to AMQP
   - Declares its output exchange
   - Binds input queue to specified input exchange
   - Consumes messages, calls `stage.process()`, publishes result
   - Handles rejection (publish to DLX with headers)
   - Graceful shutdown on SIGTERM

### Phase 2: Validation Stage (`validationd`)

First real pipeline daemon. Checks:
- Valid JSON-LD / ActivityPub structure
- Required fields present (`type`, `actor`, `object` for relevant types)
- Actor ID matches HTTP signature key host (anti-spoofing)
- `@context` includes ActivityStreams namespace
- Object size within limits

Input: `EXCHANGE_INCOMING_PROCESS`
Output: `oxifed.pipeline.validated`

### Phase 3: Spam Filter Stage (`spamfilterd`)

Hoster-level filtering:
- Per-domain rate limiting (sliding window in MongoDB or in-memory)
- Per-actor rate limiting
- Global domain blocklist (from config file)
- Keyword matching against content (configurable patterns)
- Burst detection (many activities from same actor in short window)

Input: `oxifed.pipeline.validated`
Output: `oxifed.pipeline.spam_checked`

### Phase 4: Moderation Stage (`moderationd`)

Per-domain filtering (reads `DomainDocument` config):
- Domain-level blocks/silences for the target domain
- Content keyword filters (per-domain rules)
- Force-sensitive marking for specific source domains
- Media stripping for specific source domains
- Hellthread detection (per-domain thresholds)

Input: `oxifed.pipeline.spam_checked`
Output: `oxifed.pipeline.moderated`

### Phase 5: Relationship Verification Stage (`relationshipd`)

- Verify follower relationships (reject DMs from non-followers)
- Interaction policy enforcement (if target actor has restrictions)
- Mention validation (reject mentions from blocked actors)

Input: `oxifed.pipeline.moderated`
Output: `oxifed.pipeline.verified`

### Phase 6: Storage Stage (`storaged`)

Final stage — writes accepted activities to MongoDB:
- Store `ObjectDocument` in `objects` collection
- Store `ActivityDocument` in `activities` collection
- Update follower/following records for Follow/Accept/Reject/Undo
- Timeline fan-out (see 003-timelines.md)

Input: `oxifed.pipeline.verified`
Output: MongoDB (no further AMQP exchange)

### Phase 7: Domain CRD Extension

Extend `DomainSpec` with `filters` field. Update operator reconciliation
to write filter config into `DomainDocument`. Pipeline daemons read config
from MongoDB per-message based on `target_domain`.

### Phase 8: DLQ Admin Interface

- RPC query for rejected messages (count, list, details)
- `oxiadm` commands: `oxiadm moderation rejected --domain example.com`
- Optional: retry rejected messages (re-inject into pipeline)

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/messaging.rs` | Modify | Add `PipelineMessage`, `AuditEntry`, pipeline exchange constants |
| `src/pipeline.rs` | Create | `PipelineStage` trait, `StageResult`, `PipelineRunner` |
| `crates/validationd/` | Create | Validation stage daemon |
| `crates/spamfilterd/` | Create | Spam filter stage daemon |
| `crates/moderationd/` | Create | Moderation stage daemon |
| `crates/relationshipd/` | Create | Relationship verification daemon |
| `crates/storaged/` | Create | Storage stage daemon |
| `crates/oxifed-operator/src/main.rs` | Modify | Extend DomainSpec with filter config |
| `domainservd/rabbitmq.rs` | Modify | Bind pipeline queues to exchanges, update incoming publish |

## Open Questions

1. Should the pipeline stages be separate crates or a single binary with feature flags?
   Separate crates = maximum deployment flexibility. Single binary = simpler builds.
   **Recommendation:** Separate crates, one binary per stage.

2. Should per-domain config be cached in-memory with a refresh interval, or
   queried from MongoDB on every message?
   **Recommendation:** In-memory cache with 60s TTL, refreshed on CRD change events.

3. How should stages that are not deployed be handled? If `spamfilterd` is not
   running, messages pile up in `oxifed.incoming.spam_filter` until TTL.
   **Recommendation:** The `PipelineRunner` should support a "passthrough" mode
   where a minimal daemon just forwards messages without processing. Or: make
   the binding configurable so stages can be skipped by binding the next stage
   directly to the previous stage's output exchange.
