# installed by HyperOx
# managed by HyperOx; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# H0X_INTEGRATION_ID=copilot
# H0X_INTEGRATION_VERSION=4

if ($env:H0X_ENV -ne "1") { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:H0X_PANE_ID)) { exit 0 }
if ([string]::IsNullOrWhiteSpace($env:H0X_SOCKET_PATH)) { exit 0 }

# Read hook input as raw bytes so UTF-8 payloads decode correctly even when the
# console input code page is not UTF-8 (for example CP949 on Korean Windows).
$stdinReader = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), [System.Text.Encoding]::UTF8)
$inputText = $stdinReader.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { @{} } else { $inputText | ConvertFrom-Json }
} catch {
    $payload = @{}
}

function First-Text {
    param([object[]]$Names)
    foreach ($name in $Names) {
        $value = $payload.$name
        if ($value -is [string] -and -not [string]::IsNullOrWhiteSpace($value)) {
            return $value
        }
    }
    return $null
}

function Normalize-Event {
    param([string]$Event)
    if ([string]::IsNullOrWhiteSpace($Event)) { return "" }
    return $Event.Replace("_", "").Replace("-", "").ToLowerInvariant()
}

$eventName = First-Text @("hook_event_name", "hookEventName")
if ($eventName) {
    if ((Normalize-Event $eventName) -ne "sessionstart") { exit 0 }
} elseif (
    ($payload.PSObject.Properties.Name -contains "prompt") -or
    (First-Text @("tool_name", "toolName", "notification_type", "notificationType", "stop_reason", "stopReason", "reason"))
) {
    exit 0
}

$sessionId = $payload.session_id
if ([string]::IsNullOrWhiteSpace($sessionId)) {
    $sessionId = $payload.sessionId
}
if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }

$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$h0x = if ([string]::IsNullOrWhiteSpace($env:H0X_BIN_PATH)) { "h0x" } else { $env:H0X_BIN_PATH }
& $h0x pane report-agent-session $env:H0X_PANE_ID --source h0x:copilot --agent copilot --agent-session-id $sessionId --seq $seq 2>$null | Out-Null
