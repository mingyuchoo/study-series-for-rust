metadata description = 'Creates an Azure Application Insights.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 Application Insights 이름입니다.')
param name string

@description('생성할 Application Insights의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 Application Insights의 태그입니다.')
param tags object = {}

@description('Log Analytics workspace의 리소스 ID입니다.')
param workspaceId string



////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

resource applicationInsights 'microsoft.insights/components@2020-02-02' = {
  name: name
  location: location
  tags: tags
  kind: 'web'
  properties: {
    Application_Type: 'web'
    Flow_Type: 'Redfield'
    Request_Source: 'IbizaAIExtension'
    RetentionInDays: 90
    WorkspaceResourceId: workspaceId
    IngestionMode: 'LogAnalytics'
    publicNetworkAccessForIngestion: 'Enabled'
    publicNetworkAccessForQuery: 'Enabled'
  }
}


////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Application Insights ID입니다.')
output id string = applicationInsights.id

@description('Application Insights 이름입니다.')
output name string = applicationInsights.name

