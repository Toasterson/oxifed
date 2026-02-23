# 004: pkgfed -- Subscription-Based Package Update Notifications

**Status:** Planning
**Created:** 2026-02-23
**Last Updated:** 2026-02-23

## Summary

pkgfed is a separate application built on oxifed's shared infrastructure that
enables subscription-based package release notifications via ActivityPub.

**Key distinction**: pkgfed is NOT a registry event bus that republishes every
release from every registry. It is subscriber-driven: a user says "I want
updates about package X" and pkgfed ensures they receive notifications when
X publishes a new release, advisory, or yank.

pkgfed sits on top of AMQP. It uses `domainservd` for serving ActivityPub
endpoints (actor profiles, WebFinger, inbox) and `publisherd` for delivering
outbound activities. It does not implement its own HTTP server for federation.

## Inspiration

Based on [pkgfed: ActivityPub for Package Releases](https://nesbitt.io/2026/01/25/pkgfed-activitypub-for-package-releases.html)
by Andrew Nesbitt, which proposes mapping package ecosystem concepts to
ActivityPub primitives:

| Package Concept | ActivityPub Primitive |
|---|---|
| Package | Actor (followable, has inbox/outbox) |
| Release | `Create{SoftwareApplication}` activity |
| CVE/Advisory | Reply to release post (threaded) |
| Yank | `Delete` activity on release object |
| Dependency | Follow relationship |

## Design Principles

1. **Subscription-driven, not broadcast**: pkgfed only tracks packages that
   someone has asked to follow. It does not mirror entire registries.

2. **Event-triggered publishing**: When a tracked package releases a new version,
   pkgfed publishes a `Create{SoftwareApplication}` activity to followers.
   It does not poll continuously -- it watches for events.

3. **Uses existing infrastructure**: pkgfed interacts with the fediverse through
   `domainservd` (inbound) and `publisherd` (outbound), the same way `oxiadm`
   uses `adminservd`. No duplicate HTTP signature handling.

4. **Followable from Mastodon**: `serde@packages.example.com` shows up in your
   Mastodon timeline when it releases, without any special client.

5. **No HTTP signature complexity for humans**: Users follow a package from
   their existing Mastodon/fediverse account. The plumbing is invisible.

## Architecture

```
                    Fediverse (Mastodon, etc.)
                         |
                    POST /inbox (Follow serde@packages.example.com)
                         |
                         v
                    domainservd  (serves actor, webfinger, receives follows)
                         |
                    EXCHANGE_INCOMING_PROCESS -> pipeline -> storage
                         |
                         | (stored: FollowDocument in MongoDB)
                         |
                    pkgfed daemon (AMQP consumer)
                         |
                         | Sees: new follower for package actor "serde"
                         | Action: ensure package is tracked
                         |         register with registry event source
                         |
                    ~~~~ time passes ~~~~
                         |
                    Registry Event Source
                         | (webhook, RSS, polling, WebSub -- per registry)
                         |
                    pkgfed daemon
                         |
                         | Receives: serde 1.0.217 released
                         | Action: create SoftwareApplication object
                         |         publish Create activity via AMQP
                         |
                    EXCHANGE_INTERNAL_PUBLISH  (or EXCHANGE_ACTIVITYPUB_PUBLISH)
                         |
                         v
                    domainservd (stores object + activity)
                         |
                    EXCHANGE_ACTIVITYPUB_PUBLISH
                         |
                         v
                    publisherd (delivers to followers' inboxes)
                         |
                         v
                    Mastodon user sees: "serde 1.0.217 released"
```

### How pkgfed Interacts with AMQP

pkgfed is an AMQP consumer and publisher, similar to how `domainservd`
processes admin commands:

**Consumes from:**
- A new exchange/queue for package events: `oxifed.pkgfed.events`
- Optionally: incoming follow notifications from the pipeline
  (to trigger tracking new packages)

**Publishes to:**
- `EXCHANGE_INTERNAL_PUBLISH` -- to create package actors and publish
  release activities (same path as `oxiadm`/`adminservd`)
- Or directly to `EXCHANGE_ACTIVITYPUB_PUBLISH` -- to publish delivery-ready
  activities

### Package Actors

Each tracked package is represented as an ActivityPub `Application` actor:

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://doi.org/10.5063/schema/codemeta-2.0"
  ],
  "type": "Application",
  "id": "https://packages.example.com/packages/serde",
  "name": "serde",
  "summary": "A generic serialization/deserialization framework",
  "preferredUsername": "serde",
  "inbox": "https://packages.example.com/packages/serde/inbox",
  "outbox": "https://packages.example.com/packages/serde/outbox",
  "followers": "https://packages.example.com/packages/serde/followers",
  "url": "https://crates.io/crates/serde",
  "attachment": [
    {
      "type": "PropertyValue",
      "name": "Registry",
      "value": "crates.io"
    },
    {
      "type": "PropertyValue",
      "name": "Language",
      "value": "Rust"
    }
  ]
}
```

### Release Activities

When a tracked package publishes a new release:

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://doi.org/10.5063/schema/codemeta-2.0"
  ],
  "type": "Create",
  "actor": "https://packages.example.com/packages/serde",
  "to": ["https://packages.example.com/packages/serde/followers"],
  "cc": ["https://www.w3.org/ns/activitystreams#Public"],
  "object": {
    "type": "Note",
    "attributedTo": "https://packages.example.com/packages/serde",
    "content": "<p><strong>serde 1.0.217</strong> released</p><p>A generic serialization/deserialization framework</p><p><a href=\"https://crates.io/crates/serde/1.0.217\">View on crates.io</a></p>",
    "published": "2026-02-23T12:00:00Z",
    "tag": [
      { "type": "Hashtag", "name": "#rust" },
      { "type": "Hashtag", "name": "#crates" },
      { "type": "Hashtag", "name": "#serde" }
    ],
    "attachment": [
      {
        "type": "PropertyValue",
        "name": "Version",
        "value": "1.0.217"
      },
      {
        "type": "PropertyValue",
        "name": "PURL",
        "value": "pkg:cargo/serde@1.0.217"
      }
    ]
  }
}
```

**Note**: Using `Note` type (not `SoftwareApplication`) ensures maximum
compatibility with existing Mastodon/Pleroma clients. The structured data
lives in `attachment` PropertyValue fields. A future extension could also
include the full Codemeta `SoftwareApplication` as an additional `@context`
entry for clients that understand it.

### Security Advisory Activities

When a CVE is published for a tracked package:

```json
{
  "type": "Create",
  "actor": "https://packages.example.com/packages/serde",
  "object": {
    "type": "Note",
    "content": "<p>Security Advisory for <strong>serde</strong></p><p>RUSTSEC-2026-0001: Buffer overflow in deserialize</p><p>Affected: &lt; 1.0.217</p><p>Fixed: 1.0.217</p>",
    "inReplyTo": "https://packages.example.com/packages/serde/releases/1.0.216",
    "tag": [
      { "type": "Hashtag", "name": "#security" },
      { "type": "Hashtag", "name": "#advisory" }
    ],
    "sensitive": true,
    "summary": "Security Advisory: RUSTSEC-2026-0001"
  }
}
```

Threading the advisory as a reply to the affected release post is a neat trick
from the pkgfed blog post -- it contextualizes the vulnerability.

## Data Model

### MongoDB Collections

**Package tracking** (new collection: `packages`):

```javascript
{
  _id: ObjectId,
  // Package identity
  name: "serde",
  registry: "crates.io",
  purl: "pkg:cargo/serde",
  // Actor identity (managed by domainservd)
  actor_id: "https://packages.example.com/packages/serde",
  domain: "packages.example.com",
  // Tracking state
  tracked: true,             // false = no followers, stop watching
  follower_count: 42,
  // Registry integration
  event_source_type: "rss",  // rss, webhook, polling, websub
  event_source_url: "https://crates.io/api/v1/crates/serde",
  last_checked: ISODate,
  last_version: "1.0.217",
  // Metadata
  description: "A generic serialization/deserialization framework",
  language: "Rust",
  homepage: "https://serde.rs",
  repository: "https://github.com/serde-rs/serde",
  created_at: ISODate,
  updated_at: ISODate,
}
```

### Registry Event Sources

pkgfed needs to watch registries for updates. Strategy per registry:

| Registry | Event Source | Method |
|----------|-------------|--------|
| crates.io | RSS feed per crate | Poll RSS |
| npm | Registry API | Poll API |
| PyPI | RSS feed per package | Poll RSS |
| RubyGems | WebHook (if available) | Webhook receiver |
| GitHub Releases | GitHub API / Webhooks | Webhook or poll |
| Generic | RSS/Atom feed URL | Poll RSS |

**Polling strategy**: Only poll packages that have active followers.
When follower count drops to 0, stop polling (mark `tracked: false`).
Polling interval: configurable per registry, default 15 minutes.

## Workflow

### 1. User Follows a Package

```
User on Mastodon searches: @serde@packages.example.com
    |
    | Mastodon does WebFinger lookup
    v
domainservd returns actor profile for serde
    |
    | User clicks Follow
    v
Mastodon POSTs Follow to serde's inbox
    |
    v
domainservd processes Follow (auto-accept)
    |
    | Stores FollowDocument
    | Publishes Accept via publisherd
    v
pkgfed daemon detects new follow for package actor
    |
    | If package not yet tracked:
    |   1. Look up package metadata from registry API
    |   2. Insert package document into packages collection
    |   3. Start watching for events
    | Update follower_count
```

### 2. Package Releases New Version

```
pkgfed polling loop checks crates.io API
    |
    | Detects: serde 1.0.217 (newer than last_version 1.0.216)
    v
pkgfed creates release activity
    |
    | Publishes NoteCreateMessage to EXCHANGE_INTERNAL_PUBLISH
    |   (same path as oxiadm creating a note)
    v
domainservd processes NoteCreateMessage
    |
    | Stores ObjectDocument + ActivityDocument
    | Looks up serde's followers
    | Publishes activity to EXCHANGE_ACTIVITYPUB_PUBLISH
    v
publisherd delivers to each follower's inbox
    |
    v
User sees in Mastodon: "serde 1.0.217 released"
```

### 3. Last Follower Unfollows

```
Mastodon POSTs Undo{Follow} to serde's inbox
    |
    v
domainservd processes Undo (removes FollowDocument)
    |
    v
pkgfed detects follower_count dropped to 0
    |
    | Mark package as tracked: false
    | Stop polling registry for this package
```

## pkgfed Daemon Design

### Crate: `crates/pkgfed/`

Single binary with two async tasks:

1. **Follow watcher**: Consumes follow/unfollow events from AMQP
   (or queries MongoDB on interval) to detect new package subscriptions
   and unsubscriptions.

2. **Registry poller**: Periodically checks tracked packages for new
   releases. Configurable interval per registry. Only active packages
   (follower_count > 0) are polled.

### Registry Adapter Trait

```rust
#[async_trait]
pub trait RegistryAdapter: Send + Sync {
    /// Registry name (e.g., "crates.io", "npm")
    fn name(&self) -> &str;

    /// Check for new releases of a package
    async fn check_for_updates(
        &self,
        package_name: &str,
        since_version: Option<&str>,
    ) -> Result<Vec<ReleaseInfo>, RegistryError>;

    /// Fetch package metadata
    async fn get_package_info(
        &self,
        package_name: &str,
    ) -> Result<PackageInfo, RegistryError>;
}

pub struct ReleaseInfo {
    pub version: String,
    pub published_at: DateTime<Utc>,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub changelog_url: Option<String>,
    pub is_yanked: bool,
}

pub struct PackageInfo {
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub language: Option<String>,
    pub license: Option<String>,
}
```

### Configuration

```toml
# pkgfed.toml
[general]
domain = "packages.example.com"
poll_interval_seconds = 900  # 15 minutes

[amqp]
url = "amqp://guest:guest@localhost:5672"

[mongodb]
uri = "mongodb://root:password@localhost:27017"
dbname = "domainservd"

[registries.crates_io]
enabled = true
adapter = "crates_io"
poll_interval_seconds = 600  # 10 minutes

[registries.npm]
enabled = true
adapter = "npm"
poll_interval_seconds = 900

[registries.pypi]
enabled = false
adapter = "pypi"
```

## Implementation Phases

### Phase 1: Package Actor Management

1. Create `crates/pkgfed/` with binary and basic structure
2. Define `PackageDocument` in shared database types
3. Implement: given a package name + registry, create an ActivityPub actor
   via AMQP (publish `ProfileCreateMessage` with actor_type `Application`)
4. WebFinger: ensure `serde@packages.example.com` resolves correctly
   (domainservd already handles this for any actor on any hosted domain)

### Phase 2: Follow Detection

1. pkgfed watches MongoDB `follows` collection for new follows
   targeting package actors (filter by domain `packages.example.com`)
2. On new follow: if package not tracked, call registry adapter to
   fetch metadata, insert `PackageDocument`, start tracking
3. On unfollow (follower_count -> 0): mark `tracked: false`

### Phase 3: Registry Adapters

1. Implement `CratesIoAdapter` -- uses crates.io API to check versions
2. Implement `NpmAdapter` -- uses npm registry API
3. Implement `GenericRssAdapter` -- polls RSS/Atom feeds

### Phase 4: Release Publishing

1. Polling loop: for each tracked package, check registry adapter
2. On new release: build `NoteCreateMessage` with release content
3. Publish to `EXCHANGE_INTERNAL_PUBLISH`
4. domainservd + publisherd handle the rest (storage + delivery)

### Phase 5: Security Advisories

1. Watch RustSec/GitHub Advisory Database for tracked packages
2. On new advisory: create Note as reply to affected release
3. Mark as `sensitive: true` with content warning

### Phase 6: Yank/Delete Handling

1. On yanked version: publish `Delete` activity for the release object
2. Mastodon clients show "this post was deleted"

### Phase 7: K8s Deployment

1. Dockerfile for pkgfed
2. K8s Deployment manifest
3. Domain CRD for `packages.example.com` (same as any other oxifed domain)

## Relationship to Existing Infrastructure

pkgfed does NOT duplicate any oxifed infrastructure:

| Concern | Handled By |
|---------|-----------|
| Serving actor profiles | domainservd |
| WebFinger discovery | domainservd |
| Receiving Follow/Undo | domainservd (inbox) |
| Incoming pipeline filtering | validationd, spamfilterd, etc. |
| Storing objects/activities | domainservd / storaged |
| Delivering to followers | publisherd |
| HTTP signatures | publisherd (outbound), domainservd (inbound) |
| TLS certificates | oxifed-operator + cert-manager |
| **Package tracking** | **pkgfed (new)** |
| **Registry polling** | **pkgfed (new)** |
| **Release activity creation** | **pkgfed (new, via AMQP)** |

## Files to Create

| File | Description |
|------|-------------|
| `crates/pkgfed/Cargo.toml` | Crate manifest |
| `crates/pkgfed/src/main.rs` | Daemon entry point, AMQP setup, polling loop |
| `crates/pkgfed/src/registry.rs` | RegistryAdapter trait |
| `crates/pkgfed/src/adapters/crates_io.rs` | crates.io adapter |
| `crates/pkgfed/src/adapters/npm.rs` | npm adapter |
| `crates/pkgfed/src/adapters/rss.rs` | Generic RSS adapter |
| `crates/pkgfed/src/config.rs` | Configuration types |
| `src/database.rs` | Add PackageDocument type |
| `docker/pkgfed/Dockerfile` | Container image |

## Open Questions

1. **Actor type**: `Application` vs `Service`? Mastodon uses `Application` for
   bots. `Service` is for automated accounts. Either works -- `Application`
   may be more familiar to Mastodon users.

2. **Package naming**: `serde@packages.example.com` works, but what about
   scoped npm packages like `@types/node`? The `@` prefix conflicts with
   ActivityPub mention syntax. Options:
   - URL-encode: `types-node@packages.example.com`
   - Flatten: `types__node@packages.example.com`
   - Registry prefix: `npm-types-node@packages.example.com`

3. **Follow-to-track latency**: When a user follows a package that isn't tracked
   yet, there's a delay before pkgfed creates the actor and starts tracking.
   Should pkgfed pre-create actors for popular packages? Or is lazy creation OK?
   **Recommendation**: Lazy creation. The actor must exist in domainservd before
   it can be followed (WebFinger must resolve). So either:
   - Pre-seed popular package actors (batch job)
   - Or: pkgfed exposes a "request tracking" API that creates the actor first

4. **Rate limits on registry APIs**: crates.io requests a 1 req/sec rate limit.
   npm has similar constraints. pkgfed's polling must respect these.
   **Recommendation**: Batch checks, stagger requests, respect `Retry-After`.
