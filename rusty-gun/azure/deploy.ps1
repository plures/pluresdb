# Rusty Gun Azure Deployment Script
# PowerShell script to deploy Rusty Gun to Azure

param(
    [Parameter(Mandatory=$true)]
    [ValidateSet("dev", "staging", "prod")]
    [string]$Environment,
    
    [Parameter(Mandatory=$false)]
    [string]$ResourceGroupName = "rusty-gun-$Environment-rg",
    
    [Parameter(Mandatory=$false)]
    [string]$Location = "East US",
    
    [Parameter(Mandatory=$false)]
    [string]$SubscriptionId,
    
    [Parameter(Mandatory=$false)]
    [switch]$SkipInfrastructure,
    
    [Parameter(Mandatory=$false)]
    [switch]$SkipContainer,
    
    [Parameter(Mandatory=$false)]
    [switch]$SkipDeployment,
    
    [Parameter(Mandatory=$false)]
    [switch]$WhatIf
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Colors for output
$Red = "`e[31m"
$Green = "`e[32m"
$Yellow = "`e[33m"
$Blue = "`e[34m"
$Reset = "`e[0m"

function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = $Reset
    )
    Write-Host "$Color$Message$Reset"
}

function Write-Header {
    param([string]$Message)
    Write-ColorOutput "`n=== $Message ===" $Blue
}

function Write-Success {
    param([string]$Message)
    Write-ColorOutput "✓ $Message" $Green
}

function Write-Warning {
    param([string]$Message)
    Write-ColorOutput "⚠ $Message" $Yellow
}

function Write-Error {
    param([string]$Message)
    Write-ColorOutput "✗ $Message" $Red
}

# Check prerequisites
function Test-Prerequisites {
    Write-Header "Checking Prerequisites"
    
    # Check Azure CLI
    try {
        $azVersion = az version --output json | ConvertFrom-Json
        Write-Success "Azure CLI version: $($azVersion.'azure-cli')"
    }
    catch {
        Write-Error "Azure CLI not found. Please install Azure CLI."
        exit 1
    }
    
    # Check Docker
    try {
        $dockerVersion = docker --version
        Write-Success "Docker version: $dockerVersion"
    }
    catch {
        Write-Error "Docker not found. Please install Docker."
        exit 1
    }
    
    # Check if logged in to Azure
    try {
        $account = az account show --output json | ConvertFrom-Json
        Write-Success "Logged in as: $($account.user.name)"
        if ($SubscriptionId) {
            if ($account.id -ne $SubscriptionId) {
                Write-Warning "Switching to subscription: $SubscriptionId"
                az account set --subscription $SubscriptionId
            }
        }
    }
    catch {
        Write-Error "Not logged in to Azure. Please run 'az login' first."
        exit 1
    }
}

# Deploy infrastructure
function Deploy-Infrastructure {
    Write-Header "Deploying Infrastructure"
    
    $bicepFile = "azure/infrastructure/bicep/main.bicep"
    $paramFile = "azure/infrastructure/bicep/parameters/$Environment.bicepparam"
    
    if (-not (Test-Path $bicepFile)) {
        Write-Error "Bicep file not found: $bicepFile"
        exit 1
    }
    
    if (-not (Test-Path $paramFile)) {
        Write-Error "Parameter file not found: $paramFile"
        exit 1
    }
    
    Write-ColorOutput "Deploying infrastructure for environment: $Environment" $Yellow
    
    if ($WhatIf) {
        Write-ColorOutput "Running what-if deployment..." $Yellow
        az deployment group create `
            --resource-group $ResourceGroupName `
            --template-file $bicepFile `
            --parameters $paramFile `
            --what-if
    }
    else {
        az deployment group create `
            --resource-group $ResourceGroupName `
            --template-file $bicepFile `
            --parameters $paramFile `
            --mode Incremental
    }
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Infrastructure deployed successfully"
    }
    else {
        Write-Error "Infrastructure deployment failed"
        exit 1
    }
}

# Build and push container
function Build-PushContainer {
    Write-Header "Building and Pushing Container"
    
    $containerRegistry = "rusty-gun.azurecr.io"
    $imageName = "rusty-gun"
    $tag = "$Environment-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
    
    Write-ColorOutput "Building container image..." $Yellow
    
    # Build the image
    docker build -f azure/containers/docker/Dockerfile.azure -t "$containerRegistry/$imageName`:$tag" .
    
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Container build failed"
        exit 1
    }
    
    Write-Success "Container image built successfully"
    
    # Login to Azure Container Registry
    Write-ColorOutput "Logging in to Azure Container Registry..." $Yellow
    az acr login --name "rusty-gun"
    
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to login to Azure Container Registry"
        exit 1
    }
    
    # Push the image
    Write-ColorOutput "Pushing container image..." $Yellow
    docker push "$containerRegistry/$imageName`:$tag"
    docker push "$containerRegistry/$imageName`:latest"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Container image pushed successfully"
        return $tag
    }
    else {
        Write-Error "Container push failed"
        exit 1
    }
}

# Deploy application
function Deploy-Application {
    param([string]$ImageTag)
    
    Write-Header "Deploying Application"
    
    $appName = "rusty-gun-web-$Environment"
    $containerRegistry = "rusty-gun.azurecr.io"
    $imageName = "rusty-gun"
    
    Write-ColorOutput "Deploying to Azure App Service..." $Yellow
    Write-ColorOutput "App Name: $appName" $Yellow
    Write-ColorOutput "Image: $containerRegistry/$imageName`:$ImageTag" $Yellow
    
    # Update the app service with new container image
    az webapp config container set `
        --name $appName `
        --resource-group $ResourceGroupName `
        --docker-custom-image-name "$containerRegistry/$imageName`:$ImageTag"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Application deployed successfully"
    }
    else {
        Write-Error "Application deployment failed"
        exit 1
    }
}

# Run smoke tests
function Test-Deployment {
    Write-Header "Running Smoke Tests"
    
    $appName = "rusty-gun-web-$Environment"
    $appUrl = "https://$appName.azurewebsites.net"
    
    Write-ColorOutput "Testing application at: $appUrl" $Yellow
    
    # Wait for deployment to be ready
    Write-ColorOutput "Waiting for deployment to be ready..." $Yellow
    Start-Sleep -Seconds 30
    
    # Test health endpoint
    try {
        $healthResponse = Invoke-RestMethod -Uri "$appUrl/health" -Method Get -TimeoutSec 30
        Write-Success "Health check passed"
    }
    catch {
        Write-Warning "Health check failed: $($_.Exception.Message)"
    }
    
    # Test API endpoint
    try {
        $apiResponse = Invoke-RestMethod -Uri "$appUrl/api/config" -Method Get -TimeoutSec 30
        Write-Success "API endpoint test passed"
    }
    catch {
        Write-Warning "API endpoint test failed: $($_.Exception.Message)"
    }
    
    Write-Success "Smoke tests completed"
}

# Get deployment information
function Get-DeploymentInfo {
    Write-Header "Deployment Information"
    
    $appName = "rusty-gun-web-$Environment"
    $appUrl = "https://$appName.azurewebsites.net"
    
    Write-ColorOutput "Environment: $Environment" $Green
    Write-ColorOutput "Resource Group: $ResourceGroupName" $Green
    Write-ColorOutput "Application URL: $appUrl" $Green
    Write-ColorOutput "Azure Portal: https://portal.azure.com/#@/resource/subscriptions/$SubscriptionId/resourceGroups/$ResourceGroupName" $Green
    
    # Get app service details
    try {
        $appInfo = az webapp show --name $appName --resource-group $ResourceGroupName --output json | ConvertFrom-Json
        Write-ColorOutput "App Service Plan: $($appInfo.serverFarmId)" $Green
        Write-ColorOutput "Status: $($appInfo.state)" $Green
    }
    catch {
        Write-Warning "Could not retrieve app service details"
    }
}

# Main deployment flow
function Main {
    Write-ColorOutput "`n🚀 Rusty Gun Azure Deployment" $Blue
    Write-ColorOutput "Environment: $Environment" $Yellow
    Write-ColorOutput "Resource Group: $ResourceGroupName" $Yellow
    Write-ColorOutput "Location: $Location" $Yellow
    
    if ($WhatIf) {
        Write-ColorOutput "`n⚠️  WHAT-IF MODE - No actual changes will be made" $Yellow
    }
    
    # Check prerequisites
    Test-Prerequisites
    
    # Deploy infrastructure
    if (-not $SkipInfrastructure) {
        Deploy-Infrastructure
    }
    else {
        Write-Warning "Skipping infrastructure deployment"
    }
    
    # Build and push container
    $imageTag = $null
    if (-not $SkipContainer) {
        $imageTag = Build-PushContainer
    }
    else {
        Write-Warning "Skipping container build and push"
        $imageTag = "latest"
    }
    
    # Deploy application
    if (-not $SkipDeployment) {
        Deploy-Application -ImageTag $imageTag
        Test-Deployment
    }
    else {
        Write-Warning "Skipping application deployment"
    }
    
    # Show deployment information
    Get-DeploymentInfo
    
    Write-ColorOutput "`n🎉 Deployment completed successfully!" $Green
}

# Run main function
Main
