#!/bin/bash

# PluresDB Docker Runner Script
# Makes it easy to run PluresDB with Docker

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
IMAGE="pluresdb/pluresdb:latest"
API_PORT="34567"
WEB_PORT="34568"
DATA_VOLUME="pluresdb-data"
CONFIG_VOLUME="pluresdb-config"
CONTAINER_NAME="pluresdb"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    echo "PluresDB Docker Runner"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  start     Start PluresDB (default)"
    echo "  stop      Stop PluresDB"
    echo "  restart   Restart PluresDB"
    echo "  logs      Show logs"
    echo "  status    Show status"
    echo "  clean     Remove containers and volumes"
    echo "  help      Show this help"
    echo ""
    echo "Options:"
    echo "  --api-port PORT     API port (default: 34567)"
    echo "  --web-port PORT     Web UI port (default: 34568)"
    echo "  --image IMAGE       Docker image (default: pluresdb/pluresdb:latest)"
    echo "  --no-pull           Don't pull latest image"
    echo "  --detach            Run in background (default)"
    echo "  --foreground        Run in foreground"
    echo ""
    echo "Examples:"
    echo "  $0 start"
    echo "  $0 start --api-port 8080 --web-port 8081"
    echo "  $0 logs"
    echo "  $0 stop"
}

# Function to check if Docker is running
check_docker() {
    if ! docker info >/dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
}

# Function to pull latest image
pull_image() {
    if [ "$NO_PULL" != "true" ]; then
        print_status "Pulling latest image..."
        docker pull "$IMAGE"
    fi
}

# Function to start PluresDB
start_pluresdb() {
    print_status "Starting PluresDB..."
    
    # Check if container already exists
    if docker ps -a --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        print_warning "Container '$CONTAINER_NAME' already exists. Stopping and removing it first..."
        docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
        docker rm "$CONTAINER_NAME" >/dev/null 2>&1 || true
    fi
    
    # Create volumes if they don't exist
    docker volume create "$DATA_VOLUME" >/dev/null 2>&1 || true
    docker volume create "$CONFIG_VOLUME" >/dev/null 2>&1 || true
    
    # Start container
    if [ "$FOREGROUND" = "true" ]; then
        docker run \
            --name "$CONTAINER_NAME" \
            -p "$API_PORT:34567" \
            -p "$WEB_PORT:34568" \
            -v "$DATA_VOLUME:/app/data" \
            -v "$CONFIG_VOLUME:/app/config" \
            -e PLURESDB_PORT=34567 \
            -e PLURESDB_WEB_PORT=34568 \
            -e PLURESDB_HOST=0.0.0.0 \
            -e PLURESDB_DATA_DIR=/app/data \
            -e PLURESDB_CONFIG_DIR=/app/config \
            "$IMAGE"
    else
        docker run -d \
            --name "$CONTAINER_NAME" \
            -p "$API_PORT:34567" \
            -p "$WEB_PORT:34568" \
            -v "$DATA_VOLUME:/app/data" \
            -v "$CONFIG_VOLUME:/app/config" \
            -e PLURESDB_PORT=34567 \
            -e PLURESDB_WEB_PORT=34568 \
            -e PLURESDB_HOST=0.0.0.0 \
            -e PLURESDB_DATA_DIR=/app/data \
            -e PLURESDB_CONFIG_DIR=/app/config \
            --restart unless-stopped \
            "$IMAGE"
        
        print_success "PluresDB started successfully!"
        print_status "API: http://localhost:$API_PORT"
        print_status "Web UI: http://localhost:$WEB_PORT"
        print_status "Container name: $CONTAINER_NAME"
        print_status "To view logs: $0 logs"
        print_status "To stop: $0 stop"
    fi
}

# Function to stop PluresDB
stop_pluresdb() {
    print_status "Stopping PluresDB..."
    if docker ps --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        docker stop "$CONTAINER_NAME"
        print_success "PluresDB stopped successfully!"
    else
        print_warning "PluresDB is not running."
    fi
}

# Function to restart PluresDB
restart_pluresdb() {
    print_status "Restarting PluresDB..."
    stop_pluresdb
    sleep 2
    start_pluresdb
}

# Function to show logs
show_logs() {
    if docker ps -a --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        print_status "Showing logs for $CONTAINER_NAME..."
        docker logs -f "$CONTAINER_NAME"
    else
        print_error "Container '$CONTAINER_NAME' not found."
        exit 1
    fi
}

# Function to show status
show_status() {
    print_status "PluresDB Status:"
    echo ""
    
    if docker ps --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        print_success "Container is running"
        echo ""
        docker ps --filter "name=$CONTAINER_NAME" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
        echo ""
        print_status "API: http://localhost:$API_PORT"
        print_status "Web UI: http://localhost:$WEB_PORT"
    else
        print_warning "Container is not running"
        if docker ps -a --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
            echo ""
            docker ps -a --filter "name=$CONTAINER_NAME" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
        fi
    fi
}

# Function to clean up
clean_up() {
    print_status "Cleaning up PluresDB containers and volumes..."
    
    # Stop and remove container
    if docker ps -a --format "table {{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
        docker rm "$CONTAINER_NAME" >/dev/null 2>&1 || true
        print_success "Container removed"
    fi
    
    # Remove volumes
    if docker volume ls --format "table {{.Name}}" | grep -q "^${DATA_VOLUME}$"; then
        docker volume rm "$DATA_VOLUME" >/dev/null 2>&1 || true
        print_success "Data volume removed"
    fi
    
    if docker volume ls --format "table {{.Name}}" | grep -q "^${CONFIG_VOLUME}$"; then
        docker volume rm "$CONFIG_VOLUME" >/dev/null 2>&1 || true
        print_success "Config volume removed"
    fi
    
    print_success "Cleanup completed!"
}

# Parse command line arguments
COMMAND="start"
FOREGROUND="false"
NO_PULL="false"

while [[ $# -gt 0 ]]; do
    case $1 in
        start|stop|restart|logs|status|clean|help)
            COMMAND="$1"
            shift
            ;;
        --api-port)
            API_PORT="$2"
            shift 2
            ;;
        --web-port)
            WEB_PORT="$2"
            shift 2
            ;;
        --image)
            IMAGE="$2"
            shift 2
            ;;
        --no-pull)
            NO_PULL="true"
            shift
            ;;
        --detach)
            FOREGROUND="false"
            shift
            ;;
        --foreground)
            FOREGROUND="true"
            shift
            ;;
        --help|-h)
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

# Check Docker
check_docker

# Execute command
case $COMMAND in
    start)
        pull_image
        start_pluresdb
        ;;
    stop)
        stop_pluresdb
        ;;
    restart)
        pull_image
        restart_pluresdb
        ;;
    logs)
        show_logs
        ;;
    status)
        show_status
        ;;
    clean)
        clean_up
        ;;
    help)
        show_usage
        ;;
    *)
        print_error "Unknown command: $COMMAND"
        show_usage
        exit 1
        ;;
esac
