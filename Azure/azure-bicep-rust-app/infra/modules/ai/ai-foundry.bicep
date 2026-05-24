metadata description = 'Creates an Azure AI Foundry.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 AI Foundry 이름입니다.')
param name string

@description('생성할 AI Foundry의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 AI Foundry의 태그입니다.')
param tags object = {}

@description('생성할 AI Foundry의 프로젝트 이름입니다.')
param projectName string 

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

// Azure AI Foundry
resource aiFoundry 'Microsoft.CognitiveServices/accounts@2024-12-01-preview' = {
  name: name
  location: location
  tags: tags
  sku: {
    name: 'S0'
  }
  kind: 'AIServices'
  identity: {
    type: 'SystemAssigned'
  }
  properties: {
    apiProperties: {}
    customSubDomainName: name
    networkAcls: {
      defaultAction: 'Allow'
      virtualNetworkRules: []
      ipRules: []
    }
    defaultProject: projectName
    associatedProjects: [
      projectName
    ]
    allowProjectManagement: true
    publicNetworkAccess: 'Enabled'
  }
}

// Azure AI Foundry Project
resource aiFoundryProject 'Microsoft.CognitiveServices/accounts/projects@2024-12-01-preview' = {
  parent: aiFoundry
  name: projectName
  location: location
  identity: {
    type: 'SystemAssigned'
  }
  properties: {
    description: 'Default project created with the resource'
    displayName: projectName
  }
}

resource modelDeployment_gpt_4_1 'Microsoft.CognitiveServices/accounts/deployments@2024-12-01-preview' = {
  dependsOn: [
    aiFoundryProject
  ]
  parent: aiFoundry
  name: 'gpt-4.1'
  sku: {
    name: 'GlobalStandard'
    capacity: 250
  }
  properties: {
    model: {
      format: 'OpenAI'
      name: 'gpt-4.1'
      version: '2025-04-14'
    }
    versionUpgradeOption: 'OnceNewDefaultVersionAvailable'
    currentCapacity: 250
    raiPolicyName: 'Microsoft.DefaultV2'
  }
}

resource modelDeployment_gpt_4_1_mini 'Microsoft.CognitiveServices/accounts/deployments@2024-12-01-preview' = {
  dependsOn: [
    modelDeployment_gpt_4_1
  ]
  parent: aiFoundry
  name: 'gpt-4.1-mini'
  sku: {
    name: 'GlobalStandard'
    capacity: 250
  }
  properties: {
    model: {
      format: 'OpenAI'
      name: 'gpt-4.1-mini'
      version: '2025-04-14'
    }
    versionUpgradeOption: 'OnceNewDefaultVersionAvailable'
    currentCapacity: 250
    raiPolicyName: 'Microsoft.DefaultV2'
  }
}

resource modelDeployment_text_embedding_3_large 'Microsoft.CognitiveServices/accounts/deployments@2024-12-01-preview' = {
  dependsOn: [
    modelDeployment_gpt_4_1_mini
  ]
  parent: aiFoundry
  name: 'text-embedding-3-large'
  sku: {
    name: 'GlobalStandard'
    capacity: 250
  }
  properties: {
    model: {
      format: 'OpenAI'
      name: 'text-embedding-3-large'
      version: '1'
    }
    versionUpgradeOption: 'NoAutoUpgrade'
    currentCapacity: 250
    raiPolicyName: 'Microsoft.DefaultV2'
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('AI Foundry ID입니다.')
output id string = aiFoundry.id

@description('AI Foundry 이름입니다.')
output name string = aiFoundry.name

@description('AI Foundry 프로젝트 엔드포인트입니다.')
output projectEndpoint string = aiFoundryProject.properties.endpoints['AI Foundry API']
