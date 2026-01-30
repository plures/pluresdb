# Example: Setting up PowerShell History Integration with PluresDB
# This script demonstrates how to configure and use PluresDB command history

# 1. Import the module
Import-Module PluresDB

# 2. Initialize the database
Write-Host "`n=== Initializing PluresDB History ===" -ForegroundColor Cyan
Initialize-PluresDBHistory

# 3. Configure settings
Write-Host "`n=== Configuring PluresDB ===" -ForegroundColor Cyan
Set-PluresDBConfig -CaptureOutput $false  # Don't capture output for performance
Set-PluresDBConfig -IgnorePatterns @("ls", "dir", "cd", "pwd")  # Ignore common commands

# 4. Manually add some example commands (simulating history)
Write-Host "`n=== Adding Example Commands ===" -ForegroundColor Cyan
Add-PluresDBCommand -Command "Get-Process" -ExitCode 0 -Duration 150
Add-PluresDBCommand -Command "git status" -ExitCode 0 -Duration 45
Add-PluresDBCommand -Command "git commit -m 'test'" -ExitCode 1 -Duration 120 -ErrorOutput "nothing to commit"
Add-PluresDBCommand -Command "npm install" -ExitCode 0 -Duration 5400
Add-PluresDBCommand -Command "dotnet build" -ExitCode 0 -Duration 3200

# 5. Query the history
Write-Host "`n=== Recent Command History ===" -ForegroundColor Cyan
Get-PluresDBHistory -Last 5 | Format-Table -AutoSize

# 6. Show command frequency
Write-Host "`n=== Command Frequency ===" -ForegroundColor Cyan
Get-PluresDBCommandFrequency -Top 5

# 7. Show failed commands
Write-Host "`n=== Failed Commands ===" -ForegroundColor Cyan
Get-PluresDBFailedCommands -Last 3

# 8. Query with filters
Write-Host "`n=== Git Commands Only ===" -ForegroundColor Cyan
Get-PluresDBHistory -CommandLike "git*" -Last 10

# 9. Show unique commands
Write-Host "`n=== Unique Commands ===" -ForegroundColor Cyan
Get-PluresDBHistory -Unique -Last 5 | Format-Table -AutoSize

# 10. Show host summary
Write-Host "`n=== Host Summary ===" -ForegroundColor Cyan
Get-PluresDBHostSummary | Format-Table -AutoSize

# 11. Enable automatic history integration
Write-Host "`n=== Enabling Automatic History Integration ===" -ForegroundColor Cyan
Write-Host "To enable automatic history capture, run:" -ForegroundColor Yellow
Write-Host "  Enable-PluresDBHistoryIntegration" -ForegroundColor Yellow
Write-Host "  . `$PROFILE  # Reload your profile" -ForegroundColor Yellow
Write-Host ""
Write-Host "To disable automatic history capture, run:" -ForegroundColor Yellow
Write-Host "  Disable-PluresDBHistoryIntegration" -ForegroundColor Yellow

# 12. Advanced queries using PluresDB CLI
Write-Host "`n=== Advanced Query: Slowest Commands ===" -ForegroundColor Cyan
$query = @"
SELECT command, avg_duration_ms, total_executions
FROM command_frequency
WHERE total_executions > 1
ORDER BY avg_duration_ms DESC
LIMIT 5
"@

Write-Host "Query:" -ForegroundColor Gray
Write-Host $query -ForegroundColor DarkGray
Write-Host "`nResults:" -ForegroundColor Gray
$results = pluresdb query --db "$env:USERPROFILE\.pluresdb\history.db" $query | ConvertFrom-Json
$results | Format-Table -AutoSize

Write-Host "`n=== Example Complete ===" -ForegroundColor Green
Write-Host "Database location: $env:USERPROFILE\.pluresdb\history.db" -ForegroundColor Gray
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "1. Run 'Enable-PluresDBHistoryIntegration' to auto-capture history" -ForegroundColor White
Write-Host "2. Reload your profile: . `$PROFILE" -ForegroundColor White
Write-Host "3. Use 'Get-PluresDBHistory' to query your command history" -ForegroundColor White
Write-Host "4. Use 'Get-PluresDBCommandFrequency' to see frequently used commands" -ForegroundColor White
