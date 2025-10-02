#!/bin/sh

# Azure App Service startup script for Rusty Gun
# Handles Azure-specific configuration and startup

set -e

echo "Starting Rusty Gun on Azure App Service..."

# Azure App Service sets PORT environment variable
if [ -n "$PORT" ]; then
    echo "Using Azure App Service port: $PORT"
    export RUSTY_GUN_PORT=$PORT
fi

# Set Azure-specific environment variables
export RUSTY_GUN_HOST=0.0.0.0
export RUSTY_GUN_PRODUCTION=true
export RUSTY_GUN_AZURE=true

# Create data directory if it doesn't exist
mkdir -p /app/data

# Set up logging
export RUSTY_GUN_LOG_LEVEL=${RUSTY_GUN_LOG_LEVEL:-info}
export RUSTY_GUN_LOG_FILE=/app/logs/rusty-gun.log

# Create logs directory
mkdir -p /app/logs

# Azure App Service health check endpoint
echo "Setting up health check endpoint..."

# Start the application
echo "Starting Rusty Gun application..."
exec deno run -A --unstable-kv src/main.ts serve \
    --port $RUSTY_GUN_PORT \
    --web-port $RUSTY_GUN_WEB_PORT \
    --host $RUSTY_GUN_HOST
