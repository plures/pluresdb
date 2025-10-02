#!/bin/bash

# Rusty Gun Azure Deployment Script
# Bash script to deploy Rusty Gun to Azure

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
ENVIRONMENT=""
RESOURCE_GROUP_NAME=""
LOCATION="East US"
SUBSCRIPTION_ID=""
SKIP_INFRASTRUCTURE=false
SKIP_CONTAINER=false
SKIP_DEPLOYMENT=false
WHAT_IF=false

# Function to print colored output
print_color() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

print_header() {
    print_color $BLUE "\n=== $1 ==="
}

print_success() {
    print_color $GREEN "✓ $1"
}

print_warning() {
    print_color $YELLOW "⚠ $1"
}

print_error() {
    print_color $RED "✗ $1"
}

# Show usage
show_usage() {
    echo "Rusty Gun Azure Deployment Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Required:"
    echo "  -e, --environment ENV    Environment (dev, staging, prod)"
    echo ""
    echo "Optional:"
    echo "  -g, --resource-group RG  Resource group name (default: rusty-gun-ENV-rg)"
    echo "  -l, --location LOC       Azure location (default: East US)"
    echo "  -s, --subscription ID    Azure subscription ID"
    echo "  --skip-infrastructure    Skip infrastructure deployment"
    echo "  --skip-container         Skip container build and push"
    echo "  --skip-deployment        Skip application deployment"
    echo "  --what-if                Run what-if deployment"
    echo "  -h, --help               Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 -e dev"
    echo "  $0 -e prod -g my-rg -l \"West US 2\""
    echo "  $0 -e staging --what-if"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -e|--environment)
            ENVIRONMENT="$2"
            shift 2
            ;;
        -g|--resource-group)
            RESOURCE_GROUP_NAME="$2"
            shift 2
            ;;
        -l|--location)
            LOCATION="$2"
            shift 2
            ;;
        -s|--subscription)
            SUBSCRIPTION_ID="$2"
            shift 2
            ;;
        --skip-infrastructure)
            SKIP_INFRASTRUCTURE=true
            shift
            ;;
        --skip-container)
            SKIP_CONTAINER=true
            shift
            ;;
        --skip-deployment)
            SKIP_DEPLOYMENT=true
            shift
            ;;
        --what-if)
            WHAT_IF=true
            shift
            ;;
        -h|--help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Validate required parameters
if [[ -z "$ENVIRONMENT" ]]; then
    print_error "Environment is required"
    show_usage
    exit 1
fi

if [[ ! "$ENVIRONMENT" =~ ^(dev|staging|prod)$ ]]; then
    print_error "Environment must be one of: dev, staging, prod"
    exit 1
fi

# Set default resource group name if not provided
if [[ -z "$RESOURCE_GROUP_NAME" ]]; then
    RESOURCE_GROUP_NAME="rusty-gun-${ENVIRONMENT}-rg"
fi

# Check prerequisites
check_prerequisites() {
    print_header "Checking Prerequisites"
    
    # Check Azure CLI
    if ! command -v az &> /dev/null; then
        print_error "Azure CLI not found. Please install Azure CLI."
        exit 1
    fi
    
    local az_version=$(az version --query '"azure-cli"' -o tsv)
    print_success "Azure CLI version: $az_version"
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        print_error "Docker not found. Please install Docker."
        exit 1
    fi
    
    local docker_version=$(docker --version)
    print_success "Docker version: $docker_version"
    
    # Check if logged in to Azure
    if ! az account show &> /dev/null; then
        print_error "Not logged in to Azure. Please run 'az login' first."
        exit 1
    fi
    
    local account=$(az account show --query user.name -o tsv)
    print_success "Logged in as: $account"
    
    if [[ -n "$SUBSCRIPTION_ID" ]]; then
        local current_sub=$(az account show --query id -o tsv)
        if [[ "$current_sub" != "$SUBSCRIPTION_ID" ]]; then
            print_warning "Switching to subscription: $SUBSCRIPTION_ID"
            az account set --subscription "$SUBSCRIPTION_ID"
        fi
    fi
}

# Deploy infrastructure
deploy_infrastructure() {
    print_header "Deploying Infrastructure"
    
    local bicep_file="azure/infrastructure/bicep/main.bicep"
    local param_file="azure/infrastructure/bicep/parameters/${ENVIRONMENT}.bicepparam"
    
    if [[ ! -f "$bicep_file" ]]; then
        print_error "Bicep file not found: $bicep_file"
        exit 1
    fi
    
    if [[ ! -f "$param_file" ]]; then
        print_error "Parameter file not found: $param_file"
        exit 1
    fi
    
    print_color $YELLOW "Deploying infrastructure for environment: $ENVIRONMENT"
    
    if [[ "$WHAT_IF" == true ]]; then
        print_color $YELLOW "Running what-if deployment..."
        az deployment group create \
            --resource-group "$RESOURCE_GROUP_NAME" \
            --template-file "$bicep_file" \
            --parameters "$param_file" \
            --what-if
    else
        az deployment group create \
            --resource-group "$RESOURCE_GROUP_NAME" \
            --template-file "$bicep_file" \
            --parameters "$param_file" \
            --mode Incremental
    fi
    
    if [[ $? -eq 0 ]]; then
        print_success "Infrastructure deployed successfully"
    else
        print_error "Infrastructure deployment failed"
        exit 1
    fi
}

# Build and push container
build_push_container() {
    print_header "Building and Pushing Container"
    
    local container_registry="rusty-gun.azurecr.io"
    local image_name="rusty-gun"
    local tag="${ENVIRONMENT}-$(date +%Y%m%d-%H%M%S)"
    
    print_color $YELLOW "Building container image..."
    
    # Build the image
    docker build -f azure/containers/docker/Dockerfile.azure -t "${container_registry}/${image_name}:${tag}" .
    
    if [[ $? -ne 0 ]]; then
        print_error "Container build failed"
        exit 1
    fi
    
    print_success "Container image built successfully"
    
    # Login to Azure Container Registry
    print_color $YELLOW "Logging in to Azure Container Registry..."
    az acr login --name "rusty-gun"
    
    if [[ $? -ne 0 ]]; then
        print_error "Failed to login to Azure Container Registry"
        exit 1
    fi
    
    # Push the image
    print_color $YELLOW "Pushing container image..."
    docker push "${container_registry}/${image_name}:${tag}"
    docker push "${container_registry}/${image_name}:latest"
    
    if [[ $? -eq 0 ]]; then
        print_success "Container image pushed successfully"
        echo "$tag"
    else
        print_error "Container push failed"
        exit 1
    fi
}

# Deploy application
deploy_application() {
    local image_tag=$1
    
    print_header "Deploying Application"
    
    local app_name="rusty-gun-web-${ENVIRONMENT}"
    local container_registry="rusty-gun.azurecr.io"
    local image_name="rusty-gun"
    
    print_color $YELLOW "Deploying to Azure App Service..."
    print_color $YELLOW "App Name: $app_name"
    print_color $YELLOW "Image: ${container_registry}/${image_name}:${image_tag}"
    
    # Update the app service with new container image
    az webapp config container set \
        --name "$app_name" \
        --resource-group "$RESOURCE_GROUP_NAME" \
        --docker-custom-image-name "${container_registry}/${image_name}:${image_tag}"
    
    if [[ $? -eq 0 ]]; then
        print_success "Application deployed successfully"
    else
        print_error "Application deployment failed"
        exit 1
    fi
}

# Run smoke tests
test_deployment() {
    print_header "Running Smoke Tests"
    
    local app_name="rusty-gun-web-${ENVIRONMENT}"
    local app_url="https://${app_name}.azurewebsites.net"
    
    print_color $YELLOW "Testing application at: $app_url"
    
    # Wait for deployment to be ready
    print_color $YELLOW "Waiting for deployment to be ready..."
    sleep 30
    
    # Test health endpoint
    if curl -f -s "$app_url/health" > /dev/null; then
        print_success "Health check passed"
    else
        print_warning "Health check failed"
    fi
    
    # Test API endpoint
    if curl -f -s "$app_url/api/config" > /dev/null; then
        print_success "API endpoint test passed"
    else
        print_warning "API endpoint test failed"
    fi
    
    print_success "Smoke tests completed"
}

# Get deployment information
get_deployment_info() {
    print_header "Deployment Information"
    
    local app_name="rusty-gun-web-${ENVIRONMENT}"
    local app_url="https://${app_name}.azurewebsites.net"
    
    print_color $GREEN "Environment: $ENVIRONMENT"
    print_color $GREEN "Resource Group: $RESOURCE_GROUP_NAME"
    print_color $GREEN "Application URL: $app_url"
    
    if [[ -n "$SUBSCRIPTION_ID" ]]; then
        print_color $GREEN "Azure Portal: https://portal.azure.com/#@/resource/subscriptions/$SUBSCRIPTION_ID/resourceGroups/$RESOURCE_GROUP_NAME"
    fi
    
    # Get app service details
    local app_info=$(az webapp show --name "$app_name" --resource-group "$RESOURCE_GROUP_NAME" --output json 2>/dev/null)
    if [[ $? -eq 0 ]]; then
        local server_farm=$(echo "$app_info" | jq -r '.serverFarmId')
        local state=$(echo "$app_info" | jq -r '.state')
        print_color $GREEN "App Service Plan: $server_farm"
        print_color $GREEN "Status: $state"
    else
        print_warning "Could not retrieve app service details"
    fi
}

# Main deployment flow
main() {
    print_color $BLUE "\n🚀 Rusty Gun Azure Deployment"
    print_color $YELLOW "Environment: $ENVIRONMENT"
    print_color $YELLOW "Resource Group: $RESOURCE_GROUP_NAME"
    print_color $YELLOW "Location: $LOCATION"
    
    if [[ "$WHAT_IF" == true ]]; then
        print_color $YELLOW "\n⚠️  WHAT-IF MODE - No actual changes will be made"
    fi
    
    # Check prerequisites
    check_prerequisites
    
    # Deploy infrastructure
    if [[ "$SKIP_INFRASTRUCTURE" == false ]]; then
        deploy_infrastructure
    else
        print_warning "Skipping infrastructure deployment"
    fi
    
    # Build and push container
    local image_tag="latest"
    if [[ "$SKIP_CONTAINER" == false ]]; then
        image_tag=$(build_push_container)
    else
        print_warning "Skipping container build and push"
    fi
    
    # Deploy application
    if [[ "$SKIP_DEPLOYMENT" == false ]]; then
        deploy_application "$image_tag"
        test_deployment
    else
        print_warning "Skipping application deployment"
    fi
    
    # Show deployment information
    get_deployment_info
    
    print_color $GREEN "\n🎉 Deployment completed successfully!"
}

# Run main function
main
