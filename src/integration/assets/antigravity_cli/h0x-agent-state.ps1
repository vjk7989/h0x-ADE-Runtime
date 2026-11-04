# installed by HyperOx
# managed by HyperOx; reinstalling or updating the integration overwrites this file.
# add custom hooks beside this file instead of editing it.
# H0X_INTEGRATION_ID=antigravity_cli
# H0X_INTEGRATION_VERSION=4

# Session-only: this hook reports the Antigravity conversation so HyperOx can
# resume the pane. Lifecycle state comes from HyperOx's screen detection.

param([string]$Action = "")

# Antigravity CLI expects a JSON object on stdout and this hook never injects
# anything, so every exit path emits an empty object.
function Exit-Hook {
    Write-Output "{}"
    exit 0
}

if ($Action -ne "session") { Exit-Hook }
if ($env:H0X_ENV -ne "1") { Exit-Hook }
if ([string]::IsNullOrWhiteSpace($env:H0X_PANE_ID)) { Exit-Hook }

# Read hook input as raw bytes so UTF-8 payloads decode correctly even when the
# console input code page is not UTF-8 (for example CP949 on Korean Windows).
$stdinReader = New-Object System.IO.StreamReader([Console]::OpenStandardInput(), [System.Text.Encoding]::UTF8)
$inputText = $stdinReader.ReadToEnd()
try {
    $payload = if ([string]::IsNullOrWhiteSpace($inputText)) { $null } else { $inputText | ConvertFrom-Json }
} catch {
    Exit-Hook
}

if ($null -eq $payload) { Exit-Hook }

$conversationId = if ($payload.conversationId -is [string]) { $payload.conversationId } else { $null }
if ([string]::IsNullOrWhiteSpace($conversationId)) { Exit-Hook }

$seq = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$h0x = if ([string]::IsNullOrWhiteSpace($env:H0X_BIN_PATH)) { "h0x" } else { $env:H0X_BIN_PATH }
try {
    $sessionArgs = @(
        "pane",
        "report-agent-session",
        $env:H0X_PANE_ID,
        "--source",
        "h0x:antigravity_cli",
        "--agent",
        "agy",
        "--seq",
        "$seq",
        "--agent-session-id",
        "$conversationId"
    )
    if ($payload.transcriptPath -is [string] -and -not [string]::IsNullOrWhiteSpace($payload.transcriptPath)) {
        $sessionArgs += @("--agent-session-path", "$($payload.transcriptPath)")
    }
    & $h0x @sessionArgs 2>$null | Out-Null
} catch {
}

Exit-Hook
