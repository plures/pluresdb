# PluresDB Sync Transport System

Pluggable transport layer for P2P synchronization with corporate-safe fallback options.

## Overview

PluresDB supports three sync transport modes:

| Priority | Transport | Environment | Port | Details |
|----------|-----------|-------------|------|---------|
| **Primary** | Azure relay | Corporate / anywhere | 443 | WSS on port 443, looks like normal HTTPS traffic |
| **Backup** | Vercel relay | Corporate / anywhere | 443 | Edge WebSocket functions, `*.vercel.app` domain |
| **Direct** | Hyperswarm | Home/personal only | Various | DHT + UDP holepunching — triggers corporate IDS |

## Why Multiple Transports?

**Corporate Networks** block UDP and non-standard ports:
- Hyperswarm uses UDP holepunching + DHT (looks like BitTorrent)
- Corporate IDS/firewalls flag this traffic
- Direct P2P connections fail in office environments

**Solution**: WSS relay on port 443:
- Looks like normal HTTPS traffic
- Uses standard WebSocket protocol
- Passes through corporate firewalls
- No special network configuration needed

## Architecture

### Direct Transport (Home/Personal)

```
Client A ◄────UDP holepunching────► Client B
           (best throughput)
```

**Pros**:
- Best performance (no relay)
- No third-party infrastructure
- True P2P

**Cons**:
- Blocked in corporate networks
- Flagged by IDS systems
- UDP often blocked

### Relay Transport (Corporate-Safe)

```
Phase 1: Discovery via relay
Client A ──wss:443──► Relay ◄──wss:443── Client B
                     (match by topic)

Phase 2: Direct upgrade (if possible)
Client A ◄────direct connection────► Client B
              (relay dropped)

Phase 3: Fallback (if direct fails)
Client A ──wss:443──► Relay ──wss:443──► Client B
                     (relay pipes bytes)
```

**Pros**:
- Works in corporate networks
- Looks like HTTPS traffic
- No firewall configuration needed
- Stateless relay (horizontally scalable)

**Cons**:
- Requires relay infrastructure
- Slightly higher latency
- Bandwidth limited by relay

### Auto Transport (Recommended Default)

Tries transports in order with automatic fallback:

1. **Direct** - Try Hyperswarm first (best performance)
2. **Azure Relay** - If UDP blocked, use Azure relay
3. **Vercel Relay** - If Azure unavailable, use Vercel

## Usage

### Basic Usage

```typescript
import { createTransport } from "@plures/pluresdb/sync";

// Auto transport (recommended)
const transport = createTransport({
  mode: "auto",
  azureRelayUrl: "wss://pluresdb-relay.azurewebsites.net",
  vercelRelayUrl: "wss://pluresdb-relay.vercel.app",
});

// Listen for connections
await transport.listen((connection) => {
  console.log("New peer connected");
  
  // Send data
  await connection.send(new Uint8Array([1, 2, 3]));
  
  // Receive data
  for await (const data of connection.receive()) {
    console.log("Received:", data);
  }
});

// Connect to peer
const connection = await transport.connect("peer-id-or-url");
await connection.send(new Uint8Array([4, 5, 6]));
```

### Specific Transport

```typescript
// Direct only (Hyperswarm)
const directTransport = createTransport({
  mode: "direct",
});

// Azure relay only
const azureTransport = createTransport({
  mode: "azure-relay",
  azureRelayUrl: "wss://my-relay.azurewebsites.net",
});

// Vercel relay only
const vercelTransport = createTransport({
  mode: "vercel-relay",
  vercelRelayUrl: "wss://my-relay.vercel.app",
});
```

### Configuration

```typescript
interface TransportConfig {
  mode: "auto" | "azure-relay" | "vercel-relay" | "direct";
  azureRelayUrl?: string;
  vercelRelayUrl?: string;
  syncKey?: string;
  connectionTimeoutMs?: number;
}

const defaultConfig: TransportConfig = {
  mode: "auto",
  azureRelayUrl: "wss://pluresdb-relay.azurewebsites.net",
  vercelRelayUrl: "wss://pluresdb-relay.vercel.app",
  connectionTimeoutMs: 30000,
};
```

## Transport API

### SyncTransport Interface

```typescript
interface SyncTransport {
  readonly name: string;
  connect(peerId: string): Promise<SyncConnection>;
  listen(onConnection: (conn: SyncConnection) => void): Promise<void>;
  close(): Promise<void>;
}
```

### SyncConnection Interface

```typescript
interface SyncConnection {
  send(data: Uint8Array): Promise<void>;
  receive(): AsyncIterable<Uint8Array>;
  close(): Promise<void>;
}
```

## Relay Server

### Deploy Azure Relay

```bash
# Deploy relay infrastructure
cd azure
az deployment group create \
  --resource-group pluresdb-relay-rg \
  --template-file relay/relay.bicep \
  --parameters environment=prod

# Get relay URL
az deployment group show \
  --resource-group pluresdb-relay-rg \
  --name relay \
  --query properties.outputs.relayUrl.value
```

### Deploy Vercel Relay

```bash
# Deploy to Vercel
cd azure/relay
vercel deploy --prod
```

### Run Locally

```bash
# Start relay server
cd azure/relay
npm install
npm run dev

# Server listens on port 443 (requires sudo)
```

## Testing

### Unit Tests

```bash
# Run transport tests
deno test -A --unstable-kv legacy/tests/unit/sync-transport.test.ts
```

### Integration Tests

```bash
# Start relay server
cd azure/relay
npm run dev

# Run integration tests
RUN_RELAY_TESTS=true deno test -A --unstable-kv legacy/tests/integration/relay-integration.test.ts
```

### Azure Tests

```bash
# Run Azure relay tests (requires deployed infrastructure)
npm run test:azure:relay
```

## Security

### Data Privacy

- **End-to-End Encryption**: Data is encrypted before sending
- **Relay is Blind**: Relay server can't read encrypted data
- **Topic Isolation**: Peers only see others in the same topic
- **No Storage**: Relay doesn't store any data

### Network Security

- **TLS Required**: All relay connections use WSS (WebSocket Secure)
- **Port 443**: Standard HTTPS port, no special firewall rules
- **Corporate-Safe**: Traffic looks like normal web requests

### Topic Security

Topics act as shared secrets:
- Use strong hash functions (SHA-256)
- Don't expose topics in URLs or logs
- Rotate topics periodically for sensitive data

## Performance

### Throughput Benchmarks

| Transport | Latency | Throughput | Notes |
|-----------|---------|------------|-------|
| Direct | ~5ms | 100+ Mbps | Best performance |
| Azure Relay | ~20ms | 10-50 Mbps | Same Azure region |
| Vercel Relay | ~30ms | 5-20 Mbps | Edge network |

### Scalability

- **Direct**: Limited by peer count (DHT overhead)
- **Relay**: Horizontally scalable (stateless design)
- **Auto**: Combines best of both approaches

## Troubleshooting

### Connection Failures

**Direct transport fails**:
- UDP likely blocked
- NAT traversal may fail
- Try relay transport instead

**Relay connection timeout**:
- Check relay server is running
- Verify relay URL is correct
- Test network connectivity: `curl -I https://relay-url`

**All transports fail**:
- Check internet connectivity
- Verify firewall allows outbound HTTPS
- Check relay server logs for errors

### Performance Issues

**High latency**:
- Use direct transport if possible
- Deploy relay closer to clients
- Check network path with `traceroute`

**Low throughput**:
- Relay may be overloaded
- Deploy additional relay instances
- Use CDN for relay (Vercel/Cloudflare)

## Next Steps

- [ ] Implement Vercel relay transport
- [ ] Add connection upgrade (relay → direct)
- [ ] Add metrics and monitoring
- [ ] Add relay authentication
- [ ] Add rate limiting
- [ ] Add horizontal scaling with Redis

## References

- [Azure Relay Documentation](../azure/relay/README.md)
- [WebSocket Protocol](https://tools.ietf.org/html/rfc6455)
- [Hyperswarm Documentation](https://github.com/hyperswarm/hyperswarm)
