# Rusty Gun Azure Deployment Guide

This guide covers deploying Rusty Gun to Azure for commercial hosting and support services.

## Quick Start

### Prerequisites

1. **Azure CLI** - Install from [Azure CLI](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
2. **Docker** - Install from [Docker](https://www.docker.com/get-started)
3. **Azure Subscription** - With appropriate permissions
4. **PowerShell** (Windows) or **Bash** (Linux/macOS)

### 1. Login to Azure

```bash
# Login to Azure
az login

# Set subscription (optional)
az account set --subscription "your-subscription-id"
```

### 2. Deploy to Development

```bash
# Using PowerShell (Windows)
.\azure\deploy.ps1 -Environment dev

# Using Bash (Linux/macOS)
./azure/deploy.sh -e dev
```

### 3. Deploy to Production

```bash
# Using PowerShell (Windows)
.\azure\deploy.ps1 -Environment prod -ResourceGroupName "my-rusty-gun-prod"

# Using Bash (Linux/macOS)
./azure/deploy.sh -e prod -g "my-rusty-gun-prod"
```

## Deployment Options

### Environment-Specific Deployments

| Environment | Resource Group | App Service Plan | Database | Purpose |
|-------------|----------------|------------------|----------|---------|
| `dev` | `rusty-gun-dev-rg` | B1 | B_Gen5_1 | Development |
| `staging` | `rusty-gun-staging-rg` | S1 | GP_Gen5_2 | Testing |
| `prod` | `rusty-gun-prod-rg` | P1V2 | GP_Gen5_4 | Production |

### Custom Deployment

```bash
# Custom resource group and location
./azure/deploy.sh -e prod -g "my-custom-rg" -l "West US 2"

# Skip specific steps
./azure/deploy.sh -e dev --skip-infrastructure --skip-container

# What-if deployment (preview changes)
./azure/deploy.sh -e prod --what-if
```

## Infrastructure Components

### Core Services
- **Azure App Service** - Hosts Rusty Gun application
- **Azure Database for PostgreSQL** - Data storage
- **Azure Redis Cache** - Caching and session storage
- **Azure Container Registry** - Container image storage
- **Azure Storage Account** - File storage

### Security Services
- **Azure Key Vault** - Secrets management
- **Azure Virtual Network** - Network isolation
- **Azure Application Gateway** - Load balancing and WAF
- **Azure Active Directory** - Authentication

### Monitoring Services
- **Azure Monitor** - Infrastructure monitoring
- **Application Insights** - Application performance monitoring
- **Azure Log Analytics** - Centralized logging
- **Azure Alerts** - Proactive monitoring

## Cost Optimization

### Resource Sizing

#### Development Environment
- **App Service Plan**: B1 (1 vCPU, 1.75 GB RAM)
- **PostgreSQL**: B_Gen5_1 (1 vCore, 2 GB RAM)
- **Redis**: Basic C0 (250 MB)
- **Estimated Cost**: ~$50/month

#### Staging Environment
- **App Service Plan**: S1 (1 vCPU, 1.75 GB RAM)
- **PostgreSQL**: GP_Gen5_2 (2 vCores, 10 GB RAM)
- **Redis**: Standard C1 (1 GB)
- **Estimated Cost**: ~$150/month

#### Production Environment
- **App Service Plan**: P1V2 (1 vCPU, 3.5 GB RAM)
- **PostgreSQL**: GP_Gen5_4 (4 vCores, 20 GB RAM)
- **Redis**: Premium P1 (6 GB)
- **Estimated Cost**: ~$400/month

### Cost Optimization Strategies

1. **Right-sizing**: Monitor usage and adjust resource sizes
2. **Reserved Instances**: Use 1-year or 3-year reservations for production
3. **Auto-scaling**: Scale resources based on demand
4. **Scheduling**: Stop development environments outside business hours
5. **Storage Optimization**: Use appropriate storage tiers

## Monitoring and Alerting

### Key Metrics

#### Application Metrics
- **Response Time**: < 200ms average
- **Availability**: > 99.9%
- **Error Rate**: < 0.1%
- **Throughput**: Requests per second

#### Infrastructure Metrics
- **CPU Usage**: < 70%
- **Memory Usage**: < 80%
- **Disk Usage**: < 85%
- **Network Latency**: < 100ms

#### Business Metrics
- **Active Users**: Daily/monthly active users
- **Revenue**: Monthly recurring revenue
- **Churn Rate**: Customer retention
- **Support Tickets**: Volume and resolution time

### Alert Rules

#### Critical Alerts
- Service down or unavailable
- Database connection failures
- High error rates (> 5%)
- Security incidents

#### Warning Alerts
- High CPU usage (> 80%)
- High memory usage (> 85%)
- Slow response times (> 500ms)
- Disk space low (> 90%)

## Security Configuration

### Network Security
- **VNet Integration**: Isolate application in private network
- **Private Endpoints**: Secure database and storage access
- **WAF Protection**: Web Application Firewall
- **DDoS Protection**: Distributed denial-of-service protection

### Identity and Access
- **Azure AD Integration**: Single sign-on
- **RBAC**: Role-based access control
- **Key Vault**: Secure secrets management
- **Managed Identities**: No secrets in code

### Compliance
- **SOC 2**: Security controls
- **GDPR**: Data protection
- **ISO 27001**: Information security
- **HIPAA**: Healthcare compliance (if needed)

## Support and Maintenance

### Support Tiers

#### Community Support
- **Response Time**: 72 hours
- **Channels**: GitHub Issues, Discord
- **Scope**: Bug reports, feature requests

#### Professional Support
- **Response Time**: 24 hours
- **Channels**: Email, phone
- **Scope**: Technical issues, configuration help

#### Enterprise Support
- **Response Time**: 4 hours
- **Channels**: Dedicated support engineer
- **Scope**: Custom integrations, SLA guarantees

### Maintenance Windows
- **Development**: Anytime
- **Staging**: Weekends, 2 AM - 6 AM
- **Production**: Sundays, 2 AM - 4 AM

### Backup Strategy
- **Database**: Daily automated backups, 30-day retention
- **Application**: Container image backups
- **Configuration**: Infrastructure as Code
- **Disaster Recovery**: Multi-region deployment

## Troubleshooting

### Common Issues

#### Deployment Failures
```bash
# Check Azure CLI login
az account show

# Verify resource group exists
az group show --name "rusty-gun-dev-rg"

# Check container registry access
az acr login --name "rusty-gun"
```

#### Application Issues
```bash
# Check app service logs
az webapp log tail --name "rusty-gun-web-dev" --resource-group "rusty-gun-dev-rg"

# Check app service status
az webapp show --name "rusty-gun-web-dev" --resource-group "rusty-gun-dev-rg"
```

#### Database Issues
```bash
# Check database status
az postgres flexible-server show --name "rusty-gun-db-dev" --resource-group "rusty-gun-dev-rg"

# Check database logs
az postgres flexible-server logs list --name "rusty-gun-db-dev" --resource-group "rusty-gun-dev-rg"
```

### Performance Issues

#### Slow Response Times
1. Check CPU and memory usage
2. Review database query performance
3. Check Redis cache hit rates
4. Monitor network latency

#### High Error Rates
1. Check application logs
2. Review database connection pool
3. Check external service dependencies
4. Monitor resource limits

## Scaling

### Horizontal Scaling
- **App Service**: Scale out to multiple instances
- **Database**: Read replicas for read-heavy workloads
- **Redis**: Cluster mode for high availability

### Vertical Scaling
- **App Service Plan**: Upgrade to higher tier
- **Database**: Increase vCores and memory
- **Redis**: Upgrade to higher tier

### Auto-scaling Rules
```yaml
# CPU-based scaling
- metric: CPU Percentage
  threshold: 70%
  action: Scale out
  instances: +1

# Memory-based scaling
- metric: Memory Percentage
  threshold: 80%
  action: Scale out
  instances: +1

# Request-based scaling
- metric: Requests per minute
  threshold: 1000
  action: Scale out
  instances: +1
```

## Cost Management

### Budget Alerts
- **Monthly Budget**: $500
- **Warning Threshold**: 80% ($400)
- **Critical Threshold**: 100% ($500)

### Cost Optimization
1. **Right-size Resources**: Match resources to actual usage
2. **Use Reserved Instances**: 1-year or 3-year commitments
3. **Implement Auto-scaling**: Scale based on demand
4. **Schedule Resources**: Stop dev environments at night
5. **Monitor Usage**: Regular cost analysis and optimization

## Next Steps

1. **Deploy Development Environment**: Start with dev deployment
2. **Configure Monitoring**: Set up alerts and dashboards
3. **Implement CI/CD**: Automate deployments
4. **Set up Support System**: Configure ticketing and SLA monitoring
5. **Launch Production**: Deploy to production with monitoring

## Support

- **Documentation**: [Azure Docs](https://docs.microsoft.com/en-us/azure/)
- **Issues**: [GitHub Issues](https://github.com/rusty-gun/rusty-gun/issues)
- **Discord**: [Join our Discord](https://discord.gg/rusty-gun)
- **Email**: support@rusty-gun.com
