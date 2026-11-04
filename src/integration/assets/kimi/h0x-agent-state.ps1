# installed by HyperOx
# managed by HyperOx; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# H0X_INTEGRATION_ID=kimi
# H0X_INTEGRATION_VERSION=8

param([string]$Action = "")

if (@("session", "working", "blocked", "idle") -notcontains $Action) { exit 0 }
if ($env:H0X_ENV -ne "1") { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:H0X_PANE_ID)) { exit 0 }

# Read hook input as raw bytes so UTF-8 payloads decode correctly even when the
# console input code page is not UTF-8 (for example CP949 on Korean Windows).
$stdinReader = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), [System.Text.Encoding]::UTF8)
$inputText = $stdinReader.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    $payload = $null
}

$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$sessionId = if ($null -ne $payload -and -not [string]::IsNullOrWhiteSpace($payload.session_id)) { $payload.session_id } else { $null }
$h0x = if ([string]::IsNullOrWhiteSpace($env:H0X_BIN_PATH)) { "h0x" } else { $env:H0X_BIN_PATH }

try {
    if ($Action -eq "session") {
        if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }
        & $h0x pane report-agent-session $env:H0X_PANE_ID --source h0x:kimi --agent kimi --agent-session-id $sessionId --session-start-source startup --seq $seq 2>$null | Out-Null
    } else {
        if ([string]::IsNullOrWhiteSpace($sessionId)) {
            & $h0x pane report-agent $env:H0X_PANE_ID --source h0x:kimi --agent kimi --state $Action --seq $seq 2>$null | Out-Null
        } else {
            & $h0x pane report-agent $env:H0X_PANE_ID --source h0x:kimi --agent kimi --state $Action --agent-session-id $sessionId --seq $seq 2>$null | Out-Null
        }
    }
} catch {
}
