metadata description = 'Creates "backend" Azure Container App for backend.'

////////////////////////////////////////////////////////////////////////////////
// Input Parameters
////////////////////////////////////////////////////////////////////////////////
@description('생성할 backend Container App 이름입니다.')
param name string

@description('생성할 backend Container App의 위치입니다.')
param location string = resourceGroup().location

@description('생성할 backend Container App의 태그입니다.')
param tags object = {}

@description('생성할 backend Container App의 환경 이름입니다.')
param environmentName string

@description('Container Registry 이름입니다.')
param registryName string

// @description('Key Vault의 이름입니다.')
// param keyVaultName string

// @description('Key Vault의 엔드포인트입니다.')
// param vaultUri string

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

// Container Registry 리소스 참조 (registryName이 제공된 경우)
resource containerRegistry 'Microsoft.ContainerRegistry/registries@2025-11-01' existing = {
  name: registryName
}

// Container App Environment 리소스 참조
resource containerAppsEnvironment 'Microsoft.App/managedEnvironments@2026-07-01' existing = {
  name: environmentName
}

// // Key Vault 리소스 참조
// resource keyVault 'Microsoft.KeyVault/vaults@2024-11-01' existing = {
//   name: keyVaultName
// }

resource containerApp 'Microsoft.App/containerapps@2026-07-01' = {
  // dependsOn: [
  //   keyVault  // Key Vault가 먼저 생성되어야 함
  // ]
  name: name
  location: location
  tags: union(tags, { 'azd-service-name': 'backend' })
  identity: {
    type: 'SystemAssigned'
  }
  properties: {
    environmentId: containerAppsEnvironment.id
    configuration: {
      activeRevisionsMode: 'Single'
      ingress: {
        external: true
        targetPort: 8080 // .NET application port
        transport: 'Auto'
        traffic: [
          {
            weight: 100
            latestRevision: true
          }
        ]
        allowInsecure: false
      }
      registries: [
        {
          server: containerRegistry.properties.loginServer
          identity: 'system'
        }
      ]
      maxInactiveRevisions: 100
      identitySettings: []
    }
    template: {
      containers: [
        {
          name: 'backend'
          image: 'mcr.microsoft.com/k8se/quickstart:latest'
          resources: {
            cpu: json('0.25')
            memory: '0.5Gi'
          }
          env: [
            {
              name: 'AZURE_SQL_ADMIN_LOGIN'
              value: 'AZURE-SQL-ADMIN-LOGIN'
            }
            {
              name: 'AZURE_SQL_ADMIN_LOGIN_PASSWD'
              value: 'AZURE-SQL-ADMIN-LOGIN-PASSWD'
            }
            {
              name: 'AZURE_SQL_CONNECTION_STRING'
              value: 'AZURE-SQL-CONNECTION-STRING'
            }
            // {
            //   name: 'AZURE_VAULT_ENDPOINT'
            //   value: vaultUri      // 필요 시 vaultUri 파라미터를 함께 전달
            // }
            {
              name: 'AI_PROJECT_ENDPOINT'
              value: 'AI-PROJECT-ENDPOINT'
            }
            {
              name: 'AGENT_ID'
              value: 'AGENT-ID'
            }
            {
              name: 'MODEL_DEPLOYMENT_NAME'
              value: 'MODEL-DEPLOYMENT-NAME'
            }
          ]
        }
      ]
      scale: {
        minReplicas: 0
        maxReplicas: 10
        cooldownPeriod: 300
        pollingInterval: 30
      }
    }
  }
}


////////////////////////////////////////////////////////////////////////////////
// Output Values
////////////////////////////////////////////////////////////////////////////////
@description('backend Container App resource ID')
output id string = containerApp.id

@description('backend Container App name')
output name string = containerApp.name

@description('backend Container App FQDN')
output fqdn string = containerApp.properties.configuration.ingress.fqdn

@description('backend Container App principal ID')
output principalId string = containerApp.identity.principalId

@description('backend Container App default domain')
output defaultDomain string = containerApp.properties.latestRevisionFqdn
