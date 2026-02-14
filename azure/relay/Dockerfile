# PluresDB Azure WSS Relay Server
FROM node:20-alpine

WORKDIR /app

# Install dependencies
COPY azure/relay/package.json azure/relay/package-lock.json* ./
RUN npm ci --production

# Copy server code
COPY azure/relay/server.ts ./

# Install TypeScript for running .ts directly
RUN npm install -g tsx

# Expose port 443 (WSS)
EXPOSE 443

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD node -e "require('http').get('http://localhost:${PORT:-443}/health', (r) => {process.exit(r.statusCode === 200 ? 0 : 1)})"

# Run the server
CMD ["tsx", "server.ts"]
