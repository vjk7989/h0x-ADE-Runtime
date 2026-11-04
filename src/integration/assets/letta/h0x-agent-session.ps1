# managed by HyperOx; reinstalling the integration replaces this file.
# H0X_INTEGRATION_ID=letta
# H0X_INTEGRATION_VERSION=2

param([string]$Action = "")

if ($Action -ne "session") { exit 0 }
if ($env:H0X_ENV -ne "1") { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:H0X_PANE_ID)) { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:H0X_SOCKET_PATH)) { exit 0 }

# Read hook input as raw bytes so UTF-8 payloads decode correctly even when the
# console input code page is not UTF-8 (for example CP949 on Korean Windows).
$stdinReader = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), [System.Text.Encoding]::UTF8)
$inputText = $stdinReader.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    $payload = $null
}
if ($null -eq $payload -or [string]::IsNullOrWhiteSpace($payload.conversation_id)) { exit 0 }

$sessionId = [string]$payload.conversation_id
if ($sessionId -eq "default") {
    if ([string]::IsNullOrWhiteSpace($payload.agent_id)) { exit 0 }
    $sessionId = "default:" + [string]$payload.agent_id
}

$h0x = if ([string]::IsNullOrWhiteSpace($env:H0X_BIN_PATH)) { "h0x" } else { $env:H0X_BIN_PATH }
$source = if ($payload.is_new_session -eq $true) { "new" } else { "resume" }
$commandArgs = @(
    "pane", "report-agent-session", $env:H0X_PANE_ID,
    "--source", "h0x:letta", "--agent", "letta",
    "--agent-session-id", $sessionId,
    "--seq", [string][DateTime]::UtcNow.Ticks,
    "--session-start-source", $source
)
$job = $null
try {
    $job = Start-Job -ScriptBlock {
        param($Executable, [object[]]$Arguments)
        & $Executable @Arguments *> $null
    } -ArgumentList $h0x, (,$commandArgs)
    if ($null -eq (Wait-Job -Job $job -Timeout 1)) {
        Stop-Job -Job $job
    }
} catch {
    # Hook failures must stay silent because Letta injects output into the next prompt.
    $null = $_
} finally {
    if ($null -ne $job) {
        Remove-Job -Job $job -Force -ErrorAction SilentlyContinue
    }
}
