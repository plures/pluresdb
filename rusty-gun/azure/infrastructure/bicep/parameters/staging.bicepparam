using '../main.bicep'

param resourceGroupName = 'rusty-gun-staging-rg'
param environment = 'staging'
param appName = 'rusty-gun'
param adminUsername = 'rustygundbadmin'
param adminPassword = 'StagingPassword123!'
param appServicePlanSku = 'S1'
param postgresqlSku = 'GP_Gen5_2'
param enableHA = false
param enableMonitoring = true
param enableSecurity = true
