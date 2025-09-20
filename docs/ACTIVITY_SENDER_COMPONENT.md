# Activity Sender Component Documentation

## Overview

The Activity Sender Component implements the Client-to-Server (C2S) API portion of the ActivityPub protocol, allowing authenticated clients to create, update, and delete content on behalf of users. This component handles outgoing activities from local users to the fediverse.

## Architecture

The Activity Sender Component is integrated into the `domainservd` service and provides REST endpoints for client applications to interact with the ActivityPub network. It works in conjunction with the `publisherd` service for activity delivery.

## Authentication

The component uses Bearer token authentication. Clients must include an Authorization header with a valid access token:

```
Authorization: Bearer <access_token>
```

### OAuth 2.0 Support

The component includes OAuth 2.0 endpoints for third-party application authentication:

- `GET /oauth/authorize` - Authorization endpoint
- `POST /oauth/token` - Token exchange endpoint  
- `POST /oauth/revoke` - Token revocation endpoint

## API Endpoints

### Core Activity Endpoints

#### POST /users/{username}/outbox
Submit an activity to the user's outbox. This is the main endpoint for C2S interactions.

**Request:**
```json
{
  "type": "Create",
  "object": {
    "type": "Note",
    "content": "Hello, Fediverse!"
  }
}
```

**Response:** 201 Created with Location header pointing to the new activity

### Direct Object Creation Endpoints

#### POST /users/{username}/notes
Create a new note (status/post).

**Request:**
```json
{
  "content": "This is my note",
  "to": ["https://www.w3.org/ns/activitystreams#Public"],
  "cc": ["https://example.com/users/alice/followers"],
  "sensitive": false,
  "summary": null,
  "tag": [],
  "attachment": []
}
```

**Response:** 201 Created with Location header

#### POST /users/{username}/articles
Create a new article (long-form content).

**Request:**
```json
{
  "name": "My Article Title",
  "content": "Article content in HTML or Markdown",
  "summary": "Brief summary of the article",
  "to": ["https://www.w3.org/ns/activitystreams#Public"],
  "tag": ["technology", "federated"]
}
```

**Response:** 201 Created with Location header

#### POST /users/{username}/media
Upload media files for use in posts.

**Request:** Binary file upload with Content-Type header

**Response:**
```json
{
  "type": "Document",
  "mediaType": "image/png",
  "url": "https://example.com/media/abc123"
}
```

### Object Management Endpoints

#### PUT /objects/{id}
Update an existing object (must be owned by authenticated user).

**Request:**
```json
{
  "content": "Updated content",
  "summary": "Updated summary"
}
```

**Response:** 204 No Content

#### DELETE /objects/{id}
Delete an object (must be owned by authenticated user).

**Response:** 204 No Content

### Collection Endpoints

#### GET /users/{username}/collections/featured
Get the user's featured/pinned posts.

**Query Parameters:**
- `page` - Page number for pagination
- `limit` - Number of items per page (max 100)

**Response:**
```json
{
  "@context": "https://www.w3.org/ns/activitystreams",
  "type": "OrderedCollection",
  "id": "https://example.com/users/alice/collections/featured",
  "totalItems": 5,
  "orderedItems": [...]
}
```

#### GET /users/{username}/collections/tags/{tag}
Get posts with a specific hashtag.

**Response:** OrderedCollection of posts with the specified tag

### Discovery Endpoints

#### GET /search
Search for content across the instance.

**Query Parameters:**
- `q` - Search query

**Response:**
```json
{
  "type": "Collection",
  "totalItems": 10,
  "items": [...]
}
```

#### GET /users
List users on the instance.

**Query Parameters:**
- `limit` - Number of results (max 100)

**Response:** Collection of user actors

## Supported Activity Types

The Activity Sender Component currently supports the following activity types:

- **Create** - Create new objects (notes, articles, etc.)
- **Update** - Update existing objects
- **Delete** - Delete objects
- **Follow** - Follow another actor
- **Undo** - Undo a previous activity (unfollow, unlike, etc.)
- **Like** - Like an object
- **Announce** - Share/boost an object
- **Block** - Block another actor

## Processing Flow

1. **Authentication**: Client authenticates via OAuth or API token
2. **Validation**: Request is validated for required fields and permissions
3. **Processing**: Activity is processed based on type:
   - Object activities create/update/delete the object
   - Social activities update relationships
4. **Storage**: Activity is stored in MongoDB
5. **Outbox**: Activity is added to the actor's outbox
6. **Publishing**: Activity is published to the message queue for delivery
7. **Delivery**: The `publisherd` service delivers the activity to followers

## Security Considerations

- All C2S endpoints require authentication
- Users can only modify their own objects
- Private key access is restricted to the server
- Token expiration is enforced
- Rate limiting should be implemented (TODO)

## Database Schema

### Access Tokens Collection
```javascript
{
  token: String,
  username: String,
  client_id: String,
  created_at: DateTime,
  expires_at: DateTime,
  scopes: [String]
}
```

### Outbox Collection
```javascript
{
  actor: String,        // Actor URL
  activity_id: String,  // Activity URL
  created_at: DateTime
}
```

### Media Collection
```javascript
{
  id: String,           // Media URL
  uploadedBy: String,   // Actor URL
  contentType: String,
  size: Number,
  uploadedAt: DateTime
}
```

## Configuration

The Activity Sender Component uses the following environment variables:

- `OXIFED_DOMAIN` - The domain name of the instance (default: localhost)
- `MONGODB_URI` - MongoDB connection string
- `MONGODB_DBNAME` - Database name

## Implementation Status

### Completed
✅ Basic C2S endpoints structure
✅ OAuth 2.0 endpoints
✅ Note and Article creation
✅ Media upload endpoint
✅ Object update/delete
✅ Follow activity support
✅ Collections and pagination
✅ Search functionality
✅ Token-based authentication

### TODO
- [ ] Full OAuth 2.0 flow implementation
- [ ] Rate limiting
- [ ] Input sanitization
- [ ] File storage backend for media
- [ ] Scheduled post support
- [ ] Draft support
- [ ] Polls/Questions
- [ ] Edit history
- [ ] Client application management UI
- [ ] Scope-based permissions
- [ ] WebAuthn support

## Testing

Test files are located in `/tests/test_activity_sender.rs` and cover:

- Note creation via C2S
- Object updates and deletion
- OAuth token flow
- Outbox management
- Follow activities
- Collection pagination

Run tests with:
```bash
cargo test test_activity_sender
```

## Client Implementation Guide

### Example: Creating a Post

```javascript
const createPost = async (content, token) => {
  const response = await fetch('https://example.com/users/alice/notes', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      content: content,
      to: ['https://www.w3.org/ns/activitystreams#Public'],
      cc: ['https://example.com/users/alice/followers']
    })
  });
  
  if (response.status === 201) {
    const location = response.headers.get('Location');
    console.log('Post created:', location);
  }
};
```

### Example: Following a User

```javascript
const followUser = async (targetActor, token) => {
  const response = await fetch('https://example.com/users/alice/outbox', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${token}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      type: 'Follow',
      object: targetActor
    })
  });
  
  return response.status === 201;
};
```

## Troubleshooting

### Common Issues

1. **401 Unauthorized**: Check that the token is valid and not expired
2. **403 Forbidden**: User doesn't have permission for the requested action
3. **400 Bad Request**: Check request format and required fields
4. **404 Not Found**: Object or endpoint doesn't exist

### Debug Logging

Enable debug logging with:
```bash
RUST_LOG=debug domainservd
```

## References

- [ActivityPub Specification](https://www.w3.org/TR/activitypub/)
- [OAuth 2.0 RFC](https://tools.ietf.org/html/rfc6749)
- [ActivityStreams 2.0](https://www.w3.org/TR/activitystreams-core/)