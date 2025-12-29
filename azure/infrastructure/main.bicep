// Main Bicep template for PluresDB Azure Relay Testing Infrastructure
// This template creates the core infrastructure for test, dev, and prod environments

@description('The environment name (test, dev, or prod)')
@allowed([
  'test'
  'dev'
  'prod'
])
param environment string = 'test'

@description('The Azure region for deployment')
param location string = resourceGroup().location

@description('The number of PluresDB nodes to deploy')
@minValue(1)
@maxValue(10)
param nodeCount int = 3

@description('VM size for PluresDB nodes')
param vmSize string = 'Standard_B2s'

@description('Admin username for VMs')
param adminUsername string = 'azureuser'

@description('SSH public key for VM access')
@secure()
param sshPublicKey string

// Variables
var resourcePrefix = 'pluresdb-${environment}'
var vnetName = '${resourcePrefix}-vnet'
var subnetName = '${resourcePrefix}-subnet'
var nsgName = '${resourcePrefix}-nsg'
var containerGroupName = '${resourcePrefix}-nodes'

// Virtual Network
resource vnet 'Microsoft.Network/virtualNetworks@2023-05-01' = {
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
resource nsg 'Microsoft.Network/networkSecurityGroups@2023-05-01' = {
  name: nsgName
  location: location
  properties: {
    securityRules: [
      {
        name: 'AllowPluresDBP2P'
        properties: {
          priority: 100
          direction: 'Inbound'
          access: 'Allow'
          protocol: 'Tcp'
          sourcePortRange: '*'
          destinationPortRange: '34567'
          sourceAddressPrefix: '*'
          destinationAddressPrefix: '*'
        }
      }
      {
        name: 'AllowPluresDBAPI'
        properties: {
          priority: 110
          direction: 'Inbound'
          access: 'Allow'
          protocol: 'Tcp'
          sourcePortRange: '*'
          destinationPortRange: '34568'
          sourceAddressPrefix: '*'
          destinationAddressPrefix: '*'
        }
      }
      {
        name: 'AllowSSH'
        properties: {
          priority: 120
          direction: 'Inbound'
          access: 'Allow'
          protocol: 'Tcp'
          sourcePortRange: '*'
          destinationPortRange: '22'
          sourceAddressPrefix: '*'
          destinationAddressPrefix: '*'
        }
      }
    ]
  }
}

// Storage Account for logs and data
resource storageAccount 'Microsoft.Storage/storageAccounts@2023-01-01' = {
  name: '${replace(resourcePrefix, '-', '')}storage'
  location: location
  sku: {
    name: environment == 'prod' ? 'Standard_GRS' : 'Standard_LRS'
  }
  kind: 'StorageV2'
  properties: {
    accessTier: 'Hot'
    minimumTlsVersion: 'TLS1_2'
    supportsHttpsTrafficOnly: true
  }
}

// Container Instances for PluresDB nodes
module nodes './node.bicep' = [for i in range(0, nodeCount): {
  name: '${resourcePrefix}-node-${i}'
  params: {
    location: location
    nodeName: '${resourcePrefix}-node-${i}'
    environment: environment
    nodeIndex: i
    totalNodes: nodeCount
  }
}]

// Outputs
output vnetId string = vnet.id
output storageAccountName string = storageAccount.name
output nodeNames array = [for i in range(0, nodeCount): '${resourcePrefix}-node-${i}']
output nodeIPs array = [for i in range(0, nodeCount): nodes[i].outputs.ipAddress]
