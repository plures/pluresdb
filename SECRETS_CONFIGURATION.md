# GitHub Actions Secrets Configuration

This document lists the secrets that need to be configured in your GitHub repository for the release workflow to function properly.

## Required Secrets

### 1. `NPM_TOKEN` (Optional but Recommended)
- **Purpose**: Authenticates with npm registry to publish packages
- **Where to get it**: 
  - Go to https://www.npmjs.com/settings/[your-username]/tokens
  - Create a new "Automation" token
  - Copy the token value
- **What happens if not set**: The workflow will skip npm publishing and show a warning message
- **Repository Settings**: Settings → Secrets and variables → Actions → New repository secret

### 2. `DOCKERHUB_USERNAME` (Optional)
- **Purpose**: Docker Hub username for publishing Docker images
- **Where to get it**: Your Docker Hub account username
- **What happens if not set**: The workflow will skip Docker image publishing and show a warning message
- **Repository Settings**: Settings → Secrets and variables → Actions → New repository secret

### 3. `DOCKERHUB_TOKEN` (Optional)
- **Purpose**: Docker Hub access token for publishing Docker images
- **Where to get it**:
  - Go to https://hub.docker.com/settings/security
  - Click "New Access Token"
  - Give it a name (e.g., "GitHub Actions")
  - Copy the token value (you won't see it again!)
- **What happens if not set**: The workflow will skip Docker image publishing and show a warning message
- **Repository Settings**: Settings → Secrets and variables → Actions → New repository secret

## Automatically Provided Secrets

### `GITHUB_TOKEN` (No configuration needed)
- **Purpose**: Automatically provided by GitHub Actions for repository operations
- **Note**: This secret is automatically available and does not need to be configured manually

## Summary

**Minimum Configuration**: None required (workflow will run but skip publishing steps)

**Recommended Configuration**:
- `NPM_TOKEN` - If you want to publish to npm
- `DOCKERHUB_USERNAME` + `DOCKERHUB_TOKEN` - If you want to publish Docker images

## How to Configure Secrets

1. Go to your GitHub repository
2. Navigate to **Settings** → **Secrets and variables** → **Actions**
3. Click **New repository secret**
4. Enter the secret name (e.g., `NPM_TOKEN`)
5. Enter the secret value
6. Click **Add secret**
7. Repeat for each secret you want to configure

## Verification

After configuring secrets, you can verify they work by:
1. Creating a new tag (e.g., `v1.0.0`)
2. Pushing the tag to trigger the release workflow
3. Checking the workflow logs to see if publishing steps execute

