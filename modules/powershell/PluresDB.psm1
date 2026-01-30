# PluresDB PowerShell Module
# Command history integration and database utilities for PowerShell

# Module version
$script:ModuleVersion = "1.0.0"

# Module configuration
$script:PluresDBConfig = @{
    DBPath = "$env:USERPROFILE\.pluresdb\history.db"
    ApiEndpoint = "http://localhost:34567"
    Hostname = $env:COMPUTERNAME
    ShellType = "powershell"
    ShellVersion = $PSVersionTable.PSVersion.ToString()
    Username = $env:USERNAME
    SessionId = [guid]::NewGuid().ToString()
    CaptureOutput = $false
    CaptureEnv = $false
    MaxOutputSize = 10240
    IgnorePatterns = @()
}

# Import required assemblies
Add-Type -AssemblyName System.Net.Http

#region Core Database Functions

function Initialize-PluresDBHistory {
    <#
    .SYNOPSIS
        Initialize PluresDB command history database
    .DESCRIPTION
        Creates the database schema for command history tracking
    .PARAMETER DBPath
        Path to the database file (default: ~/.pluresdb/history.db)
    .EXAMPLE
        Initialize-PluresDBHistory
    #>
    [CmdletBinding()]
    param(
        [string]$DBPath = $script:PluresDBConfig.DBPath
    )
    
    # Ensure directory exists
    $dbDir = Split-Path -Parent $DBPath
    if (-not (Test-Path $dbDir)) {
        New-Item -ItemType Directory -Path $dbDir -Force | Out-Null
    }
    
    # Read schema file
    $schemaPath = Join-Path $PSScriptRoot "..\shared\schema.sql"
    if (Test-Path $schemaPath) {
        $schema = Get-Content $schemaPath -Raw
        
        # Execute schema using pluresdb CLI
        $schema | pluresdb query --db $DBPath -
        
        Write-Host "✅ PluresDB command history initialized at: $DBPath" -ForegroundColor Green
    }
    else {
        Write-Warning "Schema file not found at: $schemaPath"
    }
}

function Add-PluresDBCommand {
    <#
    .SYNOPSIS
        Add a command to PluresDB history
    .DESCRIPTION
        Records a command execution in the PluresDB history database
    .PARAMETER Command
        The command text
    .PARAMETER ExitCode
        Command exit code
    .PARAMETER Duration
        Command duration in milliseconds
    .PARAMETER Output
        Command output (optional)
    .PARAMETER ErrorOutput
        Command error output (optional)
    .EXAMPLE
        Add-PluresDBCommand -Command "Get-Process" -ExitCode 0 -Duration 150
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Command,
        
        [int]$ExitCode = 0,
        
        [int]$Duration = 0,
        
        [string]$Output = "",
        
        [string]$ErrorOutput = "",
        
        [string]$WorkingDirectory = (Get-Location).Path
    )
    
    # Check ignore patterns
    foreach ($pattern in $script:PluresDBConfig.IgnorePatterns) {
        if ($Command -like $pattern) {
            return
        }
    }
    
    $timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    
    # Truncate output if needed
    if ($script:PluresDBConfig.CaptureOutput -and $Output.Length -gt $script:PluresDBConfig.MaxOutputSize) {
        $Output = $Output.Substring(0, $script:PluresDBConfig.MaxOutputSize) + "... (truncated)"
    }
    
    # Build insert query
    $query = @"
INSERT INTO command_history 
    (command, hostname, shell_type, shell_version, username, working_directory, 
     timestamp, duration_ms, exit_code, output, error_output, session_id)
VALUES 
    (@command, @hostname, @shell_type, @shell_version, @username, @working_directory,
     @timestamp, @duration_ms, @exit_code, @output, @error_output, @session_id)
"@
    
    try {
        # Use pluresdb CLI to insert
        $params = @{
            command = $Command
            hostname = $script:PluresDBConfig.Hostname
            shell_type = $script:PluresDBConfig.ShellType
            shell_version = $script:PluresDBConfig.ShellVersion
            username = $script:PluresDBConfig.Username
            working_directory = $WorkingDirectory
            timestamp = $timestamp
            duration_ms = $Duration
            exit_code = $ExitCode
            output = if ($script:PluresDBConfig.CaptureOutput) { $Output } else { "" }
            error_output = $ErrorOutput
            session_id = $script:PluresDBConfig.SessionId
        } | ConvertTo-Json -Compress
        
        # Execute via pluresdb
        $params | pluresdb query --db $script:PluresDBConfig.DBPath $query
    }
    catch {
        Write-Warning "Failed to add command to history: $_"
    }
}

function Get-PluresDBHistory {
    <#
    .SYNOPSIS
        Query command history from PluresDB
    .DESCRIPTION
        Retrieves command history with various filtering options
    .PARAMETER CommandLike
        Filter commands matching pattern
    .PARAMETER Hostname
        Filter by hostname
    .PARAMETER ShellType
        Filter by shell type
    .PARAMETER SuccessOnly
        Show only successful commands
    .PARAMETER FailedOnly
        Show only failed commands
    .PARAMETER Last
        Number of recent commands to return
    .PARAMETER Unique
        Show only unique commands (deduplicated)
    .EXAMPLE
        Get-PluresDBHistory -Last 10
    .EXAMPLE
        Get-PluresDBHistory -CommandLike "git*" -SuccessOnly
    .EXAMPLE
        Get-PluresDBHistory -FailedOnly -Last 5
    #>
    [CmdletBinding()]
    param(
        [string]$CommandLike,
        
        [string]$Hostname = $script:PluresDBConfig.Hostname,
        
        [string]$ShellType,
        
        [switch]$SuccessOnly,
        
        [switch]$FailedOnly,
        
        [int]$Last = 100,
        
        [switch]$Unique
    )
    
    $conditions = @()
    
    if ($CommandLike) {
        $conditions += "command LIKE '$CommandLike'"
    }
    
    if ($Hostname) {
        $conditions += "hostname = '$Hostname'"
    }
    
    if ($ShellType) {
        $conditions += "shell_type = '$ShellType'"
    }
    
    if ($SuccessOnly) {
        $conditions += "is_success = 1"
    }
    
    if ($FailedOnly) {
        $conditions += "is_success = 0"
    }
    
    $whereClause = if ($conditions.Count -gt 0) {
        "WHERE " + ($conditions -join " AND ")
    } else {
        ""
    }
    
    $table = if ($Unique) { "command_history_unique" } else { "command_history" }
    
    $query = "SELECT * FROM $table $whereClause ORDER BY timestamp DESC LIMIT $Last"
    
    pluresdb query --db $script:PluresDBConfig.DBPath $query | ConvertFrom-Json
}

function Get-PluresDBCommandFrequency {
    <#
    .SYNOPSIS
        Get command frequency statistics
    .DESCRIPTION
        Shows most frequently used commands
    .PARAMETER Top
        Number of top commands to show
    .EXAMPLE
        Get-PluresDBCommandFrequency -Top 20
    #>
    [CmdletBinding()]
    param(
        [int]$Top = 10
    )
    
    $query = "SELECT * FROM command_frequency LIMIT $Top"
    pluresdb query --db $script:PluresDBConfig.DBPath $query | ConvertFrom-Json
}

function Get-PluresDBFailedCommands {
    <#
    .SYNOPSIS
        Get failed commands for troubleshooting
    .DESCRIPTION
        Retrieves commands that failed execution
    .PARAMETER Last
        Number of recent failed commands
    .EXAMPLE
        Get-PluresDBFailedCommands -Last 10
    #>
    [CmdletBinding()]
    param(
        [int]$Last = 10
    )
    
    $query = "SELECT * FROM failed_commands LIMIT $Last"
    pluresdb query --db $script:PluresDBConfig.DBPath $query | ConvertFrom-Json
}

function Get-PluresDBSessionHistory {
    <#
    .SYNOPSIS
        Get session history statistics
    .DESCRIPTION
        Shows command statistics grouped by session
    .EXAMPLE
        Get-PluresDBSessionHistory
    #>
    [CmdletBinding()]
    param()
    
    $query = "SELECT * FROM session_history"
    pluresdb query --db $script:PluresDBConfig.DBPath $query | ConvertFrom-Json
}

function Get-PluresDBHostSummary {
    <#
    .SYNOPSIS
        Get host summary statistics
    .DESCRIPTION
        Shows command statistics per host
    .EXAMPLE
        Get-PluresDBHostSummary
    #>
    [CmdletBinding()]
    param()
    
    $query = "SELECT * FROM host_summary"
    pluresdb query --db $script:PluresDBConfig.DBPath $query | ConvertFrom-Json
}

function Clear-PluresDBHistory {
    <#
    .SYNOPSIS
        Clear command history
    .DESCRIPTION
        Removes old command history entries
    .PARAMETER OlderThanDays
        Remove entries older than specified days
    .PARAMETER Confirm
        Confirm before deletion
    .EXAMPLE
        Clear-PluresDBHistory -OlderThanDays 90
    #>
    [CmdletBinding(SupportsShouldProcess)]
    param(
        [int]$OlderThanDays = 90
    )
    
    $cutoffTimestamp = [DateTimeOffset]::UtcNow.AddDays(-$OlderThanDays).ToUnixTimeMilliseconds()
    
    if ($PSCmdlet.ShouldProcess("Command history older than $OlderThanDays days", "Delete")) {
        $query = "DELETE FROM command_history WHERE timestamp < $cutoffTimestamp"
        pluresdb query --db $script:PluresDBConfig.DBPath $query
        Write-Host "✅ Cleared command history older than $OlderThanDays days" -ForegroundColor Green
    }
}

function Set-PluresDBConfig {
    <#
    .SYNOPSIS
        Configure PluresDB module settings
    .DESCRIPTION
        Updates module configuration
    .PARAMETER CaptureOutput
        Enable/disable output capture
    .PARAMETER MaxOutputSize
        Maximum output size in bytes
    .PARAMETER IgnorePatterns
        Patterns to ignore
    .EXAMPLE
        Set-PluresDBConfig -CaptureOutput $true -MaxOutputSize 20480
    #>
    [CmdletBinding()]
    param(
        [bool]$CaptureOutput,
        
        [int]$MaxOutputSize,
        
        [string[]]$IgnorePatterns
    )
    
    if ($PSBoundParameters.ContainsKey('CaptureOutput')) {
        $script:PluresDBConfig.CaptureOutput = $CaptureOutput
    }
    
    if ($PSBoundParameters.ContainsKey('MaxOutputSize')) {
        $script:PluresDBConfig.MaxOutputSize = $MaxOutputSize
    }
    
    if ($PSBoundParameters.ContainsKey('IgnorePatterns')) {
        $script:PluresDBConfig.IgnorePatterns = $IgnorePatterns
    }
    
    Write-Host "✅ PluresDB configuration updated" -ForegroundColor Green
}

#endregion

#region History Integration

function Enable-PluresDBHistoryIntegration {
    <#
    .SYNOPSIS
        Enable automatic command history capture
    .DESCRIPTION
        Sets up PowerShell profile to automatically capture command history
    .EXAMPLE
        Enable-PluresDBHistoryIntegration
    #>
    [CmdletBinding()]
    param()
    
    $profilePath = $PROFILE.CurrentUserAllHosts
    $integrationCode = @'

# PluresDB History Integration
$PluresDBHistoryEnabled = $true

# Import PluresDB module
Import-Module PluresDB -ErrorAction SilentlyContinue

# Hook into command execution
$ExecutionContext.InvokeCommand.CommandNotFoundAction = {
    param($CommandName, $CommandLookupEventArgs)
    # This fires before command lookup, we'll use a different approach
}

# Use PSReadLine for command history integration
if (Get-Module PSReadLine) {
    Set-PSReadLineOption -AddToHistoryHandler {
        param($line)
        
        # Record command start time
        $global:PluresDBCommandStart = Get-Date
        $global:PluresDBLastCommand = $line
        
        return $true
    }
    
    # Hook into prompt to capture command results
    $global:PluresDBOriginalPrompt = Get-Command prompt -ErrorAction SilentlyContinue
    
    function global:prompt {
        if ($global:PluresDBLastCommand) {
            $duration = 0
            if ($global:PluresDBCommandStart) {
                $duration = ((Get-Date) - $global:PluresDBCommandStart).TotalMilliseconds
            }
            
            Add-PluresDBCommand -Command $global:PluresDBLastCommand `
                               -ExitCode $LASTEXITCODE `
                               -Duration $duration
            
            $global:PluresDBLastCommand = $null
        }
        
        # Call original prompt
        if ($global:PluresDBOriginalPrompt) {
            & $global:PluresDBOriginalPrompt
        } else {
            "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
        }
    }
}
'@
    
    if (-not (Test-Path $profilePath)) {
        New-Item -ItemType File -Path $profilePath -Force | Out-Null
    }
    
    $currentProfile = Get-Content $profilePath -Raw -ErrorAction SilentlyContinue
    
    if ($currentProfile -notlike "*PluresDB History Integration*") {
        Add-Content -Path $profilePath -Value $integrationCode
        Write-Host "✅ PluresDB history integration enabled in profile: $profilePath" -ForegroundColor Green
        Write-Host "   Restart PowerShell or run: . `$PROFILE" -ForegroundColor Yellow
    }
    else {
        Write-Host "⚠️  PluresDB history integration already enabled" -ForegroundColor Yellow
    }
}

function Disable-PluresDBHistoryIntegration {
    <#
    .SYNOPSIS
        Disable automatic command history capture
    .DESCRIPTION
        Removes PluresDB integration from PowerShell profile
    .EXAMPLE
        Disable-PluresDBHistoryIntegration
    #>
    [CmdletBinding()]
    param()
    
    $profilePath = $PROFILE.CurrentUserAllHosts
    
    if (Test-Path $profilePath) {
        $content = Get-Content $profilePath -Raw
        $updatedContent = $content -replace "(?s)# PluresDB History Integration.*?^}", ""
        Set-Content -Path $profilePath -Value $updatedContent
        
        Write-Host "✅ PluresDB history integration disabled" -ForegroundColor Green
        Write-Host "   Restart PowerShell to apply changes" -ForegroundColor Yellow
    }
}

#endregion

#region Export

Export-ModuleMember -Function @(
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

#endregion
