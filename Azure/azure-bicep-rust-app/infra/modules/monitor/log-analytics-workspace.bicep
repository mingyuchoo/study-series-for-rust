metadata description = 'Creates an Azure Log Analytics workspace.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 Log Analytics workspace 이름입니다.')
param name string

@description('생성할 Log Analytics workspace의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 Log Analytics workspace의 태그입니다.')
param tags object = {}

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

resource logAnalyticsWorkspace 'Microsoft.OperationalInsights/workspaces@2026-03-01' = {
  name: name
  location: location
  tags: tags
  properties: {
    sku: {
      name: 'PerGB2018'
    }
    retentionInDays: 30
    features: {
      legacy: 0
      searchVersion: 1
      enableLogAccessUsingOnlyResourcePermissions: true
    }
    workspaceCapping: {
      dailyQuotaGb: -1
    }
    publicNetworkAccessForIngestion: 'Enabled'
    publicNetworkAccessForQuery: 'Enabled'
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Log Analytics workspace ID입니다.')
output id string = logAnalyticsWorkspace.id

@description('Log Analytics workspace 이름입니다.')
output name string = logAnalyticsWorkspace.name
