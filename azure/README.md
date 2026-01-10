# Azure Relay Testing and Automation

This directory contains the Azure infrastructure and testing automation for PluresDB's P2P relay functionality.

> **📢 Important**: If you're upgrading from an older version, please see [docs/MIGRATION_GUIDE.md](docs/MIGRATION_GUIDE.md) for instructions on migrating your Azure secrets to the new format.

## Overview

The Azure infrastructure enables testing of PluresDB's P2P relay/mesh networking capabilities across multiple nodes in a cloud environment. It supports three environments:

- **Test**: For active development and experimental features
- **Dev**: For feature validation before production
- **Prod**: For production-ready deployments and final validation

## Directory Structure

```
azure/
├── infrastructure/       # Bicep templates for Azure resources
│   ├── main.bicep       # Main infrastructure template
│   └── node.bicep       # Individual node deployment template
├── scripts/             # Deployment and management scripts
│   ├── deploy.sh        # Deploy infrastructure
│   └── destroy.sh       # Destroy infrastructure
├── tests/               # Automated test suites
│   └── relay-tests.ts   # P2P relay functionality tests
└── docs/                # Documentation
    ├── TEST_PLAN.md     # Test plan and promotion criteria
    ├── ARCHITECTURE.md  # Infrastructure architecture
    ├── SECRETS.md       # Secrets configuration guide
    └── MIGRATION_GUIDE.md # Migration guide for secrets
```

## Quick Start

### Prerequisites

1. Azure CLI installed: https://docs.microsoft.com/en-us/cli/azure/install-azure-cli
2. Azure subscription with appropriate permissions
3. SSH key pair (or one will be generated)

### Deploy an Environment

```bash
# Deploy test environment with 3 nodes
cd azure/scripts
./deploy.sh --environment test --node-count 3

# Deploy dev environment in specific region
./deploy.sh --environment dev --location westus2 --node-count 5

# Deploy prod environment with custom SSH key
./deploy.sh --environment prod --ssh-key "$(cat ~/.ssh/id_rsa.pub)"
```

### Destroy an Environment

```bash
# Destroy test environment (with confirmation prompt)
./destroy.sh --environment test

# Destroy dev environment (skip confirmation)
./destroy.sh --environment dev --confirm
```

## Infrastructure Components

### Network Architecture

- **Virtual Network**: Isolated network for PluresDB nodes
- **Network Security Group**: Controls inbound/outbound traffic
  - Port 34567: P2P mesh networking
  - Port 34568: HTTP API
  - Port 22: SSH access (for debugging)

### Compute Resources

- **Container Instances**: Lightweight containers running PluresDB
  - Each node gets a public IP for P2P connectivity
  - Auto-restart on failure
  - Configurable CPU/memory resources

### Storage

- **Storage Account**: For logs, data persistence, and backups
  - Test/Dev: Standard LRS (locally redundant)
  - Prod: Standard GRS (geo-redundant)

## Testing

See [docs/TEST_PLAN.md](docs/TEST_PLAN.md) for detailed test procedures and promotion criteria.

### Run Automated Tests

```bash
# Run relay functionality tests against test environment
npm run test:azure:relay

# Run full test suite
npm run test:azure:full
```

## Cost Management

Approximate monthly costs (USD):

- **Test** (3 nodes): ~$50-75/month
- **Dev** (5 nodes): ~$100-125/month
- **Prod** (production-grade): ~$200-300/month

To minimize costs:
- Destroy test environments when not in use
- Use smaller VM sizes for dev/test
- Enable auto-shutdown for dev/test environments

## Monitoring

Monitor your deployed infrastructure:

```bash
# View all resources in environment
az resource list --resource-group pluresdb-test-rg --output table

# Get container logs
az container logs --resource-group pluresdb-test-rg --name pluresdb-test-node-0

# Check container status
az container show --resource-group pluresdb-test-rg --name pluresdb-test-node-0
```

## Troubleshooting

### Common Issues

1. **Deployment fails with quota error**
   - Check your Azure subscription quotas
   - Try a different region with available capacity

2. **Nodes can't connect to each other**
   - Verify NSG rules allow traffic on ports 34567-34568
   - Check container logs for connectivity errors

3. **Container fails to start**
   - Check Docker image is accessible
   - Verify environment variables are correctly set
   - Review container logs for startup errors

## Contributing

When adding new infrastructure or tests:

1. Update Bicep templates in `infrastructure/`
2. Add corresponding scripts in `scripts/`
3. Update test suites in `tests/`
4. Document changes in `docs/`

## Security Considerations

- SSH keys are required for VM access
- All traffic uses TLS where applicable
- Storage accounts enforce HTTPS-only
- Network security groups restrict access
- Secrets should be stored in Azure Key Vault (not implemented yet)

## Next Steps

- [ ] Add Azure Key Vault integration for secrets
- [ ] Implement automated backup/restore
- [ ] Add Application Insights for telemetry
- [ ] Create Terraform alternative to Bicep
- [ ] Add auto-scaling based on load
