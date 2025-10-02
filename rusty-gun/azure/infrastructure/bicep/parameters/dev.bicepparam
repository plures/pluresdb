using '../main.bicep'

param resourceGroupName = 'rusty-gun-dev-rg'
param environment = 'dev'
param appName = 'rusty-gun'
param adminUsername = 'rustygundbadmin'
param adminPassword = 'DevPassword123!'
param appServicePlanSku = 'B1'
param postgresqlSku = 'B_Gen5_1'
param enableHA = false
param enableMonitoring = true
param enableSecurity = true
