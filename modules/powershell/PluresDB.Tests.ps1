# PluresDB PowerShell Module Tests
# Run with: Invoke-Pester -Path modules/powershell/PluresDB.Tests.ps1

BeforeAll {
    # Import the module
    $modulePath = "$PSScriptRoot/PluresDB.psm1"
    Import-Module $modulePath -Force
    
    # Use a test database
    $script:TestDBPath = "$TestDrive/test-history.db"
    $script:PluresDBConfig.DBPath = $script:TestDBPath
}

AfterAll {
    # Clean up
    if (Test-Path $script:TestDBPath) {
        Remove-Item $script:TestDBPath -Force
    }
}

Describe "PluresDB PowerShell Module" {
    Context "Initialization" {
        It "Should initialize database" {
            # Note: This test assumes pluresdb CLI is available
            # In CI, this would be mocked or require pluresdb to be installed
            { Initialize-PluresDBHistory -DBPath $script:TestDBPath } | Should -Not -Throw
        }
    }
    
    Context "Command Recording" {
        It "Should add command to history" {
            { Add-PluresDBCommand -Command "Test-Command" -ExitCode 0 -Duration 100 } | Should -Not -Throw
        }
        
        It "Should ignore commands matching ignore patterns" {
            Set-PluresDBConfig -IgnorePatterns @("ls", "cd")
            
            # This should be silently ignored
            { Add-PluresDBCommand -Command "ls -la" -ExitCode 0 -Duration 10 } | Should -Not -Throw
        }
    }
    
    Context "Configuration" {
        It "Should update configuration" {
            { Set-PluresDBConfig -CaptureOutput $true } | Should -Not -Throw
            $script:PluresDBConfig.CaptureOutput | Should -Be $true
        }
        
        It "Should update max output size" {
            { Set-PluresDBConfig -MaxOutputSize 20480 } | Should -Not -Throw
            $script:PluresDBConfig.MaxOutputSize | Should -Be 20480
        }
        
        It "Should update ignore patterns" {
            $patterns = @("test1", "test2")
            { Set-PluresDBConfig -IgnorePatterns $patterns } | Should -Not -Throw
            $script:PluresDBConfig.IgnorePatterns | Should -Be $patterns
        }
    }
    
    Context "Module Functions" {
        It "Should export Initialize-PluresDBHistory function" {
            Get-Command Initialize-PluresDBHistory | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Add-PluresDBCommand function" {
            Get-Command Add-PluresDBCommand | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Get-PluresDBHistory function" {
            Get-Command Get-PluresDBHistory | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Get-PluresDBCommandFrequency function" {
            Get-Command Get-PluresDBCommandFrequency | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Get-PluresDBFailedCommands function" {
            Get-Command Get-PluresDBFailedCommands | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Set-PluresDBConfig function" {
            Get-Command Set-PluresDBConfig | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Enable-PluresDBHistoryIntegration function" {
            Get-Command Enable-PluresDBHistoryIntegration | Should -Not -BeNullOrEmpty
        }
        
        It "Should export Disable-PluresDBHistoryIntegration function" {
            Get-Command Disable-PluresDBHistoryIntegration | Should -Not -BeNullOrEmpty
        }
    }
}
