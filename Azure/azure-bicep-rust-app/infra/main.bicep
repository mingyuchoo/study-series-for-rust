targetScope = 'subscription'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@minLength(1)
@maxLength(32)
@description('리소스에 사용되는 고유 해시 생성을 위해 사용되는 환경 이름입니다.')
param environmentName string

@minLength(1)
@maxLength(32)
@description('모든 리소스의 기본 위치입니다.')
param location string

@minLength(1)
@maxLength(32)
@description('소유자 이름입니다.')
param ownerName string


////////////////////////////////////////////////////////////////////////////////
// Variables
////////////////////////////////////////////////////////////////////////////////
var abbrs = loadJsonContent('abbreviations.json')
var resourceToken = toLower(uniqueString(subscription().id, environmentName, location))
var tags = {
  'azd-env-name': environmentName
  owner: ownerName
}

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

// Resource Group
resource rg 'Microsoft.Resources/resourceGroups@2025-04-01' = {
  name: '${abbrs.resourcesResourceGroups}${environmentName}'
  location: location
  tags: tags
}

////////////////////////////////////////////////////////////////////////////////
// MONITORING SERVICES
////////////////////////////////////////////////////////////////////////////////

// Azure Log Analytics Workspace
module logAnalyticsWorkspace 'modules/monitor/log-analytics-workspace.bicep' = {
  scope: rg
  params: {
    name: 'choo${resourceToken}${abbrs.operationalInsightsWorkspaces}'
    location: rg.location
    tags: tags
  }
}

////////////////////////////////////////////////////////////////////////////////
// BACKEND CONTAINER SERVICE
////////////////////////////////////////////////////////////////////////////////

// Container Registry
module containerRegistry 'modules/compute/container-registry.bicep' = {
  scope: rg
  params: {
    name: 'choo${resourceToken}${abbrs.containerRegistryRegistries}'
    location: rg.location
    tags: tags
  }
}

// Container Apps Environment
module containerAppsEnvironment 'modules/compute/container-apps-environment.bicep' = {
  scope: rg
  params: {
    name: 'choo${resourceToken}${abbrs.appManagedEnvironments}'
    location: rg.location
    tags: tags
    logAnalyticsWorkspaceName: logAnalyticsWorkspace.outputs.name
  }
}

// Container App Initial - 기본 이미지로 생성
module containerAppBackendInit 'modules/compute/container-app-backend-init.bicep' = {
  scope: rg
  params: {
    name: 'choo${resourceToken}${abbrs.appContainerApps}-backend'
    location: rg.location
    tags: tags
    environmentName: containerAppsEnvironment.outputs.name
  }
}

// Assign AcrPull role to Container App
module roleAssignmentContainerAppBackend 'modules/compute/container-registry-role-assignment.bicep' = {
  scope: rg
  params: {
    registryName: containerRegistry.outputs.name
    principalId: containerAppBackendInit.outputs.principalId
  }
}

// Container App Update - 기본 이미지는 그대로 유지하고 권한과 설정만 변경해야 함
module containerAppBackendUpdate 'modules/compute/container-app-backend-update.bicep' = {
  dependsOn: [
    roleAssignmentContainerAppBackend
  ]
  scope: rg
  params: {
    name: 'choo${resourceToken}${abbrs.appContainerApps}-backend'
    location: rg.location
    tags: tags
    environmentName: containerAppsEnvironment.outputs.name
    registryName: containerRegistry.outputs.name
  }
}


//////////////////////////////////////////////////////////////////////////////////
//// AI FOUNDRY
//////////////////////////////////////////////////////////////////////////////////
//
//// Azure AI Foundry
//module aiFoundry 'modules/ai/ai-foundry.bicep' = {
//  scope: rg
//  params: {
//    name: 'choo${resourceToken}${abbrs.cognitiveServicesAccounts}'
//    location: rg.location
//    tags: tags
//    projectName: 'choo${resourceToken}${abbrs.cognitiveServicesAccounts}-prj'
//  }
//}
//
//
//////////////////////////////////////////////////////////////////////////////////
//// STORAGE
//////////////////////////////////////////////////////////////////////////////////
//
//// Storage Account
//module storageAccount 'modules/storage/storage-account.bicep' = {
//  scope: rg
//  params: {
//    name: 'choo${resourceToken}${abbrs.storageStorageAccounts}'
//    location: rg.location
//    tags: tags
//  }
//}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('리소스 그룹 ID입니다.')
output RESOURCE_GROUP_ID string = rg.id

@description('Azure Container Registry 엔드포인트입니다.')
output AZURE_CONTAINER_REGISTRY_ENDPOINT string = containerRegistry.outputs.loginServer

@description('백엔드 컨테이너 앱 엔드포인트입니다.')
output APP_BACKEND_ENDPOINT string = containerAppBackendUpdate.outputs.fqdn
