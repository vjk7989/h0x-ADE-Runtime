# installed by HyperOx
# managed by HyperOx; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# H0X_INTEGRATION_ID=grok
# H0X_INTEGRATION_VERSION=3

param([string]$Action = "")

if ($Action -ne "session") { exit 0 }
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

$event = if ($null -ne $payload -and $payload.hook_event_name -is [string]) {
    $payload.hook_event_name
} elseif ($null -ne $payload -and $payload.hookEventName -is [string]) {
    $payload.hookEventName
} else {
    $null
}
if ($null -ne $event -and $event -notin @("session_start", "SessionStart", "sessionStart")) { exit 0 }

$sessionStartSource = if ($null -ne $payload -and $payload.source -is [string]) {
    $payload.source
} else {
    $null
}

$sessionId = $env:GROK_SESSION_ID
if ([string]::IsNullOrWhiteSpace($sessionId) -and $null -ne $payload) {
    if ($payload.session_id -is [string]) { $sessionId = $payload.session_id }
    elseif ($payload.sessionId -is [string]) { $sessionId = $payload.sessionId }
}
if ([string]::IsNullOrWhiteSpace($sessionId)) { exit 0 }

$seq = [DateTime]::UtcNow.Ticks
$h0x = if ([string]::IsNullOrWhiteSpace($env:H0X_BIN_PATH)) { "h0x" } else { $env:H0X_BIN_PATH }
$h0xArgs = @(
    "pane", "report-agent-session", $env:H0X_PANE_ID,
    "--source", "h0x:grok",
    "--agent", "grok",
    "--seq", "$seq",
    "--agent-session-id", "$sessionId"
)
if (-not [string]::IsNullOrWhiteSpace($sessionStartSource)) {
    $h0xArgs += @("--session-start-source", "$sessionStartSource")
}
try {
    & $h0x @h0xArgs 2>$null | Out-Null
} catch {
}
