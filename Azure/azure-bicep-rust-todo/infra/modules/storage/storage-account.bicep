metadata description = 'Creates an Azure storage account.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 Storage Account 이름입니다.')
param name string

@description('생성할 Storage Account의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 Storage Account의 태그입니다.')
param tags object = {}

@description('생성할 Storage Account의 종류입니다.')
param kind string = 'StorageV2'

@description('생성할 Storage Account의 SKU입니다.')
param sku object = { name: 'Standard_LRS' }

@description('생성할 Storage Account의 접근 계층입니다.')
@allowed([
  'Cool'
  'Hot'
  'Premium'
])
param accessTier string = 'Hot'

@description('생성할 Storage Account의 컨테이너입니다.')
param containers array = [
  {
    name: 'documents'
  }
]

@description('생성할 Storage Account의 삭제 정책입니다.')
param deleteRetentionPolicy object = {}

////////////////////////////////////////////////////////////////////////////////
// Resource
////////////////////////////////////////////////////////////////////////////////
resource storageAccount 'Microsoft.Storage/storageAccounts@2026-06-01' = {
  name: name
  location: location
  tags: tags
  kind: kind
  sku: sku
  properties: {
    accessTier: accessTier
  }
  resource blobServices 'blobServices' = if (!empty(containers)) {
    name: 'default'
    properties: {
      deleteRetentionPolicy: deleteRetentionPolicy
    }
    resource container 'containers' = [
      for container in containers: {
        name: container.name
        properties: {
          publicAccess: container.?publicAccess ?? 'None'
        }
      }
    ]
  }
}

////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('Storage Account ID입니다.')
output id string = storageAccount.id

@description('Storage Account 이름입니다.')
output name string = storageAccount.name

@description('Storage Account의 Primary Endpoints입니다.')
output primaryEndpoints object = storageAccount.properties.primaryEndpoints
