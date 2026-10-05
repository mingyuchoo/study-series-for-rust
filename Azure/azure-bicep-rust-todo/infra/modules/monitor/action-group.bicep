metadata description = 'Creates an Azure Action Group.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 Action Group 이름입니다.')
param name string

@description('생성할 Action Group의 태그입니다.')
param tags object = {}

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

resource actionGroup 'Microsoft.Insights/actionGroups@2023-01-01' = {
  name: name
  location: 'Global'
  tags: tags
  properties: {
    groupShortName: 'SmartDetect'
    enabled: true
    emailReceivers: []
    smsReceivers: []
    webhookReceivers: []
    eventHubReceivers: []
    itsmReceivers: []
    azureAppPushReceivers: []
    automationRunbookReceivers: []
    voiceReceivers: []
    logicAppReceivers: []
    azureFunctionReceivers: []
    armRoleReceivers: [
      {
        name: 'Monitoring Contributor'
        roleId: '749f88d5-cbae-40b8-bcfc-e573ddc772fa'
        useCommonAlertSchema: true
      }
      {
        name: 'Monitoring Reader'
        roleId: '43d0d8ad-25c7-4714-9337-8ba259a9fe05'
        useCommonAlertSchema: true
      }
    ]
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Action Group의 리소스 ID입니다.')
output id string = actionGroup.id

@description('Action Group의 이름입니다.')
output name string = actionGroup.name
