# Azure Secrets Migration Guide

## Overview

The Azure Relay Tests workflow has been updated to use the current Azure Login action authentication method. This requires migrating from the deprecated `AZURE_CREDENTIALS` secret format to individual secrets.

## What Changed

**Before (Deprecated)**:
- Used a single `AZURE_CREDENTIALS` secret containing a JSON object
- Created with `az ad sp create-for-rbac --sdk-auth` command
- The `--sdk-auth` flag is now deprecated

**After (Current)**:
- Uses four separate secrets for each credential component
- Created with `az ad sp create-for-rbac` (without `--sdk-auth`)
- Compatible with latest `azure/login@v1` action

## Migration Steps

### Step 1: Create New Service Principal (or Get Existing Values)

If you need to create a new service principal:

```bash
az ad sp create-for-rbac \
  --name "pluresdb-github-actions" \
  --role contributor \
  --scopes /subscriptions/{subscription-id}
```

This will output:
```json
{
  "appId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "displayName": "pluresdb-github-actions",
  "password": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "tenant": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
}
```

### Step 2: Configure GitHub Secrets

Go to your GitHub repository → Settings → Secrets and variables → Actions

Add these four secrets:

1. **AZURE_CLIENT_ID**
   - Value: The `appId` from the service principal output
   
2. **AZURE_CLIENT_SECRET**
   - Value: The `password` from the service principal output
   
3. **AZURE_TENANT_ID**
   - Value: The `tenant` from the service principal output
   
4. **AZURE_SUBSCRIPTION_ID**
   - Value: Your Azure subscription ID
   - Find it with: `az account show --query id --output tsv`

### Step 3: Verify Configuration

After adding the secrets, trigger a manual workflow run:

1. Go to Actions → Azure Relay Tests
2. Click "Run workflow"
3. Select environment: `test`
4. Click "Run workflow"

If the Azure Login step succeeds, the migration is complete.

### Step 4: Remove Old Secret (Optional)

Once you've verified the new secrets work, you can optionally remove the old `AZURE_CREDENTIALS` secret:

1. Go to repository Settings → Secrets and variables → Actions
2. Find `AZURE_CREDENTIALS` in the list
3. Click the trash icon to delete it

## Troubleshooting

### Error: "Not all values are present"

This means one or more of the four required secrets is missing. Verify all four are configured:
- AZURE_CLIENT_ID
- AZURE_CLIENT_SECRET
- AZURE_TENANT_ID
- AZURE_SUBSCRIPTION_ID

### Error: "Invalid client secret"

The `AZURE_CLIENT_SECRET` value is incorrect or has expired. 

To reset:
```bash
az ad sp credential reset --name "pluresdb-github-actions"
```

Update the `AZURE_CLIENT_SECRET` secret with the new password value.

### Error: "Subscription not found"

The `AZURE_SUBSCRIPTION_ID` is incorrect or the service principal doesn't have access to it.

Verify:
```bash
# Check your subscription ID
az account show --query id --output tsv

# Verify service principal access
az login --service-principal \
  -u <AZURE_CLIENT_ID> \
  -p <AZURE_CLIENT_SECRET> \
  --tenant <AZURE_TENANT_ID>
  
az account list --output table
```

## Reference

For complete setup instructions, see [SECRETS.md](SECRETS.md).
