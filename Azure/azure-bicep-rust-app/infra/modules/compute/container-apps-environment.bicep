metadata description = 'Creates an Azure Container Apps Environment.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 Container Apps Environment 이름입니다.')
param name string

@description('생성할 Container Apps Environment의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 Container Apps Environment의 태그입니다.')
param tags object = {}

@description('생성할 Container Apps Environment의 태그입니다.')
param logAnalyticsWorkspaceName string

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////


// Log Analytics Workspace 리소스 참조
resource logAnalyticsWorkspace 'Microsoft.OperationalInsights/workspaces@2026-03-01' existing = {
  name: logAnalyticsWorkspaceName
}


resource containerAppsEnvironment 'Microsoft.App/managedEnvironments@2026-07-01' = {
  name: name
  location: location
  tags: tags
  properties: {
    appLogsConfiguration: {
      destination: 'log-analytics'
      logAnalyticsConfiguration: {
        customerId: logAnalyticsWorkspace.properties.customerId
        sharedKey: logAnalyticsWorkspace.listKeys().primarySharedKey
      }
    }
    zoneRedundant: false
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Container Apps Environment ID입니다.')
output id string = containerAppsEnvironment.id

@description('Container Apps Environment 이름입니다.')
output name string = containerAppsEnvironment.name
