metadata description = 'Creates an Azure Container Registry.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 Container Registry 이름입니다.')
param name string

@description('생성할 Container Registry의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 Container Registry의 태그입니다.')
param tags object = {}

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////
resource containerRegistry 'Microsoft.ContainerRegistry/registries@2025-11-01' = {
  name: name
  location: location
  tags: tags
  sku: {
    name: 'Basic'
  }
  properties: {
    adminUserEnabled: false
    anonymousPullEnabled: false
    publicNetworkAccess: 'Enabled'
    zoneRedundancy: 'Disabled'
    policies: {
      quarantinePolicy: {
        status: 'disabled'
      }
      trustPolicy: {
        type: 'Notary'
        status: 'disabled'
      }
      retentionPolicy: {
        days: 7
        status: 'disabled'
      }
    }
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Container Registry ID입니다.')
output id string = containerRegistry.id

@description('Container Registry 이름입니다.')
output name string = containerRegistry.name

@description('Container Registry 로그인 서버입니다.')
output loginServer string = containerRegistry.properties.loginServer
