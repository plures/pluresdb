@{
    # Module manifest for PluresDB PowerShell module
    
    RootModule = 'PluresDB.psm1'
    ModuleVersion = '1.0.0'
    GUID = 'a7b3c4d5-e6f7-4a5b-9c8d-1e2f3a4b5c6d'
    Author = 'PluresDB Team'
    CompanyName = 'PluresDB'
    Copyright = '(c) 2026 PluresDB. All rights reserved.'
    Description = 'PowerShell module for PluresDB command history integration and database utilities'
    
    PowerShellVersion = '5.1'
    
    FunctionsToExport = @(
        'Initialize-PluresDBHistory',
        'Add-PluresDBCommand',
        'Get-PluresDBHistory',
        'Get-PluresDBCommandFrequency',
        'Get-PluresDBFailedCommands',
        'Get-PluresDBSessionHistory',
        'Get-PluresDBHostSummary',
        'Clear-PluresDBHistory',
        'Set-PluresDBConfig',
        'Enable-PluresDBHistoryIntegration',
        'Disable-PluresDBHistoryIntegration'
    )
    
    CmdletsToExport = @()
    VariablesToExport = @()
    AliasesToExport = @()
    
    PrivateData = @{
        PSData = @{
            Tags = @('PluresDB', 'Database', 'History', 'CommandLine', 'P2P')
            LicenseUri = 'https://github.com/plures/pluresdb/blob/main/LICENSE'
            ProjectUri = 'https://github.com/plures/pluresdb'
            ReleaseNotes = 'Initial release with command history integration'
        }
    }
}
