# PluresDB Azure Infrastructure Architecture

## Overview

This document describes the architecture of the PluresDB Azure testing infrastructure, designed to validate P2P relay functionality across multiple cloud-hosted nodes.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Azure Resource Group                          │
│                     (pluresdb-{env}-rg)                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │                   Virtual Network                           │   │
│  │                  (10.0.0.0/16)                             │   │
│  │                                                             │   │
│  │  ┌──────────────────────────────────────────────────────┐  │   │
│  │  │              Subnet (10.0.1.0/24)                    │  │   │
│  │  │                                                       │  │   │
│  │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐          │  │   │
│  │  │  │  Node 0  │  │  Node 1  │  │  Node 2  │          │  │   │
│  │  │  │Container │  │Container │  │Container │   ...    │  │   │
│  │  │  │Instance  │  │Instance  │  │Instance  │          │  │   │
│  │  │  │          │  │          │  │          │          │  │   │
│  │  │  │ :34567   │  │ :34567   │  │ :34567   │          │  │   │
│  │  │  │ :34568   │  │ :34568   │  │ :34568   │          │  │   │
│  │  │  └─────┬────┘  └─────┬────┘  └─────┬────┘          │  │   │
│  │  │        │             │             │                │  │   │
│  │  │        └─────────────┼─────────────┘                │  │   │
│  │  │                      │                              │  │   │
│  │  │              P2P Mesh Network                       │  │   │
│  │  └──────────────────────────────────────────────────────┘  │   │
│  │                                                             │   │
│  │  ┌──────────────────────────────────────────────────────┐  │   │
│  │  │           Network Security Group                     │  │   │
│  │  │  - Allow :34567 (P2P)                               │  │   │
│  │  │  - Allow :34568 (API)                               │  │   │
│  │  │  - Allow :22 (SSH)                                  │  │   │
│  │  └──────────────────────────────────────────────────────┘  │   │
│  └────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │                   Storage Account                           │   │
│  │  - Logs                                                     │   │
│  │  - Persistent data (optional)                              │   │
│  │  - Backups                                                  │   │
│  └────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Components

### 1. Resource Group

Each environment (test, dev, prod) has its own resource group:
- **Naming**: `pluresdb-{environment}-rg`
- **Purpose**: Logical container for all resources
- **Tags**: 
  - `environment`: test/dev/prod
  - `project`: pluresdb

### 2. Virtual Network (VNet)

Provides network isolation and internal communication:
- **Address Space**: 10.0.0.0/16
- **Subnet**: 10.0.1.0/24 (supports up to 251 nodes)
- **DNS**: Azure-provided DNS

### 3. Network Security Group (NSG)

Controls inbound/outbound traffic:

| Priority | Rule Name        | Direction | Port  | Protocol | Purpose               |
|----------|------------------|-----------|-------|----------|-----------------------|
| 100      | AllowPluresDBP2P | Inbound   | 34567 | TCP      | P2P mesh networking   |
| 110      | AllowPluresDBAPI | Inbound   | 34568 | TCP      | HTTP REST API         |
| 120      | AllowSSH         | Inbound   | 22    | TCP      | SSH access (optional) |

### 4. Container Instances

Lightweight containers running PluresDB:

**Specifications**:
- **Image**: `plures/pluresdb:latest`
- **CPU**: 1 vCPU (configurable)
- **Memory**: 2GB (configurable)
- **Restart Policy**: Always
- **IP Address**: Public (for P2P connectivity)

**Environment Variables**:
- `NODE_ENV`: test/dev/prod
- `NODE_INDEX`: 0, 1, 2, ... (unique per node)
- `TOTAL_NODES`: Total number of nodes in deployment
- `PLURESDB_PORT`: 34567
- `PLURESDB_API_PORT`: 34568

### 5. Storage Account

Provides persistent storage:
- **Type**: 
  - Test/Dev: Standard_LRS (Locally Redundant Storage)
  - Prod: Standard_GRS (Geo-Redundant Storage)
- **Access Tier**: Hot
- **Security**: HTTPS-only, TLS 1.2 minimum
- **Uses**:
  - Application logs
  - Test data backups
  - Configuration files
  - Deployment artifacts

## Network Communication

### P2P Mesh Network

Nodes communicate using a mesh topology:

```
Node 0 ←→ Node 1
  ↕        ↕
Node 2 ←→ Node 3
  ↕        ↕
 ...      ...
```

**Protocol**: WebSocket over TCP
**Port**: 34567
**Features**:
- Auto-discovery via broadcast/multicast
- Heartbeat for liveness detection
- CRDT-based data synchronization
- Automatic reconnection on failure

### API Access

Each node exposes an HTTP REST API:
- **Port**: 34568
- **Protocol**: HTTP/HTTPS
- **Endpoints**:
  - `GET /health`: Health check
  - `GET /peers`: List connected peers
  - `POST /data`: Write data
  - `GET /data/:key`: Read data
  - `GET /metrics`: Prometheus metrics

## Deployment Models

### Minimal Test Deployment (3 nodes)

```
Cost: ~$50-75/month
CPU: 3 vCPUs
Memory: 6GB
Storage: 100GB LRS
Network: ~50GB/month
```

**Use Case**: Quick feature testing, development

### Standard Dev Deployment (5 nodes)

```
Cost: ~$100-125/month
CPU: 5 vCPUs
Memory: 10GB
Storage: 250GB LRS
Network: ~100GB/month
```

**Use Case**: Integration testing, performance validation

### Production Deployment (7-10 nodes)

```
Cost: ~$200-300/month
CPU: 7-10 vCPUs
Memory: 14-20GB
Storage: 500GB GRS
Network: ~200GB/month
```

**Use Case**: Production-grade validation, demonstrations

## Scaling Considerations

### Horizontal Scaling

Add more nodes to the mesh:
- **Maximum**: Limited by subnet size (251 nodes in /24)
- **Performance**: Linear scaling up to ~20 nodes
- **Overhead**: Each node maintains N-1 connections

### Vertical Scaling

Increase resources per node:
- **CPU**: 1-4 vCPUs recommended
- **Memory**: 2-8GB recommended
- **Storage**: Based on data size

### Regional Distribution

Deploy nodes across multiple Azure regions:
- **Benefits**: Test geo-distribution, latency sensitivity
- **Costs**: Higher network egress charges
- **Complexity**: More complex network setup

## High Availability

### Container Instance Restart

Containers automatically restart on failure:
- **Policy**: Always
- **Backoff**: Exponential
- **Max Retries**: Unlimited

### Data Persistence

Options for data persistence:
1. **In-Memory** (default): Fast, no persistence
2. **Azure Files**: SMB file share mount
3. **Azure Blob**: Object storage via SDK
4. **Azure Disk**: Block storage mount

### Disaster Recovery

For production environments:
- **Backup**: Daily snapshots to blob storage
- **Retention**: 30 days
- **RPO**: 24 hours
- **RTO**: 1 hour

## Monitoring & Observability

### Metrics (Future)

- **Application**: Via Prometheus endpoint
  - Request rate
  - Error rate
  - P2P connection count
  - Data sync latency

- **Infrastructure**: Via Azure Monitor
  - CPU utilization
  - Memory utilization
  - Network throughput
  - Disk I/O

### Logging (Future)

- **Application Logs**: To Azure Storage
- **Container Logs**: Via `az container logs`
- **Network Logs**: NSG flow logs

### Alerting (Future)

- Container failures
- High CPU/memory usage
- Network connectivity issues
- Data sync delays

## Security Architecture

### Network Security

- **NSG Rules**: Minimal required ports only
- **Private Endpoints**: Consider for production
- **DDoS Protection**: Azure DDoS basic (included)

### Access Control

- **Azure RBAC**: Role-based access to resources
- **SSH Keys**: Required for VM access (if using VMs)
- **API Keys**: For PluresDB API access (future)

### Data Security

- **Encryption in Transit**: TLS 1.2+
- **Encryption at Rest**: Azure Storage encryption
- **Key Management**: Azure Key Vault (future)

## Cost Optimization

### Strategies

1. **Auto-shutdown**: Destroy test environments when not in use
2. **Reserved Instances**: For predictable workloads
3. **Spot Instances**: For fault-tolerant workloads (future)
4. **Right-sizing**: Monitor and adjust resource allocation
5. **Storage Tiers**: Use cool/archive for old data

### Monitoring Costs

Use Azure Cost Management:
- Set budgets and alerts
- Review spending weekly
- Identify cost anomalies
- Optimize resource usage

## Future Enhancements

1. **Azure Kubernetes Service (AKS)**: For larger deployments
2. **Application Insights**: For advanced telemetry
3. **Azure DevOps Integration**: For CI/CD
4. **Azure Key Vault**: For secrets management
5. **Azure CDN**: For static content delivery
6. **Auto-scaling**: Based on load or schedule
7. **Multi-region**: Global distribution
8. **Azure Front Door**: For load balancing
9. **Private Link**: For secure connectivity

## References

- [Azure Container Instances Documentation](https://docs.microsoft.com/en-us/azure/container-instances/)
- [Azure Virtual Networks Documentation](https://docs.microsoft.com/en-us/azure/virtual-network/)
- [Azure Bicep Documentation](https://docs.microsoft.com/en-us/azure/azure-resource-manager/bicep/)
- [PluresDB Documentation](../../README.md)
