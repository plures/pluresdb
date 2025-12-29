#!/bin/bash
# Deploy PluresDB infrastructure to Azure

set -e

# Default values
ENVIRONMENT="test"
LOCATION="eastus"
NODE_COUNT=3
RESOURCE_GROUP=""
SSH_PUBLIC_KEY=""

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --environment)
      ENVIRONMENT="$2"
      shift 2
      ;;
    --location)
      LOCATION="$2"
      shift 2
      ;;
    --node-count)
      NODE_COUNT="$2"
      shift 2
      ;;
    --resource-group)
      RESOURCE_GROUP="$2"
      shift 2
      ;;
    --ssh-key)
      SSH_PUBLIC_KEY="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Validate environment
if [[ ! "$ENVIRONMENT" =~ ^(test|dev|prod)$ ]]; then
  echo "Error: Environment must be test, dev, or prod"
  exit 1
fi

# Set resource group if not provided
if [ -z "$RESOURCE_GROUP" ]; then
  RESOURCE_GROUP="pluresdb-${ENVIRONMENT}-rg"
fi

# Generate SSH key if not provided
if [ -z "$SSH_PUBLIC_KEY" ]; then
  if [ -f ~/.ssh/id_rsa.pub ]; then
    SSH_PUBLIC_KEY=$(cat ~/.ssh/id_rsa.pub)
  else
    echo "Error: No SSH public key provided and none found at ~/.ssh/id_rsa.pub"
    exit 1
  fi
fi

echo "=================================================="
echo "Deploying PluresDB Azure Infrastructure"
echo "=================================================="
echo "Environment:     $ENVIRONMENT"
echo "Location:        $LOCATION"
echo "Node Count:      $NODE_COUNT"
echo "Resource Group:  $RESOURCE_GROUP"
echo "=================================================="

# Check if Azure CLI is installed
if ! command -v az &> /dev/null; then
  echo "Error: Azure CLI is not installed"
  echo "Please install it from: https://docs.microsoft.com/en-us/cli/azure/install-azure-cli"
  exit 1
fi

# Check if logged in to Azure
if ! az account show &> /dev/null; then
  echo "Error: Not logged in to Azure"
  echo "Please run: az login"
  exit 1
fi

# Create resource group if it doesn't exist
echo "Creating resource group: $RESOURCE_GROUP"
az group create \
  --name "$RESOURCE_GROUP" \
  --location "$LOCATION" \
  --tags environment="$ENVIRONMENT" project="pluresdb"

# Deploy infrastructure
echo "Deploying infrastructure..."
DEPLOYMENT_NAME="pluresdb-${ENVIRONMENT}-$(date +%Y%m%d-%H%M%S)"

az deployment group create \
  --name "$DEPLOYMENT_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --template-file "$(dirname "$0")/../infrastructure/main.bicep" \
  --parameters \
    environment="$ENVIRONMENT" \
    location="$LOCATION" \
    nodeCount="$NODE_COUNT" \
    sshPublicKey="$SSH_PUBLIC_KEY"

# Get deployment outputs
echo "Deployment complete!"
echo "Getting deployment outputs..."
az deployment group show \
  --name "$DEPLOYMENT_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.outputs

echo "=================================================="
echo "Deployment successful!"
echo "=================================================="
echo "Resource Group: $RESOURCE_GROUP"
echo "Deployment:     $DEPLOYMENT_NAME"
echo ""
echo "To view resources:"
echo "  az resource list --resource-group $RESOURCE_GROUP --output table"
echo ""
echo "To destroy this environment:"
echo "  ./destroy.sh --environment $ENVIRONMENT --resource-group $RESOURCE_GROUP"
echo "=================================================="
