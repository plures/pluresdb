# Azure Testing Secrets Configuration

This document describes the secrets and credentials needed for Azure relay testing.

## Optional Credentials for GitHub Actions

> **Note**: The Azure relay tests are **optional**. If you don't configure Azure credentials, the scheduled tests will be automatically skipped with a notification. You only need to configure this if you want to run Azure infrastructure tests.

To enable automated Azure testing in GitHub Actions, you can optionally configure Azure credentials using one of two methods:

## Method 1: OIDC Authentication (Recommended)

**OpenID Connect (OIDC)** provides secretless authentication to Azure and is the recommended approach for security and maintainability.

### Benefits of OIDC
- ✅ No secrets stored in GitHub (more secure)
- ✅ Short-lived tokens (automatic rotation)
- ✅ No credential expiration management
- ✅ Azure-recommended best practice
- ✅ Better audit trail

### Setup Steps

#### 1. Create Azure App Registration

```bash
# Get your subscription and tenant IDs
SUBSCRIPTION_ID=$(az account show --query id --output tsv)
TENANT_ID=$(az account show --query tenantId --output tsv)

# Create the app registration
APP_ID=$(az ad app create --display-name "pluresdb-github-actions" \
  --query appId --output tsv)

echo "Application (Client) ID: $APP_ID"
echo "Tenant ID: $TENANT_ID"
echo "Subscription ID: $SUBSCRIPTION_ID"
```

#### 2. Create Service Principal and Assign Role

```bash
# Create service principal
az ad sp create --id $APP_ID

# Assign Contributor role to the subscription (or specific resource group)
az role assignment create \
  --assignee $APP_ID \
  --role Contributor \
  --scope /subscriptions/$SUBSCRIPTION_ID
```

#### 3. Add Federated Credential for GitHub

```bash
# For the main branch (adjust repo name if needed)
az ad app federated-credential create \
  --id $APP_ID \
  --parameters '{
    "name": "github-main",
    "issuer": "https://token.actions.githubusercontent.com",
    "subject": "repo:plures/pluresdb:ref:refs/heads/main",
    "audiences": ["api://AzureADTokenExchange"]
  }'

# Optional: Add for pull requests
az ad app federated-credential create \
  --id $APP_ID \
  --parameters '{
    "name": "github-pr",
    "issuer": "https://token.actions.githubusercontent.com",
    "subject": "repo:plures/pluresdb:pull_request",
    "audiences": ["api://AzureADTokenExchange"]
  }'
```

#### 4. Configure GitHub Secrets

Add these three secrets in your GitHub repository:

1. **AZURE_CLIENT_ID**
   - Go to Settings → Secrets and variables → Actions
   - Click "New repository secret"
   - Name: `AZURE_CLIENT_ID`
   - Value: The Application (Client) ID from step 1

2. **AZURE_TENANT_ID**
   - Click "New repository secret"
   - Name: `AZURE_TENANT_ID`
   - Value: The Tenant ID from step 1

3. **AZURE_SUBSCRIPTION_ID**
   - Click "New repository secret"
   - Name: `AZURE_SUBSCRIPTION_ID`
   - Value: The Subscription ID from step 1

### Verification

Test the OIDC setup:

```bash
# List federated credentials
az ad app federated-credential list --id $APP_ID

# Verify role assignment
az role assignment list --assignee $APP_ID --output table
```

The workflow will automatically use OIDC when these three secrets are configured.

---

## Method 2: Service Principal with Secret (Legacy)

> **⚠️ DEPRECATED**: This method uses the deprecated `--sdk-auth` flag and stores long-lived secrets. **Use OIDC (Method 1) instead for better security.**

<details>
<summary>Click to see legacy setup instructions (not recommended)</summary>

### AZURE_CREDENTIALS (Legacy)

A JSON object containing Azure Service Principal authentication credentials.

**How to create a Service Principal and get the credentials**:

```bash
# Create a service principal with Contributor role
# WARNING: --sdk-auth is deprecated
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

</details>

---

## Alternative: Individual Secrets (Legacy - for backward compatibility)

> **Note**: If you're setting up new credentials, use **OIDC (Method 1)** instead. This section is only for backward compatibility with existing setups.

<details>
<summary>Click to see individual secrets setup (legacy)</summary>

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

</details>

---

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

### 1. Use OIDC Instead of Secrets

**Always prefer OIDC (Method 1) over storing credentials** for:
- No secret storage (eliminates secret leakage risk)
- Automatic token rotation
- Better audit trail
- Azure-recommended approach

### 2. Limit Service Principal Scope

Instead of subscription-level access, scope to specific resource groups:

**For OIDC:**
```bash
# Create resource group
az group create --name pluresdb-github-rg --location eastus

# Assign Contributor role scoped to resource group only
az role assignment create \
  --assignee $APP_ID \
  --role Contributor \
  --scope /subscriptions/{subscription-id}/resourceGroups/pluresdb-github-rg
```

**For legacy service principal with secret:**
```bash
# Create resource group
az group create --name pluresdb-github-rg --location eastus

# Create service principal scoped to resource group only
az ad sp create-for-rbac \
  --name "pluresdb-github-limited" \
  --role contributor \
  --scopes /subscriptions/{subscription-id}/resourceGroups/pluresdb-github-rg
```

### 3. Use Separate Credentials per Environment

**For OIDC:**
Create separate app registrations for different environments with different federated credentials:

```bash
# Test environment
az ad app create --display-name "pluresdb-test"
# Add federated credential for test environment/branch

# Production environment
az ad app create --display-name "pluresdb-prod"
# Add federated credential for prod environment/branch
```

### 4. Monitor Service Principal Activity

```bash
# View app registration sign-ins (for OIDC)
az monitor activity-log list \
  --caller $APP_ID \
  --start-time "2024-01-01" \
  --output table
```

## Verification

**For OIDC setup:**
```bash
# Verify federated credentials are configured
az ad app federated-credential list --id $APP_ID

# Verify role assignments
az role assignment list --assignee $APP_ID --output table
```

**For legacy setup with secrets:**
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

**This is expected behavior** if you haven't configured Azure credentials. The workflow is designed to gracefully skip Azure tests when credentials are missing.

**To fix** (if you want to run the tests):
1. Follow **Method 1: OIDC Authentication** (recommended) above
2. Configure the three OIDC secrets in GitHub (`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID`)
3. Wait for the next scheduled run or manually trigger the workflow

**To disable notifications**:
- If you don't plan to use Azure testing, you can disable the scheduled workflow by commenting out the `schedule` section in `.github/workflows/azure-relay-tests.yml`

### OIDC Authentication Failed

**Error**: `AADSTS700016: Application with identifier was not found` or `federated credential not found`

**Solution**:
1. Verify federated credential is configured:
   ```bash
   az ad app federated-credential list --id $APP_ID
   ```
2. Check the subject matches your repository and branch:
   - Should be: `repo:plures/pluresdb:ref:refs/heads/main`
   - Or for PRs: `repo:plures/pluresdb:pull_request`
3. Verify the three GitHub secrets are correctly set:
   - `AZURE_CLIENT_ID` (Application/Client ID)
   - `AZURE_TENANT_ID` (Directory/Tenant ID)
   - `AZURE_SUBSCRIPTION_ID` (Subscription ID)
4. Ensure the workflow has `id-token: write` permission (already configured)

### Legacy Authentication Failed

**Error**: `AADSTS7000215: Invalid client secret`, `Login failed with Error: Using auth-type: SERVICE_PRINCIPAL. Not all values are present.`

**Solution**: 
- **Recommended**: Migrate to OIDC (Method 1) instead of fixing legacy auth
- If you must use legacy auth:
  - Verify the `AZURE_CREDENTIALS` secret is configured in GitHub with a valid JSON object
  - The JSON must include: `clientId`, `clientSecret`, `subscriptionId`, and `tenantId`
  - Check if credentials have expired (secrets expire after 1-2 years)
  - Consider switching to OIDC to avoid credential expiration issues

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
