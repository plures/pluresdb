// Bicep template for PluresDB Azure WSS Relay Server
// Corporate-safe WebSocket relay on port 443 (looks like HTTPS traffic)

@description('The environment name (test, dev, or prod)')
@allowed([
  'test'
  'dev'
  'prod'
])
param environment string = 'test'

@description('The Azure region for deployment')
param location string = resourceGroup().location

// Variables
var resourcePrefix = 'pluresdb-relay-${environment}'
var containerName = '${resourcePrefix}-container'
var relayImage = 'plures/pluresdb-relay:latest'

// Container Instance for relay server
resource relayContainer 'Microsoft.ContainerInstance/containerGroups@2023-09-01' = {
  name: containerName
  location: location
  properties: {
    containers: [
      {
        name: 'relay'
        properties: {
          image: relayImage
          resources: {
            requests: {
              cpu: 1
              memoryInGB: 1
            }
          }
          ports: [
            {
              port: 443
              protocol: 'TCP'
            }
            {
              port: 80
              protocol: 'TCP'
            }
          ]
          environmentVariables: [
            {
              name: 'NODE_ENV'
              value: environment
            }
            {
              name: 'PORT'
              value: '443'
            }
            {
              name: 'ENABLE_TLS'
              value: 'true'
            }
          ]
        }
      }
    ]
    osType: 'Linux'
    restartPolicy: 'Always'
    ipAddress: {
      type: 'Public'
      dnsNameLabel: resourcePrefix
      ports: [
        {
          port: 443
          protocol: 'TCP'
        }
        {
          port: 80
          protocol: 'TCP'
        }
      ]
    }
  }
}

// Outputs
output relayUrl string = 'wss://${relayContainer.properties.ipAddress.fqdn}'
output relayIp string = relayContainer.properties.ipAddress.ip
output relayFqdn string = relayContainer.properties.ipAddress.fqdn
