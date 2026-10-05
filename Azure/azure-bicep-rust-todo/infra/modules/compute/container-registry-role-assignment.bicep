metadata description = '역할 할당을 생성합니다.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('Container Registry 이름')
param registryName string

@description('Container App 관리형 ID')
param principalId string

@description('Role definition ID for AcrPull')
param roleDefinitionId string = '7f951dda-4ed3-4680-a7ca-43fe172d538d' // AcrPull role

////////////////////////////////////////////////////////////////////////////////
// Resource
////////////////////////////////////////////////////////////////////////////////

// Container Registry 리소스 참조 (registryName이 제공된 경우)
resource containerRegistry 'Microsoft.ContainerRegistry/registries@2025-11-01' existing = {
  name: registryName
}

// AcrPull 역할 할당 - Container App 관리형 ID 가 ACR 이미지를 끌어올 수 있도록 권한 부여
resource roleAssignment 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(containerRegistry.id, principalId, roleDefinitionId)
  scope: containerRegistry
  properties: {
    principalId: principalId
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleDefinitionId)
    principalType: 'ServicePrincipal'
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Role assignment resource ID')
output id string = roleAssignment.id
