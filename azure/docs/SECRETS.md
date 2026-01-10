# Azure Testing Secrets Configuration

This document describes the secrets and credentials needed for Azure relay testing.

## Required Secrets for GitHub Actions

To enable automated Azure testing in GitHub Actions, configure these secrets in your repository settings:

### 1. AZURE_CLIENT_ID

Azure Service Principal client ID for authentication.

**How to create a Service Principal and get the values**:

```bash
# Create a service principal with Contributor role
az ad sp create-for-rbac \
  --name "pluresdb-github-actions" \
  --role contributor \
  --scopes /subscriptions/{subscription-id}

# The output will be a JSON object with the required values
```

**Example output**:
```json
{
  "appId": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "displayName": "pluresdb-github-actions",
  "password": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "tenant": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
}
```

**In GitHub**:
1. Go to repository Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Name: `AZURE_CLIENT_ID`
4. Value: The `appId` value from the output above
5. Click "Add secret"

### 2. AZURE_CLIENT_SECRET

Azure Service Principal client secret (password) for authentication.

**In GitHub**:
1. Settings → Secrets and variables → Actions
2. New repository secret
3. Name: `AZURE_CLIENT_SECRET`
4. Value: The `password` value from the service principal creation output
5. Click "Add secret"

### 3. AZURE_TENANT_ID

Azure Active Directory tenant ID.

**In GitHub**:
1. Settings → Secrets and variables → Actions
2. New repository secret
3. Name: `AZURE_TENANT_ID`
4. Value: The `tenant` value from the service principal creation output
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

### Authentication Failed

**Error**: `AADSTS7000215: Invalid client secret` or `Login failed with Error: Using auth-type: SERVICE_PRINCIPAL. Not all values are present.`

**Solution**: 
- Verify all four required secrets are configured in GitHub:
  - `AZURE_CLIENT_ID`
  - `AZURE_CLIENT_SECRET`
  - `AZURE_TENANT_ID`
  - `AZURE_SUBSCRIPTION_ID`
- Check if credentials have expired
- Reset credentials: `az ad sp credential reset --name "pluresdb-github-actions"`
- Update the `AZURE_CLIENT_SECRET` with the new password value

### Insufficient Permissions

**Error**: `AuthorizationFailed: does not have authorization to perform action`

**Solution**:
- Verify service principal has Contributor role
- Check the scope is correct (subscription or resource group)
- Verify role assignment: `az role assignment list --assignee {clientId}`

### Subscription Not Found

**Error**: `SubscriptionNotFound`

**Solution**:
- Verify `AZURE_SUBSCRIPTION_ID` secret is correct
- Check service principal has access: `az account list --output table`

## Additional Resources

- [Azure Service Principal Documentation](https://docs.microsoft.com/azure/active-directory/develop/app-objects-and-service-principals)
- [GitHub Actions Azure Login](https://github.com/Azure/login)
- [Azure RBAC Documentation](https://docs.microsoft.com/azure/role-based-access-control/)
