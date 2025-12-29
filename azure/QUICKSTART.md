# Azure Relay Testing - Quick Start Guide

This guide will help you quickly set up and run PluresDB relay tests in Azure.

## Prerequisites

Before you begin, ensure you have:

1. **Azure Account**: Active Azure subscription ([Free trial available](https://azure.microsoft.com/free/))
2. **Azure CLI**: Installed and configured ([Installation guide](https://docs.microsoft.com/cli/azure/install-azure-cli))
3. **Permissions**: Contributor role on your Azure subscription
4. **SSH Key**: Or willingness to generate one

## Step 1: Login to Azure

```bash
# Login to your Azure account
az login

# Verify you're logged in and see your subscriptions
az account list --output table

# Set your default subscription (if you have multiple)
az account set --subscription "Your Subscription Name"
```

## Step 2: Clone the Repository

```bash
git clone https://github.com/plures/pluresdb.git
cd pluresdb
```

## Step 3: Deploy Test Environment

Deploy a test environment with 3 nodes:

```bash
cd azure/scripts

# Make scripts executable
chmod +x *.sh

# Deploy test environment
./deploy.sh --environment test --node-count 3

# Wait for deployment to complete (typically 3-5 minutes)
```

The script will:
- Create a resource group: `pluresdb-test-rg`
- Deploy a virtual network with security groups
- Create 3 PluresDB container instances
- Set up storage for logs
- Display the IP addresses of all nodes

## Step 4: Verify Deployment

Check that your nodes are running:

```bash
# List all resources in the test environment
az resource list --resource-group pluresdb-test-rg --output table

# Check container status
az container list --resource-group pluresdb-test-rg --output table

# View logs from first node
az container logs --resource-group pluresdb-test-rg --name pluresdb-test-node-0
```

Get the IP addresses of your nodes:

```bash
az container list \
  --resource-group pluresdb-test-rg \
  --query "[].{name:name, ip:ipAddress.ip, status:instanceView.state}" \
  --output table
```

## Step 5: Test Node Connectivity

Test that nodes are accessible:

```bash
# Get the IP of the first node
NODE_IP=$(az container show \
  --resource-group pluresdb-test-rg \
  --name pluresdb-test-node-0 \
  --query ipAddress.ip \
  --output tsv)

# Test health endpoint
curl http://$NODE_IP:34568/health

# Test peers endpoint (may take 30s for nodes to discover each other)
curl http://$NODE_IP:34568/peers
```

Expected response from `/health`:
```json
{"status": "healthy"}
```

## Step 6: Run Automated Tests

Run the automated relay tests:

```bash
# Navigate back to repository root
cd ../..

# Set environment variables for tests
export AZURE_NODE_COUNT=3
export SKIP_AZURE_TESTS=false

# Get actual IPs from your deployment
NODE_IPS=$(az container list \
  --resource-group pluresdb-test-rg \
  --query "[].ipAddress.ip" \
  --output tsv | tr '\n' ',' | sed 's/,$//')
export AZURE_NODE_IPS=$NODE_IPS

# Run the tests
npm run test:azure:relay
```

## Step 7: Cleanup

When you're done testing, destroy the infrastructure to avoid charges:

```bash
cd azure/scripts

# Destroy test environment
./destroy.sh --environment test --confirm

# Verify cleanup
az group exists --name pluresdb-test-rg
# Should return: false
```

## Common Tasks

### View Container Logs

```bash
# View logs from a specific node
az container logs \
  --resource-group pluresdb-test-rg \
  --name pluresdb-test-node-0

# Stream logs in real-time
az container logs \
  --resource-group pluresdb-test-rg \
  --name pluresdb-test-node-0 \
  --follow
```

### Restart a Container

```bash
az container restart \
  --resource-group pluresdb-test-rg \
  --name pluresdb-test-node-0
```

### Check Resource Costs

```bash
# View current spending
az consumption usage list \
  --start-date $(date -d '7 days ago' +%Y-%m-%d) \
  --end-date $(date +%Y-%m-%d) \
  --output table
```

### Scale the Deployment

To change the number of nodes:

```bash
# Destroy existing deployment
./destroy.sh --environment test --confirm

# Deploy with different node count
./deploy.sh --environment test --node-count 5
```

## Troubleshooting

### Deployment Fails

**Problem**: `az deployment group create` fails

**Solutions**:
1. Check your Azure subscription has available quota
2. Try a different region: `./deploy.sh --environment test --location westus2`
3. Verify Azure CLI is up to date: `az upgrade`

### Nodes Can't Connect to Each Other

**Problem**: Peers list is empty

**Solutions**:
1. Wait 30-60 seconds for discovery to complete
2. Check NSG rules: `az network nsg rule list --resource-group pluresdb-test-rg --nsg-name pluresdb-test-nsg --output table`
3. Verify containers are running: `az container list --resource-group pluresdb-test-rg --output table`

### Tests Fail

**Problem**: `npm run test:azure:relay` fails

**Solutions**:
1. Verify `AZURE_NODE_BASE_IP` is set correctly
2. Check nodes are healthy: `curl http://<node-ip>:34568/health`
3. Review test output for specific error messages
4. Check container logs for errors

### Can't Access Health Endpoint

**Problem**: `curl http://<node-ip>:34568/health` times out

**Solutions**:
1. Verify the IP address is correct
2. Check NSG allows port 34568: `az network nsg rule show --resource-group pluresdb-test-rg --nsg-name pluresdb-test-nsg --name AllowPluresDBAPI`
3. Verify container is running: `az container show --resource-group pluresdb-test-rg --name pluresdb-test-node-0 --query instanceView.state`

## Next Steps

- **Explore the Test Plan**: See [TEST_PLAN.md](../docs/TEST_PLAN.md) for detailed testing procedures
- **Understand the Architecture**: Read [ARCHITECTURE.md](../docs/ARCHITECTURE.md)
- **Deploy to Dev**: When tests pass, deploy to dev environment
- **Set up CI/CD**: Configure GitHub Actions for automated testing

## Cost Estimates

Running this test environment:
- **3 nodes**: ~$2-3 per day (~$60-90 per month if left running)
- **Storage**: ~$1 per month

**💡 Pro Tip**: Destroy test environments when not actively using them to minimize costs!

## Getting Help

If you encounter issues:

1. Check the [troubleshooting section](#troubleshooting) above
2. Review container logs: `az container logs --resource-group pluresdb-test-rg --name pluresdb-test-node-0`
3. Check [GitHub Issues](https://github.com/plures/pluresdb/issues)
4. Open a new issue with:
   - Error messages
   - Container logs
   - Deployment outputs
   - Steps to reproduce

## Additional Resources

- [Azure Container Instances Documentation](https://docs.microsoft.com/azure/container-instances/)
- [Azure CLI Reference](https://docs.microsoft.com/cli/azure/)
- [PluresDB Documentation](../../README.md)
- [Test Plan](../docs/TEST_PLAN.md)
- [Architecture Overview](../docs/ARCHITECTURE.md)
