# Azure Testing Secrets Configuration

This document describes the secrets and credentials needed for Azure relay testing.

## Optional Secret for GitHub Actions

> **Note**: The Azure relay tests are **optional**. If you don't configure Azure credentials, the scheduled tests will be automatically skipped with a notification. You only need to configure this if you want to run Azure infrastructure tests.

To enable automated Azure testing in GitHub Actions, you can optionally configure the `AZURE_CREDENTIALS` secret in your repository settings.

### AZURE_CREDENTIALS (Optional)

A JSON object containing Azure Service Principal authentication credentials.

> **Important**: This secret is **optional**. If not configured:
> - Scheduled Azure relay tests will be automatically skipped
> - A notification will be logged in the workflow run
> - On the first scheduled run without credentials, an issue will be created to guide you through setup
> - You can manually trigger the workflow when ready by configuring the secret

**How to create a Service Principal and get the credentials**:

```bash
# Create a service principal with Contributor role
az ad sp create-for-rbac \
  --name "pluresdb-github-actions" \
  --role contributor \
  --scopes /subscriptions/{subscription-id} \
  --sdk-auth

# The output will be a JSON object that you'll use as the AZURE_CREDENTIALS secret
```

**Example output**:
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

**In GitHub**:
1. Go to repository Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Name: `AZURE_CREDENTIALS`
4. Value: Paste the entire JSON object from the command output above
5. Click "Add secret"

> **Note**: The `--sdk-auth` flag is deprecated but still supported for generating the JSON format required by the Azure Login action (currently using v1 in this workflow). For production use, consider migrating to OpenID Connect (OIDC) authentication which doesn't require storing secrets.

## Alternative: Individual Secrets (Legacy)

If you prefer to use individual secrets instead of the JSON format, you can configure these four separate secrets:

### 1. AZURE_CLIENT_ID

Azure Service Principal client ID for authentication.

**In GitHub**:
1. Settings → Secrets and variables → Actions
2. New repository secret
3. Name: `AZURE_CLIENT_ID`
4. Value: The `clientId` value from the service principal creation output
5. Click "Add secret"

### 2. AZURE_CLIENT_SECRET

Azure Service Principal client secret (password) for authentication.

**In GitHub**:
1. Settings → Secrets and variables → Actions
2. New repository secret
3. Name: `AZURE_CLIENT_SECRET`
4. Value: The `clientSecret` value from the service principal creation output
5. Click "Add secret"

### 3. AZURE_TENANT_ID

Azure Active Directory tenant ID.

**In GitHub**:
1. Settings → Secrets and variables → Actions
2. New repository secret
3. Name: `AZURE_TENANT_ID`
4. Value: The `tenantId` value from the service principal creation output
5. Click "Add secret"

### 4. AZURE_SUBSCRIPTION_ID

Your Azure subscription ID.

**How to find**:
```bash
az account show --query id --output tsv
```

**In GitHub**:
1. Settings → Secrets and variables → Actions
2. New repository secret
3. Name: `AZURE_SUBSCRIPTION_ID`
4. Value: Your subscription ID (e.g., `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`)

> **Note**: The workflow currently uses `AZURE_CREDENTIALS` (JSON format). If you want to use individual secrets, you'll need to modify the workflow file to use them instead.

## Optional Secrets

### AZURE_LOCATION

Default Azure region for deployments (can be overridden in workflow).

**Default**: `eastus`

**In GitHub**:
1. Settings → Secrets and variables → Actions → Variables
2. New repository variable
3. Name: `AZURE_LOCATION`
4. Value: Your preferred region (e.g., `westus2`, `northeurope`)

## Local Development Secrets

For local testing, you can use Azure CLI authentication (no secrets needed):

```bash
# Login to Azure
az login

# Set your subscription
az account set --subscription "Your Subscription Name"

# Run deployment scripts - they'll use your CLI credentials
cd azure/scripts
./deploy.sh --environment test
```

## Service Principal Permissions

The service principal needs these permissions:

1. **Contributor** role on the subscription (or resource group)
2. Ability to create/delete:
   - Resource Groups
   - Virtual Networks
   - Network Security Groups
   - Container Instances
   - Storage Accounts

## Security Best Practices

### 1. Limit Service Principal Scope

Instead of subscription-level access, create resource group first:

```bash
# Create resource group
az group create --name pluresdb-github-rg --location eastus

# Create service principal scoped to resource group only
az ad sp create-for-rbac \
  --name "pluresdb-github-limited" \
  --role contributor \
  --scopes /subscriptions/{subscription-id}/resourceGroups/pluresdb-github-rg

# Extract the values from the output and configure them as separate GitHub secrets:
# - AZURE_CLIENT_ID (appId from output)
# - AZURE_CLIENT_SECRET (password from output)
# - AZURE_TENANT_ID (tenant from output)
# - AZURE_SUBSCRIPTION_ID (use: az account show --query id --output tsv)
```

### 2. Rotate Credentials Regularly

```bash
# Reset service principal credentials
az ad sp credential reset \
  --name "pluresdb-github-actions"

# Update GitHub secrets with new credentials:
# - Update AZURE_CLIENT_SECRET with the new password value
```

### 3. Use Separate Service Principals per Environment

```bash
# Test environment
az ad sp create-for-rbac \
  --name "pluresdb-test-sp" \
  --role contributor \
  --scopes /subscriptions/{subscription-id}/resourceGroups/pluresdb-test-rg

# Production environment (with more restrictions)
az ad sp create-for-rbac \
  --name "pluresdb-prod-sp" \
  --role "Virtual Machine Contributor" \
  --scopes /subscriptions/{subscription-id}/resourceGroups/pluresdb-prod-rg
```

### 4. Monitor Service Principal Activity

```bash
# View service principal sign-ins
az monitor activity-log list \
  --caller "pluresdb-github-actions" \
  --start-time "2024-01-01" \
  --output table
```

## Verification

Test your secrets are configured correctly:

```bash
# Test Azure CLI login with service principal
az login --service-principal \
  -u {clientId} \
  -p {clientSecret} \
  --tenant {tenantId}

# Verify access
az account show
az group list --output table
```

## Troubleshooting

### Tests Are Being Skipped (Credentials Not Configured)

**Symptom**: Scheduled Azure relay tests show "Skipped" status, and you see a message like "Azure credentials are not configured"

**This is expected behavior** if you haven't configured the `AZURE_CREDENTIALS` secret. The workflow is designed to gracefully skip Azure tests when credentials are missing.

**To fix** (if you want to run the tests):
1. Follow the instructions above to create a Service Principal
2. Configure the `AZURE_CREDENTIALS` secret in GitHub
3. Wait for the next scheduled run or manually trigger the workflow

**To disable notifications**:
- If you don't plan to use Azure testing, you can disable the scheduled workflow by commenting out the `schedule` section in `.github/workflows/azure-relay-tests.yml`

### Authentication Failed

**Error**: `AADSTS7000215: Invalid client secret`, `Login failed with Error: Using auth-type: SERVICE_PRINCIPAL. Not all values are present.`, or `Unexpected input(s) 'client-secret'`

**Solution**: 
- Verify the `AZURE_CREDENTIALS` secret is configured in GitHub with a valid JSON object
- The JSON must include: `clientId`, `clientSecret`, `subscriptionId`, and `tenantId`
- Check if credentials have expired
- Reset credentials and update the secret:
  ```bash
  az ad sp create-for-rbac \
    --name "pluresdb-github-actions" \
    --role contributor \
    --scopes /subscriptions/{subscription-id} \
    --sdk-auth
  ```
- Copy the entire JSON output and update the `AZURE_CREDENTIALS` secret in GitHub

### Insufficient Permissions

**Error**: `AuthorizationFailed: does not have authorization to perform action`

**Solution**:
- Verify service principal has Contributor role
- Check the scope is correct (subscription or resource group)
- Verify role assignment: `az role assignment list --assignee {clientId}`

### Subscription Not Found

**Error**: `SubscriptionNotFound`

**Solution**:
- Verify the `subscriptionId` field in the `AZURE_CREDENTIALS` JSON is correct
- Check service principal has access: `az account list --output table`
- If the subscription ID is wrong, recreate the service principal or manually update the JSON

## Additional Resources

- [Azure Service Principal Documentation](https://docs.microsoft.com/azure/active-directory/develop/app-objects-and-service-principals)
- [GitHub Actions Azure Login](https://github.com/Azure/login)
- [Azure RBAC Documentation](https://docs.microsoft.com/azure/role-based-access-control/)
