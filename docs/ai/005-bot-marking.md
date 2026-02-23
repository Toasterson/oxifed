# 005: Bot Account Marking and Filtering

**Status:** Planning
**Created:** 2026-02-24
**Last Updated:** 2026-02-24

## Summary

Implement proper bot/automated account support in oxifed so that:
1. Bot accounts (like Akh-medu conversational AI actors) are clearly marked
2. Users on other servers can identify and filter bots if they choose
3. Bots can still follow and read information (opt-out doesn't block reads)
4. The marking is compatible with Mastodon, GoToSocial, Pleroma/Akkoma

## Context: Akh-medu Integration

Akh-medu is a Vector Symbolic AI Engine for conversational bots. When deployed
with oxifed, Akh-medu actors should:
- Be clearly marked as automated (`type: Service`)
- Have a visible link to the human operator
- Be able to follow other accounts and read their posts
- Respect `#nobot` preferences — don't interact with users who opt out
- Default to `unlisted` visibility to avoid flooding public timelines
- Be filterable by remote servers and users

Users who don't want to participate in AI conversations should be able to
filter bot interactions, but bots should still be able to read public content
(the outbox is public; you don't need to follow to read it).

## Research: Fediverse Bot Conventions

### Actor Types (ActivityStreams 2.0)

| Type | Usage | Convention |
|------|-------|-----------|
| `Person` | Human users | Default for regular accounts |
| `Service` | Bot/automated accounts | De-facto standard for user-facing bots |
| `Application` | Instance system actor | Reserved for server-level actors (e.g., `/actor`) |
| `Group` | Community accounts | Used by Lemmy for communities |
| `Organization` | Org accounts | Rarely used in practice |

**Convention**: `Service` = bot. All major implementations (Mastodon, GoToSocial,
Pleroma, Misskey) treat `type: Service` as the bot flag. `Application` is reserved
for the instance actor.

### Mastodon's Approach

- Actor `type: Service` -> displays "Automated" badge (robot icon)
- API field `bot: true/false` on Account entity
- `PATCH /api/v1/accounts/update_credentials` accepts `bot` boolean
- No built-in "hide all bots" user preference (requested but not implemented)
- Users must mute/block individual bots
- `discoverable` field controls profile directory inclusion
- `indexable` field controls full-text search inclusion

### Mastodon Namespace Extensions

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://w3id.org/security/v1",
    {
      "discoverable": "http://joinmastodon.org/ns#discoverable",
      "indexable": "http://joinmastodon.org/ns#indexable"
    }
  ]
}
```

### GoToSocial Interaction Policies

GoToSocial adds per-post `interactionPolicy` controlling who can like, reply,
or boost each post. This provides fine-grained control that could exclude bots:

```json
{
  "interactionPolicy": {
    "canReply": {
      "automaticApproval": ["followers_collection"],
      "manualApproval": ["as:Public"]
    }
  }
}
```

### Operator Linking

No standardized property for "this bot is operated by X." Current approaches:
1. Profile bio text: "Operated by @human@example.com" (most common)
2. Profile metadata PropertyValue fields
3. `alsoKnownAs`: semantically "same entity," not "operated by"
4. `attributedTo`: theoretically correct ("created by") but unused for actors
5. Proposed: dedicated `automatedBy` field (Mastodon issue #28994, unimplemented)

### `#nobot` Convention

Community convention where users put `#nobot` in their bio to opt out of
all bot interactions. Bots are expected to:
- Check target actor's `summary` (bio) for `#nobot` before interacting
- Not follow, reply to, boost, or like posts from `#nobot` users
- `#nobot` implies `#noindex` (don't index their content either)
- Per-post `#nobot` in content means "don't reply to this post"

### Bot Etiquette

1. Mark as bot (`type: Service`)
2. Identify operator (in bio or metadata)
3. Respect `#nobot` and blocks
4. Default to `unlisted` visibility (don't spam public timelines)
5. Don't follow without consent (wait for user to follow first, or explicit opt-in)
6. Provide unsubscribe instructions
7. Rate limit posting (community guideline: ~3 posts/day for timeline bots)

## Implementation Plan

### Phase 1: Actor Type Support in Database and API

#### 1a. ActorDocument Fields

Add to `ActorDocument` in `src/database.rs`:

```rust
/// Whether this account is automated (derived from actor_type == Service)
pub bot: bool,

/// Opt into discovery features (Mastodon compat)
pub discoverable: Option<bool>,

/// Allow full-text search indexing (Mastodon compat)
pub indexable: Option<bool>,

/// Human operator's actor URI (non-standard, stored in metadata)
pub operated_by: Option<String>,
```

The `actor_type` field already exists as `String`. Ensure it defaults to
`"Person"` for regular accounts and is set to `"Service"` for bots.

#### 1b. Actor Serialization

When serving actor JSON via ActivityPub endpoints, include:

```json
{
  "@context": [
    "https://www.w3.org/ns/activitystreams",
    "https://w3id.org/security/v1",
    {
      "discoverable": "http://joinmastodon.org/ns#discoverable",
      "indexable": "http://joinmastodon.org/ns#indexable"
    }
  ],
  "type": "Service",
  "discoverable": true,
  "indexable": false,
  "attachment": [
    {
      "type": "PropertyValue",
      "name": "Operated by",
      "value": "<a href=\"https://example.com/users/human\">@human@example.com</a>"
    },
    {
      "type": "PropertyValue",
      "name": "Source",
      "value": "<a href=\"https://akh-medu.example\">Akh-medu AI Engine</a>"
    }
  ]
}
```

#### 1c. Inbound Actor Processing

When fetching/processing remote actors in `client.rs` and `domainservd`:
- If `type` is `Service` or `Application` -> set `bot: true`
- If `type` is `Person`, `Group`, `Organization` -> set `bot: false`
- Parse and store `discoverable`, `indexable`, `alsoKnownAs`, `movedTo`

### Phase 2: Bot Account Creation via oxiadm/adminservd

#### 2a. Profile Creation with Bot Flag

Extend `ProfileCreateMessage` in `messaging.rs`:

```rust
pub struct ProfileCreateMessage {
    pub subject: String,
    pub summary: Option<String>,
    pub icon: Option<String>,
    pub properties: Option<Value>,
    /// Actor type: "Person" (default), "Service" (bot), "Application"
    pub actor_type: Option<String>,
    /// URI of the human operator's account
    pub operated_by: Option<String>,
    /// Opt into discovery
    pub discoverable: Option<bool>,
}
```

#### 2b. oxiadm Command

```
oxiadm person create bot@example.com \
  --bot \
  --operated-by human@example.com \
  --summary "An Akh-medu conversational AI. Operated by @human@example.com"
```

The `--bot` flag sets `actor_type: "Service"`. The `--operated-by` adds the
operator link as a profile metadata field.

#### 2c. adminservd Route

`POST /api/v1/persons` already accepts a JSON body. Add optional fields:
```json
{
  "subject": "bot@example.com",
  "summary": "An Akh-medu AI",
  "actor_type": "Service",
  "operated_by": "human@example.com",
  "discoverable": true
}
```

### Phase 3: `#nobot` Enforcement in publisherd

When publisherd delivers activities on behalf of a bot account (`actor_type: Service`):

1. **Before following**: Check target actor's `summary` field in MongoDB
   (fetched during actor resolution) for `#nobot`. If present, skip delivery.
2. **Before replying**: Same check on the author of the post being replied to.
3. **Before boosting/liking**: Same check.
4. **Cache**: Cache `#nobot` status per-actor with TTL (actor profiles are
   already fetched for inbox resolution).

Implementation in `build_signing_client` or `process_activity`:

```rust
// If the sending actor is a bot, check recipient for #nobot
if sender_is_bot {
    if let Some(ref summary) = recipient_actor.summary {
        let summary_lower = summary.to_lowercase();
        if summary_lower.contains("#nobot") {
            info!(
                "Skipping delivery to {} (has #nobot in bio)",
                recipient_url
            );
            continue;
        }
    }
}
```

### Phase 4: Bot Visibility Defaults

When a bot account creates a post (via Akh-medu -> AMQP -> domainservd):

1. **Default visibility**: If no visibility is specified, bot accounts default
   to `unlisted` instead of `public`. This means:
   - `to: [followers_collection]`
   - `cc: [as:Public]`
   - Posts appear to followers but NOT on local/federated public timelines
2. **Configurable**: The operator can override per-post if needed.

In `domainservd/rabbitmq.rs`, when processing `NoteCreateMessage`:

```rust
// Default bot posts to unlisted
let visibility = if actor_doc.bot && note_msg.visibility.is_none() {
    "unlisted"
} else {
    note_msg.visibility.as_deref().unwrap_or("public")
};
```

### Phase 5: Bot Filtering in the Pipeline

Add bot-awareness to the moderation pipeline stage (`moderationd`):

#### 5a. Per-Domain Bot Policy

Extend `DomainSpec` filters:

```yaml
apiVersion: oxifed.io/v1alpha1
kind: Domain
metadata:
  name: example-com
spec:
  hostname: example.com
  filters:
    bot_policy: allow         # allow | silence | reject
    # allow: bots can interact normally
    # silence: bot interactions are accepted but hidden from public timelines
    # reject: reject all activities from bot accounts (except reads)
```

#### 5b. Moderation Stage Check

In `moderationd`, when processing an incoming activity:

```rust
if source_actor.bot {
    match domain_config.bot_policy {
        BotPolicy::Reject => {
            // Reject the activity (but note: Follows are still accepted
            // so the bot can read the public outbox)
            return StageResult::Reject {
                reason: "Bot interactions rejected by domain policy".into(),
                message,
            };
        }
        BotPolicy::Silence => {
            // Rewrite: downgrade visibility to unlisted
            // (accepted but hidden from public timelines)
            return StageResult::Rewrite {
                reason: "Bot activity silenced by domain policy".into(),
                message: downgrade_visibility(message),
            };
        }
        BotPolicy::Allow => {
            // Pass through normally
        }
    }
}
```

#### 5c. Follow Exception

Even when `bot_policy: reject`, Follow activities from bots should be
**accepted** (auto-accept or manual approval depending on domain config).
The bot needs to follow to receive posts in its inbox for reading.
The rejection applies to Create, Announce, Like — interactive activities.

This means a bot can:
- Follow accounts (receive posts)
- Read public outboxes (no follow needed)
- NOT reply, boost, or like when rejected

### Phase 6: User-Level Bot Filtering (Future)

Per-user preferences stored in `ActorDocument`:

```rust
/// User preference for bot interactions
pub bot_filter: Option<BotFilter>,
```

```rust
pub enum BotFilter {
    /// Accept all bot interactions (default)
    Allow,
    /// Require manual approval for bot interactions
    ManualApproval,
    /// Reject all bot interactions
    Reject,
}
```

Enforced in the relationship verification pipeline stage (`relationshipd`):
- Check target actor's `bot_filter` preference
- If `ManualApproval`: hold in pending interactions queue
- If `Reject`: reject the activity

### Phase 7: Mastodon API Compatibility (Future)

If/when oxifed implements a Mastodon-compatible client API:

- `GET /api/v1/accounts/:id` returns `bot: true/false`
- `PATCH /api/v1/accounts/update_credentials` accepts `bot: true/false`
  and `discoverable: true/false`
- Map `bot: true` -> `actor_type = "Service"` internally
- Map `bot: false` -> `actor_type = "Person"` internally

## Akh-medu Integration Pattern

Akh-medu interacts with oxifed purely through AMQP, the same way oxiadm
uses adminservd:

```
Akh-medu AI Engine
    |
    | AMQP: publish ProfileCreateMessage (actor_type: "Service")
    |        publish NoteCreateMessage (reply to conversation)
    |        publish FollowActivityMessage (follow information sources)
    v
EXCHANGE_INTERNAL_PUBLISH
    |
    v
domainservd (processes commands, stores actors/objects)
    |
    v
publisherd (delivers to remote inboxes with HTTP signatures)
```

Akh-medu never deals with HTTP signatures, WebFinger, or ActivityPub protocol
details. It just publishes messages to AMQP and domainservd/publisherd handle
the federation complexity.

### Akh-medu Actor Example

```
Username: akh@ai.example.com
Type: Service
Bio: "I'm Akh-medu, a conversational AI powered by Vector Symbolic Architecture.
     Operated by @admin@example.com. I follow accounts to learn from conversations.
     If you don't want to interact with me, add #nobot to your bio."
Metadata:
  - Operated by: @admin@example.com
  - Engine: Akh-medu VSA
  - Source: https://akh-medu.example
```

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/database.rs` | Modify | Add `bot`, `discoverable`, `indexable`, `operated_by` to ActorDocument |
| `src/messaging.rs` | Modify | Add `actor_type`, `operated_by`, `discoverable` to ProfileCreateMessage |
| `src/lib.rs` | Modify | Ensure `discoverable`/`indexable` in actor JSON-LD context |
| `crates/domainservd/src/activitypub.rs` | Modify | Serialize bot fields in actor JSON output |
| `crates/domainservd/src/rabbitmq.rs` | Modify | Handle bot fields in profile create, default visibility |
| `crates/publisherd/src/main.rs` | Modify | `#nobot` check before delivery for bot actors |
| `crates/oxiadm/src/main.rs` | Modify | Add `--bot` and `--operated-by` flags to person create |
| `crates/adminservd/src/routes/` | Modify | Accept bot fields in person create API |
| `crates/moderationd/` | Modify | Bot policy enforcement in moderation stage |
| `crates/oxifed-operator/src/main.rs` | Modify | Add `bot_policy` to DomainSpec filters |

## Open Questions

1. **Should `Application` type actors be treated differently from `Service`?**
   Mastodon treats both as bots. GoToSocial reserves `Application` for the
   instance actor. **Recommendation**: Follow GoToSocial's convention. Only
   use `Application` for instance-level actors. Use `Service` for user-facing
   bots. Treat both as `bot: true` when processing remote actors.

2. **Should #nobot checking happen in publisherd or in the pipeline?**
   For outbound: publisherd (before delivery).
   For inbound: moderation pipeline stage (before storage).
   **Recommendation**: Both. Outbound #nobot in publisherd prevents sending
   to users who opted out. Inbound bot_policy in moderationd prevents
   unwanted bot content from being stored.

3. **How should Akh-medu handle rate limiting?**
   Bot accounts should have stricter rate limits than human accounts.
   **Recommendation**: Add `rate_limit_multiplier` to bot accounts
   (default 0.5x = half the rate of human accounts). Enforced in spamfilterd.
