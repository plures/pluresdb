#!/bin/bash
# Destroy PluresDB infrastructure in Azure

set -e

# Default values
ENVIRONMENT="test"
RESOURCE_GROUP=""
CONFIRM=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --environment)
      ENVIRONMENT="$2"
      shift 2
      ;;
    --resource-group)
      RESOURCE_GROUP="$2"
      shift 2
      ;;
    --confirm)
      CONFIRM=true
      shift
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Set resource group if not provided
if [ -z "$RESOURCE_GROUP" ]; then
  RESOURCE_GROUP="pluresdb-${ENVIRONMENT}-rg"
fi

echo "=================================================="
echo "Destroying PluresDB Azure Infrastructure"
echo "=================================================="
echo "Environment:     $ENVIRONMENT"
echo "Resource Group:  $RESOURCE_GROUP"
echo "=================================================="

# Check if Azure CLI is installed
if ! command -v az &> /dev/null; then
  echo "Error: Azure CLI is not installed"
  exit 1
fi

# Check if logged in to Azure
if ! az account show &> /dev/null; then
  echo "Error: Not logged in to Azure"
  echo "Please run: az login"
  exit 1
fi

# Check if resource group exists
if ! az group exists --name "$RESOURCE_GROUP" --output tsv | grep -q "true"; then
  echo "Resource group $RESOURCE_GROUP does not exist"
  exit 0
fi

# Confirm deletion
if [ "$CONFIRM" = false ]; then
  echo ""
  echo "WARNING: This will delete ALL resources in $RESOURCE_GROUP"
  echo "This action cannot be undone!"
  echo ""
  read -p "Are you sure you want to continue? (yes/no): " answer
  if [ "$answer" != "yes" ]; then
    echo "Aborted"
    exit 0
  fi
fi

# Delete resource group
echo "Deleting resource group: $RESOURCE_GROUP"
az group delete \
  --name "$RESOURCE_GROUP" \
  --yes \
  --no-wait

echo "=================================================="
echo "Deletion initiated!"
echo "=================================================="
echo "The resource group is being deleted in the background."
echo ""
echo "To check deletion status:"
echo "  az group exists --name $RESOURCE_GROUP"
echo "=================================================="
