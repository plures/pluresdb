# Azure Secrets Migration Guide

## Overview

The Azure Relay Tests workflow uses the `azure/login@v1` action which requires credentials in a specific JSON format. This guide helps you configure the correct `AZURE_CREDENTIALS` secret.

## What Changed

**Previous Issues**:
- Workflow was trying to use individual `client-id`, `tenant-id`, `subscription-id`, and `client-secret` parameters
- The `client-secret` parameter is not supported by `azure/login@v1`
- This caused authentication failures

**Current Solution**:
- Use a single `AZURE_CREDENTIALS` secret containing a JSON object with all credentials
- This is the standard format for `azure/login@v1` with service principal authentication

## Migration Steps

### Step 1: Create Service Principal with JSON Output

```bash
# Create or recreate your service principal with the --sdk-auth flag
az ad sp create-for-rbac \
  --name "pluresdb-github-actions" \
  --role contributor \
  --scopes /subscriptions/{subscription-id} \
  --sdk-auth
```

This will output a JSON object like:
```json
{
  "clientId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "clientSecret": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "subscriptionId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "tenantId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "activeDirectoryEndpointUrl": "https://login.microsoftonline.com",
  "resourceManagerEndpointUrl": "https://management.azure.com/",
  "activeDirectoryGraphResourceId": "https://graph.windows.net/",
  "sqlManagementEndpointUrl": "https://management.core.windows.net:8443/",
  "galleryEndpointUrl": "https://gallery.azure.com/",
  "managementEndpointUrl": "https://management.core.windows.net/"
}
```

> **Note**: If you already have a service principal, you can reset its credentials to get new output:
> ```bash
> az ad sp credential reset --name "pluresdb-github-actions" --sdk-auth
> ```

### Step 2: Configure GitHub Secret

Go to your GitHub repository → Settings → Secrets and variables → Actions

Add or update this secret:

1. **AZURE_CREDENTIALS**
   - Click "New repository secret" (or edit if it already exists)
   - Name: `AZURE_CREDENTIALS`
   - Value: Paste the **entire JSON object** from the service principal output
   - Click "Add secret" or "Update secret"

### Step 3: Remove Old Secrets (Optional)

If you previously configured individual secrets, you can remove them as they're no longer used:

1. Go to repository Settings → Secrets and variables → Actions
2. Remove these secrets if they exist:
   - `AZURE_CLIENT_ID`
   - `AZURE_CLIENT_SECRET`
   - `AZURE_TENANT_ID`
   - `AZURE_SUBSCRIPTION_ID`

### Step 4: Verify Configuration

After adding the secret, trigger a manual workflow run:

1. Go to Actions → Azure Relay Tests
2. Click "Run workflow"
3. Select environment: `test`
4. Click "Run workflow"

If the Azure Login step succeeds, the migration is complete.

## Troubleshooting

### Error: "Not all values are present"

This means the `AZURE_CREDENTIALS` secret is missing or malformed. Verify:
- The secret exists in GitHub repository settings
- The secret contains a valid JSON object (not just the client ID or password)
- The JSON includes all required fields: `clientId`, `clientSecret`, `subscriptionId`, `tenantId`

### Error: "Unexpected input(s) 'client-secret'"

This error appears when the workflow is trying to use individual parameters instead of the JSON format. Make sure:
- You're using the latest version of the workflow from this PR
- The workflow uses `creds: ${{ secrets.AZURE_CREDENTIALS }}` for Azure Login steps

### Error: "Invalid client secret"

The `clientSecret` value in the JSON is incorrect or has expired. 

To reset:
```bash
az ad sp credential reset --name "pluresdb-github-actions" --sdk-auth
```

Update the `AZURE_CREDENTIALS` secret with the new JSON output.

### Error: "Subscription not found"

The `subscriptionId` in the JSON is incorrect or the service principal doesn't have access to it.

Verify:
```bash
# Check your subscription ID
az account show --query id --output tsv

# Verify service principal access (login with the credentials)
az login --service-principal \
  -u {clientId from JSON} \
  -p {clientSecret from JSON} \
  --tenant {tenantId from JSON}
  
az account list --output table
```

## Reference

For complete setup instructions, see [SECRETS.md](SECRETS.md).
