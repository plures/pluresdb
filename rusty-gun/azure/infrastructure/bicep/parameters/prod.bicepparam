using '../main.bicep'

param resourceGroupName = 'rusty-gun-prod-rg'
param environment = 'prod'
param appName = 'rusty-gun'
param adminUsername = 'rustygundbadmin'
param adminPassword = 'ProdPassword123!'
param appServicePlanSku = 'P1V2'
param postgresqlSku = 'GP_Gen5_4'
param enableHA = true
param enableMonitoring = true
param enableSecurity = true
