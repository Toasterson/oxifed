# 002: Delivery Feedback System

**Status:** Planning
**Created:** 2026-02-23
**Last Updated:** 2026-02-23

## Summary

publisherd currently delivers activities and logs results but never reports
delivery outcomes back to the system. Admins have no visibility into whether
a Follow was delivered, whether a remote server rejected our signature, or
whether a follower's server is permanently gone.

This plan adds a delivery report mechanism via AMQP so that delivery outcomes
are stored, queryable, and can trigger automatic remediation.

## Current Problems

### 1. Fire-and-Forget Admin Commands

adminservd publishes commands (ProfileCreate, FollowActivity, etc.) to
`EXCHANGE_INTERNAL_PUBLISH` and returns `202 Accepted` immediately.
If domainservd fails to process the command (domain not found, duplicate, etc.),
the error is logged to domainservd's stdout and silently lost.

### 2. Premature Activity Status

In `domainservd/rabbitmq.rs`, `handle_follow()` creates an `ActivityDocument`
with `status: Completed` before the Follow activity is even published to the
AMQP exchange. The activity hasn't been delivered yet.

### 3. No Delivery Status Tracking

publisherd's `process_activity()` tracks success/failure counters but only
logs them. The HTTP status code from remote servers (401, 403, 410, etc.)
is available in `ClientError::StatusError` but is discarded after logging.

### 4. No Automatic Cleanup

When a remote actor returns 410 Gone, the follower relationship should be
cleaned up automatically. Currently dead followers accumulate silently.

### 5. Standalone AMQP Connection

`publish_activity_to_activitypub_exchange()` (rabbitmq.rs:1156) creates a
brand-new AMQP connection on every call instead of using the pool.

## Architecture

```
publisherd delivers activity to remote inbox
    |
    | For each recipient: emit DeliveryReport
    v
[Exchange: oxifed.delivery.report] (fanout)
    |
    v
[Queue: oxifed.delivery.reports] (durable)
    |
    | Consumer (in domainservd or dedicated daemon)
    v
    +-> MongoDB: delivery_reports collection
    +-> Update ActivityDocument.status (Pending -> Completed/Failed)
    +-> Auto-actions:
        - 410 Gone -> remove dead follower
        - 403 repeated -> flag remote server for review
        - 401 repeated -> warn admin about key issues
```

## Message Types

### DeliveryReport

```rust
pub struct DeliveryReport {
    /// Unique report ID
    pub report_id: String,
    /// Activity that was being delivered
    pub activity_id: String,
    /// Actor who originated the activity
    pub actor_id: String,
    /// Activity type (Follow, Create, Announce, etc.)
    pub activity_type: String,
    /// Target recipient actor URL
    pub recipient: String,
    /// Inbox URL that was POSTed to
    pub inbox_url: String,
    /// Delivery outcome
    pub result: DeliveryResult,
    /// Number of attempts made
    pub attempts: u32,
    /// Timestamp
    pub delivered_at: String,
}

pub enum DeliveryResult {
    /// Successfully delivered (2xx)
    Success { status_code: u16 },
    /// Permanent failure -- do not retry
    PermanentFailure {
        status_code: u16,
        reason: String,
        suggested_action: Option<SuggestedAction>,
    },
    /// Transient failure -- may succeed later
    TransientFailure {
        status_code: Option<u16>,
        reason: String,
    },
    /// Could not resolve recipient inbox
    ResolutionFailure { reason: String },
}

pub enum SuggestedAction {
    RemoveFollower { actor_id: String },
    FlagRemoteServer { domain: String },
    RotateKeys { actor_id: String },
}
```

### Error Classification

| HTTP Status | Classification | Suggested Action |
|-------------|---------------|------------------|
| 2xx | Success | None |
| 401 | Permanent | RotateKeys (repeated) |
| 403 | Permanent | FlagRemoteServer (repeated) |
| 404 | Permanent | RemoveFollower |
| 410 | Permanent | RemoveFollower |
| 429 | Transient | Backoff, retry later |
| 500-503 | Transient | Retry with backoff |
| Network error | Transient | Retry with backoff |
| JSON/URL parse | Permanent | None (bad data) |

## Database Schema

### delivery_reports Collection

```javascript
{
  _id: ObjectId,
  activity_id: String,
  actor_id: String,
  activity_type: String,
  recipient: String,
  inbox_url: String,
  success: Boolean,
  status_code: Number,         // nullable
  result_message: String,
  attempts: Number,
  suggested_action: String,    // nullable
  action_taken: Boolean,
  delivered_at: ISODate
}
```

**Indexes:**
- `{ activity_id: 1 }` -- lookup delivery status of specific activity
- `{ actor_id: 1, delivered_at: -1 }` -- recent delivery history per actor
- `{ success: 1, action_taken: 1 }` -- find failures needing action
- `{ delivered_at: 1 }` with TTL (30 days) -- automatic expiry

## Implementation Phases

### Phase 1: Report Emission (publisherd)

1. Add `DeliveryReport`, `DeliveryResult`, `SuggestedAction` to `messaging.rs`
2. Add `EXCHANGE_DELIVERY_REPORT` and `QUEUE_DELIVERY_REPORTS` constants
3. In publisherd `start()`: declare the delivery report exchange
4. Add a report channel to worker context
5. Modify `process_activity()`: after each recipient delivery, emit a `DeliveryReport`
6. Add `classify_error()` to map HTTP status codes to permanent/transient + suggested action
7. Fix: change `deliver_with_retry()` to capture and return the status code

### Phase 2: Report Storage (domainservd or dedicated daemon)

1. Add `DeliveryReportDocument` to `database.rs`
2. Add `delivery_reports_collection()` to `DatabaseManager`
3. Create indexes on the collection
4. Add report consumer to domainservd (or new `delivery-reportd` crate)
5. Consumer: deserialize `DeliveryReport`, insert `DeliveryReportDocument`

### Phase 3: Activity Status Updates

1. Change `handle_follow()` and other activity creators to set `status: Pending`
   instead of `Completed`
2. Report consumer: update `ActivityDocument.status` based on delivery outcomes
   - All recipients succeeded -> `Completed`
   - Any permanent failure -> `Failed` with error detail
   - Still retrying -> keep `Pending`

### Phase 4: Automatic Remediation

1. On 410 Gone: call `db_manager.update_follow_status(follower, following, Cancelled)`
   and mark `action_taken: true` on the report
2. On repeated 403 from same domain (3+ in 24h): insert into `flagged_servers`
   collection for admin review
3. On repeated 401 for same actor: log warning suggesting key rotation

### Phase 5: Admin Visibility

New RPC types:
- `DeliveryRpcRequest::GetActivityStatus { activity_id }` -- per-recipient results
- `DeliveryRpcRequest::ListFailures { actor_id, limit }` -- recent failures
- `DeliveryRpcRequest::ListDeadFollowers { actor_id }` -- 410 Gone followers

New adminservd routes:
- `GET /api/v1/delivery/status/:activity_id`
- `GET /api/v1/delivery/failures?actor=...`
- `GET /api/v1/delivery/dead-followers?actor=...`

New oxiadm commands:
- `oxiadm activity status <activity-id>`
- `oxiadm activity delivery-failures [--actor user@domain]`
- `oxiadm activity cleanup-dead-followers [--actor user@domain] [--dry-run]`

### Phase 6: Fix Connection Leak

Replace the standalone AMQP connection in `publish_activity_to_activitypub_exchange()`
with the existing `deadpool_lapin::Pool` from AppState.

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/messaging.rs` | Modify | Add DeliveryReport types, exchange/queue constants |
| `src/database.rs` | Modify | Add DeliveryReportDocument, collection, indexes |
| `crates/publisherd/src/main.rs` | Modify | Emit delivery reports, error classification |
| `crates/domainservd/src/rabbitmq.rs` | Modify | Add report consumer, fix activity status, fix connection leak |
| `crates/adminservd/src/routes/` | Modify | Add delivery status routes |
| `crates/oxiadm/src/main.rs` | Modify | Add delivery status commands |
