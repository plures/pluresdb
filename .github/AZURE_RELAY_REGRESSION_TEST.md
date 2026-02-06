# Azure Relay Tests - Regression Test Documentation

## The Bug

**Issue**: Azure relay tests failed with authentication error even when OIDC credentials were configured.

**Root Cause**: GitHub Actions secrets are **not accessible** in workflow `if:` conditional expressions. They always evaluate to empty strings in that context.

## Reproduction of the Bug

### Original Code (Broken)
```yaml
# In .github/workflows/azure-relay-tests.yml (commit f26dc90)

- name: Azure Login (OIDC)
  if: ${{ secrets.AZURE_CLIENT_ID != '' }}  # ❌ Always FALSE
  uses: azure/login@v2
  with:
    client-id: ${{ secrets.AZURE_CLIENT_ID }}
    tenant-id: ${{ secrets.AZURE_TENANT_ID }}
    subscription-id: ${{ secrets.AZURE_SUBSCRIPTION_ID }}

- name: Azure Login (Legacy)
  if: ${{ secrets.AZURE_CLIENT_ID == '' && secrets.AZURE_CREDENTIALS != '' }}  # ❌ Only checks AZURE_CREDENTIALS
  uses: azure/login@v2
  with:
    creds: ${{ secrets.AZURE_CREDENTIALS }}
```

### What Happened
1. Even when `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, and `AZURE_SUBSCRIPTION_ID` secrets were configured
2. The condition `if: ${{ secrets.AZURE_CLIENT_ID != '' }}` **always evaluated to false**
3. OIDC login step was always skipped
4. Legacy login step always ran (checking only AZURE_CREDENTIALS)
5. Login failed with: "Not all values are present. Ensure 'client-id' and 'tenant-id' are supplied"

### Why It Failed
GitHub Actions **does not expose secret values** in the `if:` conditional context. Per GitHub's security model:
- Secrets can be used in `with:` parameters (e.g., `client-id: ${{ secrets.AZURE_CLIENT_ID }}`)
- Secrets can be used in `run:` shell scripts (e.g., `echo "${{ secrets.AZURE_CLIENT_ID }}"`)
- Secrets **cannot** be accessed in `if:` expressions - they're always empty

## The Fix

### Fixed Code (Working)
```yaml
# Step 1: Add auth_type output to check-credentials job
check-credentials:
  outputs:
    has_credentials: ${{ steps.check.outputs.has_credentials }}
    auth_type: ${{ steps.check.outputs.auth_type }}  # NEW
  steps:
    - name: Check if Azure credentials are configured
      id: check
      run: |
        # Secrets ARE accessible in shell scripts
        if [ -n "${{ secrets.AZURE_CLIENT_ID }}" ] && [ -n "${{ secrets.AZURE_TENANT_ID }}" ] && [ -n "${{ secrets.AZURE_SUBSCRIPTION_ID }}" ]; then
          echo "has_credentials=true" >> $GITHUB_OUTPUT
          echo "auth_type=oidc" >> $GITHUB_OUTPUT  # NEW
        elif [ -n "${{ secrets.AZURE_CREDENTIALS }}" ]; then
          echo "has_credentials=true" >> $GITHUB_OUTPUT
          echo "auth_type=legacy" >> $GITHUB_OUTPUT  # NEW
        else
          echo "has_credentials=false" >> $GITHUB_OUTPUT
          echo "auth_type=none" >> $GITHUB_OUTPUT  # NEW
        fi

# Step 2: Use auth_type output in conditionals (not secrets)
- name: Azure Login (OIDC)
  if: ${{ needs.check-credentials.outputs.auth_type == 'oidc' }}  # ✅ Works!
  uses: azure/login@v2
  with:
    client-id: ${{ secrets.AZURE_CLIENT_ID }}
    tenant-id: ${{ secrets.AZURE_TENANT_ID }}
    subscription-id: ${{ secrets.AZURE_SUBSCRIPTION_ID }}

- name: Azure Login (Legacy)
  if: ${{ needs.check-credentials.outputs.auth_type == 'legacy' }}  # ✅ Works!
  uses: azure/login@v2
  with:
    creds: ${{ secrets.AZURE_CREDENTIALS }}
```

## Regression Test

The automated regression test is in `.github/workflows/test-workflow-logic.yml`.

### Test Cases

1. **Verify auth_type output exists**
   - Ensures the fix is in place
   - Checks that `auth_type:` is declared in job outputs

2. **Verify all auth_type branches are set**
   - Confirms `auth_type=oidc` for OIDC credentials
   - Confirms `auth_type=legacy` for legacy credentials
   - Confirms `auth_type=none` for no credentials

3. **Verify no direct secret checks (the bug)**
   - Fails if it finds `if: ${{ secrets.AZURE_CLIENT_ID` in Azure Login steps
   - This is the regression test - it would fail on the original broken code

4. **Verify correct auth_type usage**
   - Confirms Azure Login (OIDC) checks `auth_type == 'oidc'`
   - Confirms Azure Login (Legacy) checks `auth_type == 'legacy'`

### Running the Test Locally

```bash
# Run the validation checks
cd .github/workflows
bash -c '
  # Test that would FAIL on original broken code
  if grep -A 5 "name: Azure Login" azure-relay-tests.yml | grep -q "if:.*secrets\.AZURE_CLIENT_ID"; then
    echo "❌ REGRESSION: Found the bug - direct secret check in conditional"
    exit 1
  fi
  
  # Test that would PASS on fixed code
  if grep -A 5 "name: Azure Login (OIDC)" azure-relay-tests.yml | grep -q "auth_type == .oidc."; then
    echo "✅ PASS: Using auth_type output (the fix)"
  fi
'
```

### Manual Verification

You can also verify the fix works by checking the workflow run logs:

**Before fix (run 21325698059)**:
```
Azure Login (OIDC): Skipped
Azure Login (Legacy): Running
Error: Not all values are present. Ensure 'client-id' and 'tenant-id' are supplied.
```

**After fix**:
```
check-credentials:
  ✓ Azure OIDC credentials are configured
  has_credentials=true
  auth_type=oidc

deploy-infrastructure:
  Azure Login (OIDC): Running (auth_type == 'oidc') ✓
  Azure Login (Legacy): Skipped (auth_type != 'legacy') ✓
  Login successful ✓
```

## Impact

This fix is critical for:
- ✅ OIDC authentication to work when configured
- ✅ Scheduled Azure relay tests to run successfully
- ✅ P2P relay functionality validation in Azure
- ✅ Proper credential detection and selection

## References

- **Original failure**: https://github.com/plures/pluresdb/actions/runs/21325698059
- **GitHub Actions secrets documentation**: https://docs.github.com/en/actions/security-guides/encrypted-secrets#using-encrypted-secrets-in-a-workflow
- **Fix commit**: dfa61db
