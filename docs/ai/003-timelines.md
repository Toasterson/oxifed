# 003: Timelines and Collection Pagination

**Status:** Planning
**Created:** 2026-02-23
**Last Updated:** 2026-02-23

## Summary

Implement home timeline (fan-out-on-write), fix local/federated timelines
(fan-out-on-read, partially implemented), and add proper OrderedCollectionPage
pagination to outbox, followers, and following endpoints.

## Current State

### What Exists

- `DatabaseManager::get_public_timeline()` -- queries objects with `visibility: public`
- `DatabaseManager::get_local_timeline()` -- queries objects with `local: true, visibility: public`
- `DatabaseManager::get_actor_outbox()` -- queries activities by actor
- Outbox handler in `activitypub.rs` with `TODO: Implement proper pagination`
- Followers/following handlers return all items inline (no pagination)

### What's Missing

- **Home timeline**: No fan-out mechanism. When a `Create{Note}` arrives at an
  inbox, it's stored but never appears in followers' timelines.
- **Pagination**: Outbox, followers, following all return everything inline.
  ActivityPub spec requires `OrderedCollectionPage` with `first`/`last`/`next`/`prev`.
- **Multi-domain local timeline**: `get_local_timeline()` doesn't filter by domain.
- **Timeline fan-out**: No `TimelineEntry` collection or mechanism.

## Research: ActivityPub Collections

### Spec Requirements

- Collections are `OrderedCollection` with `totalItems`
- Large collections MUST NOT inline items -- use `first`/`last` links
- Pages are `OrderedCollectionPage` with `partOf`, `next`, `prev`
- Items in reverse chronological order (newest first)

### Mastodon Convention (de-facto standard)

- `?page=true` -- return a page, not the collection root
- `?page=true&max_id=X` -- cursor-based backward pagination (older items)
- `?page=true&min_id=X` -- cursor-based forward pagination (newer items)
- Home timeline: fan-out-on-write via Redis sorted sets (status IDs only)
- Local/federated: fan-out-on-read via database queries

## Architecture: Home Timeline Fan-Out

### Strategy: Fan-Out-On-Write with MongoDB

Use a `timelines` collection in MongoDB rather than Redis. Rationale:
- No additional infrastructure dependency
- Durable by default (Redis timelines can be lost)
- Sufficient performance for oxifed's scale
- Query flexibility for multi-domain filtering

### TimelineEntry Document

```rust
pub struct TimelineEntry {
    pub id: Option<ObjectId>,
    /// Actor whose timeline this belongs to
    pub owner_actor_id: String,
    /// Timeline type
    pub timeline_type: TimelineType,  // Home, Notifications, Direct
    /// Reference to the activity
    pub activity_id: String,
    /// Reference to the object (for quick access)
    pub object_id: Option<String>,
    /// Actor who performed the activity
    pub source_actor_id: String,
    /// Activity type (Create, Announce, etc.)
    pub activity_type: String,
    /// Timestamp for ordering
    pub published: DateTime<Utc>,
    /// Boost deduplication: original object ID if this is an Announce
    pub reblog_of: Option<String>,
    /// Domain of the timeline owner
    pub domain: String,
    /// Visibility of the underlying object
    pub visibility: String,
}

pub enum TimelineType {
    Home,
    Notifications,
    Direct,
}
```

### Indexes on `timelines` Collection

```javascript
// Primary timeline query
{ "owner_actor_id": 1, "timeline_type": 1, "published": -1 }

// Cursor-based pagination with _id
{ "owner_actor_id": 1, "timeline_type": 1, "_id": -1 }

// Boost deduplication
{ "owner_actor_id": 1, "timeline_type": 1, "reblog_of": 1 }

// Cleanup of old entries (optional TTL)
{ "published": 1 }
```

### Fan-Out Flow

Fan-out happens in the storage stage of the incoming pipeline (see 001):

```
storaged receives accepted activity
    |
    | Store ObjectDocument + ActivityDocument in MongoDB
    |
    | If activity type is Create, Announce, or Update:
    |   1. Look up local followers of the activity's target actor(s)
    |   2. For each local follower:
    |      a. Check boost deduplication (if Announce, is object already in feed?)
    |      b. Check visibility (followers-only: must actually follow)
    |      c. Insert TimelineEntry
    |
    | If activity type is Follow, Like, Mention:
    |   Insert TimelineEntry with type=Notifications for the target actor
```

### Boost Deduplication

If user A and user B both boost the same post, and you follow both:
- First Announce: insert `TimelineEntry` with `reblog_of: Some(original_object_id)`
- Second Announce: check `reblog_of` index, skip if already present

### Multi-Domain Local Timeline

Add domain parameter to `get_local_timeline()`:

```rust
pub async fn get_domain_local_timeline(
    &self,
    domain: &str,
    limit: i64,
    before_id: Option<&str>,
) -> Result<Vec<ObjectDocument>, DatabaseError>
```

Filter by `attributed_to` matching the domain prefix.

## Collection Pagination

### Pattern

All collection endpoints (outbox, followers, following) follow the same pattern:

**Without `?page` parameter** -- return collection root:

```json
{
  "@context": "https://www.w3.org/ns/activitystreams",
  "type": "OrderedCollection",
  "id": "https://example.com/users/alice/outbox",
  "totalItems": 42,
  "first": "https://example.com/users/alice/outbox?page=true",
  "last": "https://example.com/users/alice/outbox?page=true&min_id=0"
}
```

**With `?page=true`** -- return first page:

```json
{
  "@context": "https://www.w3.org/ns/activitystreams",
  "type": "OrderedCollectionPage",
  "id": "https://example.com/users/alice/outbox?page=true",
  "partOf": "https://example.com/users/alice/outbox",
  "next": "https://example.com/users/alice/outbox?page=true&max_id=abc123",
  "orderedItems": [ ... ]
}
```

**With `?page=true&max_id=X`** -- return page before cursor:

```json
{
  "@context": "https://www.w3.org/ns/activitystreams",
  "type": "OrderedCollectionPage",
  "id": "https://example.com/users/alice/outbox?page=true&max_id=abc123",
  "partOf": "https://example.com/users/alice/outbox",
  "next": "https://example.com/users/alice/outbox?page=true&max_id=def456",
  "prev": "https://example.com/users/alice/outbox?page=true&min_id=abc124",
  "orderedItems": [ ... ]
}
```

### Query Parameters

```rust
pub struct CollectionQuery {
    pub page: Option<bool>,
    pub max_id: Option<String>,   // items older than this cursor
    pub min_id: Option<String>,   // items newer than this cursor
    pub since_id: Option<String>, // all items newer than this
    pub limit: Option<i64>,       // items per page, default 20, max 40
}
```

### Cursor Strategy

Use MongoDB `_id` (ObjectId) as cursor since it's naturally ordered by time.
For the `object_id` / `activity_id` fields, use string comparison with the
full URL as the cursor value.

## Implementation Phases

### Phase 1: Collection Pagination

Fix outbox, followers, following to support `OrderedCollectionPage`:

1. Add `CollectionQuery` to `activitypub.rs` query parameters
2. Modify `get_outbox` handler:
   - Without `page`: return root with `totalItems`, `first`, `last`
   - With `page`: return page with `orderedItems`, `next`, `prev`, `partOf`
3. Modify `get_followers` handler: same pattern
4. Modify `get_following` handler: same pattern
5. Add count methods to `DatabaseManager`: `count_actor_outbox()`,
   `count_followers()`, `count_following()`

### Phase 2: Multi-Domain Local Timeline

1. Add `domain` parameter to `get_local_timeline()`
2. Add `get_domain_local_timeline()` to `DatabaseManager`
3. Wire up to domainservd route (if a timeline API is added)

### Phase 3: Home Timeline Fan-Out

1. Add `TimelineEntry` document type to `database.rs`
2. Add `timelines` collection with indexes to `DatabaseManager`
3. Add `insert_timeline_entry()`, `get_home_timeline()`, `check_reblog_exists()`
4. Integrate fan-out into the storage pipeline stage (storaged from 001)
   - Or: add fan-out to domainservd's existing incoming activity handling
     as an interim solution before the full pipeline is deployed

### Phase 4: Notification Timeline

1. Follow/Like/Mention activities create `TimelineEntry` with type `Notifications`
2. Add `get_notifications()` to `DatabaseManager`

### Phase 5: Timeline API

Add API endpoints (Mastodon-compatible where possible):
- `GET /api/v1/timelines/home` -- home timeline for authenticated user
- `GET /api/v1/timelines/public` -- federated timeline
- `GET /api/v1/timelines/public?local=true` -- local timeline
- `GET /api/v1/notifications` -- notification timeline

These require user authentication (OAuth bearer token or similar).

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/database.rs` | Modify | Add TimelineEntry, timelines collection, count methods, cursor queries |
| `src/lib.rs` | Modify | Add OrderedCollectionPage fields to Collection type |
| `crates/domainservd/src/activitypub.rs` | Modify | Paginate outbox/followers/following, add timeline API routes |
| `crates/storaged/` or `domainservd/rabbitmq.rs` | Modify | Fan-out logic in storage stage |
