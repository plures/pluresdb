// Rusty Gun Azure Infrastructure - Main Bicep Template
// This template deploys the complete Azure infrastructure for Rusty Gun hosted services

@description('The name of the resource group')
param resourceGroupName string = 'rusty-gun-rg'

@description('The Azure region for all resources')
param location string = resourceGroup().location

@description('Environment name (dev, staging, prod)')
@allowed(['dev', 'staging', 'prod'])
param environment string = 'dev'

@description('Application name')
param appName string = 'rusty-gun'

@description('Admin username for PostgreSQL')
@secure()
param adminUsername string = 'rustygundbadmin'

@description('Admin password for PostgreSQL')
@secure()
param adminPassword string

@description('SKU for App Service Plan')
@allowed(['B1', 'B2', 'S1', 'S2', 'S3', 'P1V2', 'P2V2', 'P3V2'])
param appServicePlanSku string = 'P1V2'

@description('SKU for PostgreSQL')
@allowed(['B_Gen5_1', 'B_Gen5_2', 'GP_Gen5_2', 'GP_Gen5_4', 'GP_Gen5_8', 'MO_Gen5_2', 'MO_Gen5_4', 'MO_Gen5_8'])
param postgresqlSku string = 'GP_Gen5_2'

@description('Enable high availability')
param enableHA bool = false

@description('Enable monitoring')
param enableMonitoring bool = true

@description('Enable security features')
param enableSecurity bool = true

// Variables
var appServicePlanName = '${appName}-asp-${environment}'
var webAppName = '${appName}-web-${environment}'
var containerRegistryName = '${appName}acr${environment}'
var postgresqlServerName = '${appName}-db-${environment}'
var postgresqlDatabaseName = '${appName}_db'
var keyVaultName = '${appName}-kv-${environment}'
var storageAccountName = '${appName}storage${environment}'
var redisCacheName = '${appName}-redis-${environment}'
var applicationInsightsName = '${appName}-ai-${environment}'
var logAnalyticsWorkspaceName = '${appName}-law-${environment}'
var vnetName = '${appName}-vnet-${environment}'
var subnetName = '${appName}-subnet-${environment}'
var nsgName = '${appName}-nsg-${environment}'

// Resource Group
resource resourceGroup 'Microsoft.Resources/resourceGroups@2021-04-01' = {
  name: resourceGroupName
  location: location
}

// Virtual Network
resource vnet 'Microsoft.Network/virtualNetworks@2021-05-01' = if (enableSecurity) {
  name: vnetName
  location: location
  properties: {
    addressSpace: {
      addressPrefixes: [
        '10.0.0.0/16'
      ]
    }
    subnets: [
      {
        name: subnetName
        properties: {
          addressPrefix: '10.0.1.0/24'
          networkSecurityGroup: {
            id: nsg.id
          }
        }
      }
    ]
  }
}

// Network Security Group
resource nsg 'Microsoft.Network/networkSecurityGroups@2021-05-01' = if (enableSecurity) {
  name: nsgName
  location: location
  properties: {
    securityRules: [
      {
        name: 'AllowHTTPS'
        properties: {
          priority: 1000
          access: 'Allow'
          direction: 'Inbound'
          destinationPortRange: '443'
          protocol: 'Tcp'
          sourceAddressPrefix: 'Internet'
          destinationAddressPrefix: '*'
        }
      }
      {
        name: 'AllowHTTP'
        properties: {
          priority: 1010
          access: 'Allow'
          direction: 'Inbound'
          destinationPortRange: '80'
          protocol: 'Tcp'
          sourceAddressPrefix: 'Internet'
          destinationAddressPrefix: '*'
        }
      }
      {
        name: 'AllowSSH'
        properties: {
          priority: 1020
          access: 'Allow'
          direction: 'Inbound'
          destinationPortRange: '22'
          protocol: 'Tcp'
          sourceAddressPrefix: 'Internet'
          destinationAddressPrefix: '*'
        }
      }
    ]
  }
}

// Storage Account
resource storageAccount 'Microsoft.Storage/storageAccounts@2021-06-01' = {
  name: storageAccountName
  location: location
  sku: {
    name: 'Standard_LRS'
  }
  kind: 'StorageV2'
  properties: {
    accessTier: 'Hot'
    supportsHttpsTrafficOnly: true
    minimumTlsVersion: 'TLS1_2'
    allowBlobPublicAccess: false
  }
}

// Container Registry
resource containerRegistry 'Microsoft.ContainerRegistry/registries@2021-06-01-preview' = {
  name: containerRegistryName
  location: location
  sku: {
    name: 'Standard'
  }
  properties: {
    adminUserEnabled: true
  }
}

// Log Analytics Workspace
resource logAnalyticsWorkspace 'Microsoft.OperationalInsights/workspaces@2021-06-01' = if (enableMonitoring) {
  name: logAnalyticsWorkspaceName
  location: location
  properties: {
    sku: {
      name: 'PerGB2018'
    }
    retentionInDays: 30
  }
}

// Application Insights
resource applicationInsights 'Microsoft.Insights/components@2020-02-02' = if (enableMonitoring) {
  name: applicationInsightsName
  location: location
  kind: 'web'
  properties: {
    Application_Type: 'web'
    WorkspaceResourceId: logAnalyticsWorkspace.id
  }
}

// Key Vault
resource keyVault 'Microsoft.KeyVault/vaults@2021-06-01-preview' = if (enableSecurity) {
  name: keyVaultName
  location: location
  properties: {
    sku: {
      family: 'A'
      name: 'standard'
    }
    tenantId: subscription().tenantId
    accessPolicies: []
    enabledForDeployment: false
    enabledForDiskEncryption: false
    enabledForTemplateDeployment: true
    enableSoftDelete: true
    softDeleteRetentionInDays: 90
    enableRbacAuthorization: true
  }
}

// PostgreSQL Server
resource postgresqlServer 'Microsoft.DBforPostgreSQL/flexibleServers@2021-06-01' = {
  name: postgresqlServerName
  location: location
  sku: {
    name: postgresqlSku
    tier: postgresqlSku == 'B_Gen5_1' || postgresqlSku == 'B_Gen5_2' ? 'Burstable' : 'GeneralPurpose'
  }
  properties: {
    administratorLogin: adminUsername
    administratorLoginPassword: adminPassword
    storage: {
      storageSizeGB: 32
    }
    backup: {
      backupRetentionDays: 7
      geoRedundantBackup: enableHA ? 'Enabled' : 'Disabled'
    }
    highAvailability: enableHA ? {
      mode: 'ZoneRedundant'
    } : null
    version: '13'
  }
}

// PostgreSQL Database
resource postgresqlDatabase 'Microsoft.DBforPostgreSQL/flexibleServers/databases@2021-06-01' = {
  parent: postgresqlServer
  name: postgresqlDatabaseName
  properties: {
    charset: 'utf8'
    collation: 'en_US.utf8'
  }
}

// Redis Cache
resource redisCache 'Microsoft.Cache/redis@2021-06-01' = {
  name: redisCacheName
  location: location
  properties: {
    sku: {
      name: 'Standard'
      family: 'C'
      capacity: 1
    }
    enableNonSslPort: false
    minimumTlsVersion: '1.2'
  }
}

// App Service Plan
resource appServicePlan 'Microsoft.Web/serverfarms@2021-03-01' = {
  name: appServicePlanName
  location: location
  sku: {
    name: appServicePlanSku
    tier: appServicePlanSku == 'B1' || appServicePlanSku == 'B2' ? 'Basic' : 
          appServicePlanSku == 'S1' || appServicePlanSku == 'S2' || appServicePlanSku == 'S3' ? 'Standard' : 'PremiumV2'
    capacity: 1
  }
  kind: 'linux'
  properties: {
    reserved: true
  }
}

// Web App
resource webApp 'Microsoft.Web/sites@2021-03-01' = {
  name: webAppName
  location: location
  kind: 'app,linux,container'
  properties: {
    serverFarmId: appServicePlan.id
    siteConfig: {
      linuxFxVersion: 'DOCKER|${containerRegistry.properties.loginServer}/rusty-gun:latest'
      alwaysOn: true
      httpLoggingEnabled: true
      logsDirectorySizeLimit: 35
      detailedErrorLoggingEnabled: true
      publishingUsername: '${appName}-publish'
      scmType: 'None'
      use32BitWorkerProcess: false
      webSocketsEnabled: true
      managedPipelineMode: 'Integrated'
      virtualApplications: [
        {
          virtualPath: '/'
          physicalPath: 'site\\wwwroot'
          preloadEnabled: true
        }
      ]
      appSettings: [
        {
          name: 'WEBSITES_ENABLE_APP_SERVICE_STORAGE'
          value: 'false'
        }
        {
          name: 'DOCKER_REGISTRY_SERVER_URL'
          value: 'https://${containerRegistry.properties.loginServer}'
        }
        {
          name: 'DOCKER_REGISTRY_SERVER_USERNAME'
          value: containerRegistry.listCredentials().username
        }
        {
          name: 'DOCKER_REGISTRY_SERVER_PASSWORD'
          value: containerRegistry.listCredentials().passwords[0].value
        }
        {
          name: 'DOCKER_CUSTOM_IMAGE_NAME'
          value: '${containerRegistry.properties.loginServer}/rusty-gun:latest'
        }
        {
          name: 'RUSTY_GUN_PORT'
          value: '34567'
        }
        {
          name: 'RUSTY_GUN_WEB_PORT'
          value: '34568'
        }
        {
          name: 'RUSTY_GUN_HOST'
          value: '0.0.0.0'
        }
        {
          name: 'RUSTY_GUN_DATABASE_URL'
          value: 'postgresql://${adminUsername}:${adminPassword}@${postgresqlServer.properties.fullyQualifiedDomainName}:5432/${postgresqlDatabaseName}?sslmode=require'
        }
        {
          name: 'RUSTY_GUN_REDIS_URL'
          value: 'redis://${redisCache.properties.hostName}:6380'
        }
        {
          name: 'RUSTY_GUN_STORAGE_ACCOUNT'
          value: storageAccount.properties.primaryEndpoints.blob
        }
        {
          name: 'RUSTY_GUN_STORAGE_KEY'
          value: storageAccount.listKeys().keys[0].value
        }
        {
          name: 'RUSTY_GUN_KEY_VAULT_URL'
          value: keyVault.properties.vaultUri
        }
        {
          name: 'RUSTY_GUN_APPLICATION_INSIGHTS_KEY'
          value: applicationInsights.properties.InstrumentationKey
        }
        {
          name: 'RUSTY_GUN_ENVIRONMENT'
          value: environment
        }
        {
          name: 'RUSTY_GUN_PRODUCTION'
          value: environment == 'prod' ? 'true' : 'false'
        }
      ]
      connectionStrings: [
        {
          name: 'DefaultConnection'
          connectionString: 'postgresql://${adminUsername}:${adminPassword}@${postgresqlServer.properties.fullyQualifiedDomainName}:5432/${postgresqlDatabaseName}?sslmode=require'
          type: 'PostgreSQL'
        }
      ]
    }
    httpsOnly: true
    clientAffinityEnabled: false
  }
}

// Outputs
output resourceGroupName string = resourceGroup.name
output webAppName string = webApp.name
output webAppUrl string = 'https://${webApp.properties.defaultHostName}'
output containerRegistryName string = containerRegistry.name
output containerRegistryUrl string = containerRegistry.properties.loginServer
output postgresqlServerName string = postgresqlServer.name
output postgresqlServerFqdn string = postgresqlServer.properties.fullyQualifiedDomainName
output redisCacheName string = redisCache.name
output redisCacheHostName string = redisCache.properties.hostName
output keyVaultName string = keyVault.name
output keyVaultUrl string = keyVault.properties.vaultUri
output applicationInsightsName string = applicationInsights.name
output applicationInsightsKey string = applicationInsights.properties.InstrumentationKey
output storageAccountName string = storageAccount.name
output storageAccountUrl string = storageAccount.properties.primaryEndpoints.blob
