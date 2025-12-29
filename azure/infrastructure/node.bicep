// Bicep template for individual PluresDB node deployment

@description('The Azure region for deployment')
param location string

@description('The name for this PluresDB node')
param nodeName string

@description('The environment name (test, dev, or prod)')
param environment string

@description('The index of this node')
param nodeIndex int

@description('Total number of nodes in the deployment')
param totalNodes int

// Container Instance for PluresDB node
resource containerGroup 'Microsoft.ContainerInstance/containerGroups@2023-05-01' = {
  name: nodeName
  location: location
  properties: {
    containers: [
      {
        name: 'pluresdb'
        properties: {
          image: 'plures/pluresdb:latest'
          resources: {
            requests: {
              cpu: 1
              memoryInGB: 2
            }
          }
          ports: [
            {
              port: 34567
              protocol: 'TCP'
            }
            {
              port: 34568
              protocol: 'TCP'
            }
          ]
          environmentVariables: [
            {
              name: 'NODE_ENV'
              value: environment
            }
            {
              name: 'NODE_INDEX'
              value: string(nodeIndex)
            }
            {
              name: 'TOTAL_NODES'
              value: string(totalNodes)
            }
            {
              name: 'PLURESDB_PORT'
              value: '34567'
            }
            {
              name: 'PLURESDB_API_PORT'
              value: '34568'
            }
          ]
        }
      }
    ]
    osType: 'Linux'
    restartPolicy: 'Always'
    ipAddress: {
      type: 'Public'
      ports: [
        {
          port: 34567
          protocol: 'TCP'
        }
        {
          port: 34568
          protocol: 'TCP'
        }
      ]
    }
  }
}

output containerGroupId string = containerGroup.id
output ipAddress string = containerGroup.properties.ipAddress.ip
