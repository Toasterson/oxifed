# API Reference

HTTP endpoints exposed by domainservd. Default bind address: `0.0.0.0:8080`.

> **Note:** HTTP signature verification on incoming S2S requests is a placeholder that accepts all requests. OAuth endpoints are stubs.

## Endpoint Summary

### Discovery

| Method | Path | Auth | Status |
|--------|------|------|--------|
| GET | `/.well-known/webfinger?resource=acct:user@domain` | No | Implemented |
| GET | `/nodeinfo/2.0` | No | Implemented |
| GET | `/search` | No | Implemented |

### Actors

| Method | Path | Auth | Status |
|--------|------|------|--------|
| GET | `/users/{username}` | No | Implemented |
| GET | `/users` | No | Implemented |
| GET | `/users/{username}/followers` | No | Implemented |
| GET | `/users/{username}/following` | No | Implemented |
| GET | `/users/{username}/liked` | No | Implemented |
| GET | `/users/{username}/featured` | No | Implemented |
| GET | `/users/{username}/collections/featured` | No | Implemented |
| GET | `/users/{username}/collections/tags/{tag}` | No | Implemented |

### Server-to-Server (S2S)

| Method | Path | Auth | Status |
|--------|------|------|--------|
| POST | `/users/{username}/inbox` | HTTP Signature* | Implemented |
| GET/POST | `/users/{username}/outbox` | HTTP Signature* | Implemented |
| POST | `/inbox` | HTTP Signature* | Implemented (shared inbox) |

\* Signature verification is a placeholder -- all requests are accepted.

### Client-to-Server (C2S)

| Method | Path | Auth | Status |
|--------|------|------|--------|
| POST | `/users/{username}/notes` | None** | Implemented |
| POST | `/users/{username}/articles` | None** | Implemented |
| POST | `/users/{username}/media` | None** | Stub |

\** No authentication is enforced on C2S endpoints currently.

### Objects

| Method | Path | Auth | Status |
|--------|------|------|--------|
| GET | `/objects/{id}` | No | Implemented |
| PUT | `/objects/{id}` | No | Implemented |
| DELETE | `/objects/{id}` | No | Implemented |
| GET | `/activities/{id}` | No | Implemented |

### OAuth

| Method | Path | Auth | Status |
|--------|------|------|--------|
| GET | `/oauth/authorize` | No | Stub |
| POST | `/oauth/token` | No | Stub |
| POST | `/oauth/revoke` | No | Stub |

## Endpoint Details

### WebFinger

```
GET /.well-known/webfinger?resource=acct:alice@example.com
```

Returns a JRD (JSON Resource Descriptor) per RFC 7033.

Supported resource formats:
- `acct:user@domain` -- standard WebFinger format
- `act:user@domain` -- alias, converted to `acct:` internally
- `https://domain/users/username` -- HTTP URL lookup

Response: `application/jrd+json`

```bash
curl "http://localhost:8080/.well-known/webfinger?resource=acct:alice@example.com"
```

### NodeInfo

```
GET /nodeinfo/2.0
```

Returns NodeInfo 2.0 metadata about the instance.

```bash
curl http://localhost:8080/nodeinfo/2.0
```

### Actor Profile

```
GET /users/{username}
Accept: application/activity+json
```

Returns the ActivityPub actor document.

```bash
curl -H "Accept: application/activity+json" http://localhost:8080/users/alice
```

### Inbox (per-actor)

```
POST /users/{username}/inbox
Content-Type: application/activity+json
```

Receives incoming ActivityPub activities. Processes Follow, Like, Announce, Undo, Create, Update, Delete, Accept, and Reject activities.

### Shared Inbox

```
POST /inbox
Content-Type: application/activity+json
```

Shared inbox for activities addressed to multiple recipients on this server.

### Create Note (C2S)

```
POST /users/{username}/notes
Content-Type: application/json
```

Creates a Note object and publishes a Create activity.

### Create Article (C2S)

```
POST /users/{username}/articles
Content-Type: application/json
```

Creates an Article object and publishes a Create activity.

### Object Retrieval

```
GET /objects/{id}
Accept: application/activity+json
```

Returns the ActivityPub object by ID.

### Collections

```
GET /users/{username}/followers
GET /users/{username}/following
GET /users/{username}/outbox
GET /users/{username}/liked
GET /users/{username}/featured
```

Returns OrderedCollection or OrderedCollectionPage documents. Pagination is incomplete in some endpoints.
