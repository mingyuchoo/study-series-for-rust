metadata description = 'Creates "QuickStart" Azure Container App for backend.'

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

////////////////////////////////////////////////////////////////////////////////
// Resources
////////////////////////////////////////////////////////////////////////////////

// Container App Environment 리소스 참조
resource containerAppsEnvironment 'Microsoft.App/managedEnvironments@2026-07-01' existing = {
  name: environmentName
}

resource containerApp 'Microsoft.App/containerapps@2026-07-01' = {
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
        targetPort: 80
        transport: 'Auto'
        traffic: [
          {
            weight: 100
            latestRevision: true
          }
        ]
        allowInsecure: false
      }
      maxInactiveRevisions: 100
      identitySettings: []
    }
    template: {
      containers: [
        {
          name: 'backend'
          image: 'mcr.microsoft.com/k8se/quickstart:latest' // default images
          resources: {
            cpu: json('0.25')
            memory: '0.5Gi'
          }
          env: []
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
