# snac2 Federation Test Environment

## Overview
Successfully set up a snac2 ActivityPub server for federation testing using Docker.

## Setup Instructions

### 1. Clone and Build
```bash
git clone https://codeberg.org/grunfink/snac2.git
cd snac2
docker-compose up -d --build
```

### 2. Services Running
- **snac2 server**: Port 8001 (direct access)
- **nginx-alpine-ssl proxy**: Ports 80/443 (HTTPS with self-signed certs)
- **Data volume**: `./data` directory (persistent storage)

## Test User Credentials
- **Username**: `testuser`
- **Password**: `yFVV7tHGB3CgSvwd`
- **Actor URL**: `https://localhost/testuser`
- **Configuration**: https://localhost/testuser (web interface)

## Federation Endpoints

### WebFinger Discovery
```bash
# Direct access (port 8001)
curl "http://localhost:8001/.well-known/webfinger?resource=acct:testuser@localhost"

# Through nginx proxy (port 80)
curl "http://localhost/.well-known/webfinger?resource=acct:testuser@localhost"
```

### ActivityPub Actor
```bash
# Direct access
curl -H "Accept: application/activity+json" "http://localhost:8001/testuser"

# Through proxy
curl -H "Accept: application/activity+json" "http://localhost/testuser"
```

### NodeInfo
```bash
# Discovery endpoint
curl "http://localhost/.well-known/nodeinfo"

# Actual nodeinfo
curl "http://localhost:8001/nodeinfo_2_0"
```

### Key Endpoints for Federation
- **Actor**: `https://localhost/testuser`
- **Inbox**: `https://localhost/testuser/inbox`
- **Outbox**: `https://localhost/testuser/outbox`
- **Shared Inbox**: `https://localhost/shared-inbox`
- **WebFinger**: `https://localhost/.well-known/webfinger`
- **NodeInfo**: `https://localhost/.well-known/nodeinfo`

## Starting/Stopping the Instance

### Start
```bash
cd snac2
docker-compose up -d
```

### Stop
```bash
cd snac2
docker-compose down
```

### View Logs
```bash
# All services
docker-compose logs

# Just snac2
docker-compose logs snac

# Follow logs
docker-compose logs -f
```

## Configuration Notes

### Server Configuration
- **Host**: `localhost`
- **Protocol**: `https` (configured for SSL through nginx)
- **Port**: 8001 (internal), 80/443 (external through nginx)
- **Data Directory**: `/data/data` (in container), `./data/data` (on host)

### For Local Federation Testing
1. **Use `localhost` as domain**: Already configured
2. **Self-signed SSL**: Pre-configured certificates included
3. **Accept invalid certificates**: Other federation software will need to accept self-signed certs for testing

### Database
- **No external database required**: snac2 uses file-based storage
- **Data persisted** in `./data` directory
- **Backup**: Simply copy the `./data` directory

## Verified Working Features
✅ WebFinger discovery  
✅ ActivityPub actor document  
✅ Inbox/Outbox endpoints  
✅ NodeInfo protocol  
✅ HTTPS proxy with nginx  
✅ User account creation  
✅ Web interface access  

## Next Steps for Federation Testing
1. **Configure external access**: Update host configuration if testing with remote instances
2. **Certificate setup**: Replace self-signed certs for production federation testing
3. **User setup**: Complete user profile configuration at https://localhost/testuser
4. **Test posts**: Create test posts through the web interface
5. **Federation testing**: Try following/being followed by other ActivityPub instances

## Configuration Tweaks for Federation

### To enable external federation:
1. Update `server.json` with your actual domain
2. Replace self-signed certificates with valid ones
3. Configure DNS/port forwarding
4. Update nginx configuration for your domain

### Current Docker Compose Configuration:
- Automatic initialization with test user
- Volume persistence for data
- Network isolation
- SSL termination at nginx layer
- Health monitoring ready

## Troubleshooting
- **Check container status**: `docker-compose ps`
- **View logs**: `docker-compose logs snac`
- **Restart**: `docker-compose restart`
- **Clean restart**: `docker-compose down && docker-compose up -d`
- **Access container**: `docker-compose exec snac sh`

## Security Notes
⚠️ **This is a TEST ENVIRONMENT**:
- Uses self-signed certificates
- Default passwords shown in logs
- No rate limiting configured
- Suitable for development/testing only

For production use, update certificates, change passwords, and review security configurations.