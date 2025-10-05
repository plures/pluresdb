# Docker Packaging for PluresDB

This directory contains Docker configurations for running PluresDB in containerized environments.

## Quick Start

### Using Docker Compose (Recommended)

```bash
# Clone the repository
git clone https://github.com/plures/pluresdb.git
cd pluresdb/packaging/docker

# Start PluresDB
docker-compose up -d

# View logs
docker-compose logs -f

# Stop PluresDB
docker-compose down
```

### Using Docker directly

```bash
# Build the image
docker build -f packaging/docker/Dockerfile -t plures/pluresdb:latest .

# Run the container
docker run -p 34567:34567 -p 34568:34568 plures/pluresdb:latest

# Run with persistent storage
docker run -p 34567:34567 -p 34568:34568 \
  -v pluresdb-data:/app/data \
  -v pluresdb-config:/app/config \
  plures/pluresdb:latest
```

## Configuration Files

### docker-compose.yml

Basic Docker Compose configuration for development and testing.

### docker-compose.prod.yml

Production-ready configuration with:

- Resource limits
- Nginx reverse proxy (optional)
- SSL/TLS support
- Security headers
- Rate limiting

### nginx.conf

Nginx configuration for production deployment with:

- SSL termination
- Load balancing
- Security headers
- Rate limiting
- WebSocket support

## Environment Variables

| Variable              | Default       | Description      |
| --------------------- | ------------- | ---------------- |
| `PLURESDB_PORT`       | `34567`       | API server port  |
| `PLURESDB_WEB_PORT`   | `34568`       | Web UI port      |
| `PLURESDB_HOST`       | `0.0.0.0`     | Bind address     |
| `PLURESDB_DATA_DIR`   | `/app/data`   | Data directory   |
| `PLURESDB_CONFIG_DIR` | `/app/config` | Config directory |
| `PLURESDB_LOG_LEVEL`  | `info`        | Log level        |
| `PLURESDB_PRODUCTION` | `false`       | Production mode  |

## Volumes

- `pluresdb-data`: Persistent storage for application data
- `pluresdb-config`: Configuration files
- `redis-data`: Redis data (if using Redis profile)

## Ports

- `34567`: API server
- `34568`: Web UI
- `6379`: Redis (if enabled)
- `80`: HTTP (Nginx, if enabled)
- `443`: HTTPS (Nginx, if enabled)

## Profiles

### with-redis

Enables Redis for caching and session storage:

```bash
docker-compose --profile with-redis up -d
```

### with-nginx

Enables Nginx reverse proxy for production:

```bash
docker-compose --profile with-nginx up -d
```

## Production Deployment

### 1. Basic Production Setup

```bash
# Use production compose file
docker-compose -f docker-compose.prod.yml up -d
```

### 2. With Nginx Reverse Proxy

```bash
# Generate SSL certificates (replace with your domain)
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout ssl/key.pem -out ssl/cert.pem

# Start with Nginx
docker-compose -f docker-compose.prod.yml --profile with-nginx up -d
```

### 3. With Redis Caching

```bash
# Start with Redis
docker-compose -f docker-compose.prod.yml --profile with-redis up -d
```

## Health Checks

The container includes health checks that verify:

- API server is responding
- Web UI is accessible
- Database connectivity

View health status:

```bash
docker ps
docker inspect pluresdb --format='{{.State.Health.Status}}'
```

## Monitoring

### View Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f pluresdb
```

### Resource Usage

```bash
# Container stats
docker stats

# Specific container
docker stats pluresdb
```

## Security Considerations

1. **Non-root user**: Container runs as `pluresdb` user (UID 1001)
2. **Minimal base image**: Uses Alpine Linux for smaller attack surface
3. **Security headers**: Nginx configuration includes security headers
4. **Rate limiting**: Prevents abuse with request rate limiting
5. **SSL/TLS**: Production setup includes SSL termination

## Troubleshooting

### Container won't start

```bash
# Check logs
docker-compose logs pluresdb

# Check container status
docker ps -a
```

### Port conflicts

```bash
# Check what's using the ports
netstat -tulpn | grep :34567
netstat -tulpn | grep :34568

# Use different ports
docker run -p 8080:34567 -p 8081:34568 plures/pluresdb:latest
```

### Permission issues

```bash
# Fix volume permissions
docker-compose down
sudo chown -R 1001:1001 ./data
docker-compose up -d
```

### Database issues

```bash
# Reset database
docker-compose down -v
docker-compose up -d
```

## Development

### Building from source

```bash
# Build with development dependencies
docker build -f packaging/docker/Dockerfile \
  --target development \
  -t plures/pluresdb:dev .
```

### Running tests

```bash
# Run tests in container
docker run --rm plures/pluresdb:latest test
```

## Contributing

When modifying Docker configurations:

1. Test with both development and production setups
2. Verify health checks work correctly
3. Update documentation for any new environment variables
4. Test with different volume configurations
5. Verify security settings are appropriate

## Support

For Docker-related issues:

- Check the logs: `docker-compose logs -f`
- Verify configuration: `docker-compose config`
- Test connectivity: `curl http://localhost:34567/api/config`
- Open an issue on GitHub with container logs
