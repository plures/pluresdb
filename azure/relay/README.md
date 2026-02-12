# PluresDB Azure WSS Relay Server

Corporate-safe WebSocket relay server that runs on port 443 (looks like HTTPS traffic).

## Overview

The relay server enables P2P synchronization in corporate environments where:
- UDP is blocked (no Hyperswarm direct connections)
- Non-standard ports are blocked
- WebSocket traffic is inspected

By running on port 443 with TLS, the relay looks like normal HTTPS traffic and passes through most corporate firewalls.

## Architecture

```
┌─────────┐                          ┌─────────┐
│ Client A│─────wss:443────┐    ┌────│ Client B│
└─────────┘                 │    │    └─────────┘
                            ▼    ▼
                        ┌───────────┐
                        │   Relay   │
                        │  Server   │
                        └───────────┘
```

### How It Works

1. **Join Topic**: Clients connect and join a topic (namespace)
2. **Peer Matching**: Server groups peers by topic
3. **Data Relay**: Server forwards data between peers in the same topic
4. **Stateless**: No data storage - just pipes bytes

## Protocol

Messages are JSON over WebSocket:

```typescript
interface RelayMessage {
  type: "join" | "data" | "peer-joined" | "peer-left" | "error";
  peerId?: string;
  topic?: string;
  data?: string; // Base64-encoded binary data
}
```

### Client → Server

**Join Topic**:
```json
{
  "type": "join",
  "topic": "my-topic-hash",
  "peerId": "optional-peer-id"
}
```

**Send Data**:
```json
{
  "type": "data",
  "data": "base64-encoded-data"
}
```

### Server → Client

**Peer Joined**:
```json
{
  "type": "peer-joined",
  "peerId": "peer-123",
  "topic": "my-topic-hash"
}
```

**Peer Left**:
```json
{
  "type": "peer-left",
  "peerId": "peer-123",
  "topic": "my-topic-hash"
}
```

**Data from Peer**:
```json
{
  "type": "data",
  "peerId": "peer-123",
  "data": "base64-encoded-data"
}
```

**Error**:
```json
{
  "type": "error",
  "payload": "Error message"
}
```

## Deployment

### Local Development

```bash
cd azure/relay
npm install
npm run dev
```

Server starts on port 443 (requires sudo on Linux/Mac).

### Docker

```bash
# Build image
docker build -t plures/pluresdb-relay:latest -f azure/relay/Dockerfile .

# Run locally
docker run -p 443:443 plures/pluresdb-relay:latest
```

### Azure Container Instance

```bash
# Deploy relay infrastructure
az deployment group create \
  --resource-group pluresdb-relay-rg \
  --template-file azure/relay/relay.bicep \
  --parameters environment=prod

# Get relay URL
az deployment group show \
  --resource-group pluresdb-relay-rg \
  --name relay \
  --query properties.outputs.relayUrl.value
```

## Configuration

Environment variables:

- `PORT` - WebSocket port (default: 443)
- `NODE_ENV` - Environment (test/dev/prod)
- `ENABLE_TLS` - Enable TLS/SSL (default: true)

## Monitoring

The server logs:
- Peer joins/leaves
- Data relay events
- Topic statistics (every 5 minutes)
- Dead connection cleanup

Example logs:
```
[Relay] PluresDB WSS Relay Server listening on port 443
[Relay] Peer peer-abc joined topic topic-xyz (2 peers in topic)
[Relay] Relayed data from peer-abc to 1 peers in topic topic-xyz
[Relay] Stats: 3 topics, 12 total peers
[Relay]   - topic-xyz: 2 peers
[Relay]   - topic-123: 5 peers
[Relay]   - topic-456: 5 peers
```

## Security

- **No Data Storage**: Server doesn't store any data - just pipes bytes
- **Topic Isolation**: Peers only see others in the same topic
- **TLS Required**: All connections should use WSS (not WS)
- **Stateless**: Server can be restarted without data loss
- **No Authentication**: Topics act as shared secrets (use strong hashes)

## Performance

- **Horizontal Scaling**: Deploy multiple instances behind load balancer
- **Connection Limits**: Default Node.js limits apply (~65k connections per instance)
- **Cleanup**: Dead connections cleaned every 30 seconds
- **Stats**: Topic/peer stats logged every 5 minutes

## Troubleshooting

**Port 443 requires sudo**:
```bash
# On Linux/Mac, use authbind or setcap
sudo setcap 'cap_net_bind_service=+ep' $(which node)
```

**Connection refused**:
- Check firewall allows port 443
- Verify server is running: `docker ps` or `az container show`

**Peers not matching**:
- Verify both clients use same topic hash
- Check server logs for join events

## Next Steps

- [ ] Add TLS certificate support (Let's Encrypt)
- [ ] Add authentication/authorization
- [ ] Add rate limiting
- [ ] Add metrics export (Prometheus)
- [ ] Add horizontal scaling with Redis pub/sub
