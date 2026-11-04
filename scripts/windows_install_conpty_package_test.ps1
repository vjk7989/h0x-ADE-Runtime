param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$installerPath = (Resolve-Path -LiteralPath "$PSScriptRoot\..\distribution\install.ps1").Path
$bootstrapPath = (Resolve-Path -LiteralPath "$PSScriptRoot\..\distribution\install.cmd").Path
$bootstrapContent = Get-Content -LiteralPath $bootstrapPath -Raw
foreach ($forbiddenCommand in @("Invoke-RestMethod", "Invoke-WebRequest", "Invoke-Expression", "iex")) {
    if ($bootstrapContent -match "(?i)\b$forbiddenCommand\b") {
        throw "CMD bootstrap uses forbidden PowerShell network execution: $forbiddenCommand"
    }
}
if ($bootstrapContent -notmatch "(?i)\bcurl\.exe\b") {
    throw "CMD bootstrap does not download through curl.exe"
}
$parseErrors = $null
$tokens = $null
$installerAst = [System.Management.Automation.Language.Parser]::ParseFile(
    $installerPath,
    [ref]$tokens,
    [ref]$parseErrors
)
if ($parseErrors.Count -ne 0) {
    throw ($parseErrors | Out-String)
}
$forbiddenPowerShellCommands = @(
    $installerAst.FindAll(
        { param($node) $node -is [System.Management.Automation.Language.CommandAst] },
        $true
    ) |
        ForEach-Object { $_.GetCommandName() } |
        Where-Object {
            $_ -in @("Invoke-RestMethod", "Invoke-WebRequest", "Invoke-Expression", "irm", "iwr", "iex")
        }
)
if ($forbiddenPowerShellCommands.Count -ne 0) {
    throw "installer uses forbidden PowerShell network execution: $($forbiddenPowerShellCommands -join ', ')"
}
foreach ($functionName in @("Prepend-PathEntry", "Update-PathRegistryEntry")) {
    $definition = $installerAst.FindAll(
        {
            param($node)
            $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
                $node.Name -eq $functionName
        },
        $true
    ) | Select-Object -First 1
    if ($null -eq $definition) {
        throw "installer is missing function $functionName"
    }
    Invoke-Expression $definition.Extent.Text
}

$pathTestVariable = "H0X_INSTALLER_PATH_TEST"
$oldPathTestVariable = [Environment]::GetEnvironmentVariable($pathTestVariable, "Process")
$ownedPathTestVariable = "H0X_INSTALLER_OWNED_PATH_TEST"
$oldOwnedPathTestVariable = [Environment]::GetEnvironmentVariable($ownedPathTestVariable, "Process")
$testRegistryPath = "Software\H0xInstallerTests-$([Guid]::NewGuid().ToString('N'))"
$testEnvironmentKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($testRegistryPath)
if ($null -eq $testEnvironmentKey) {
    throw "unable to create temporary installer test registry key"
}
try {
    [Environment]::SetEnvironmentVariable($pathTestVariable, "C:\expanded", "Process")
    [Environment]::SetEnvironmentVariable($ownedPathTestVariable, "C:\HyperOx", "Process")
    $testEnvironmentKey.SetValue(
        "Path",
        "C:\HyperOx\releases\deleted;%$ownedPathTestVariable%\bin;C:\HyperOx\bin;%$pathTestVariable%\bin;`"C:\Program Files\Tool`";C:\existing;C:\HyperOx\releases\old\nested;C:\HyperOx\releases-old\old;C:\HyperOx\releases\deleted",
        [Microsoft.Win32.RegistryValueKind]::ExpandString
    )
    $ownedEntries = @("C:\HyperOx\bin", "C:\HyperOx\current")
    $pathChanged = Update-PathRegistryEntry `
        -EnvironmentKey $testEnvironmentKey `
        -Entry "C:\HyperOx\releases\new" `
        -OwnedEntriesToRemove $ownedEntries `
        -OwnedEntryParentToRemove "C:\HyperOx\releases"
    if (-not $pathChanged) {
        throw "installer PATH update reported no change"
    }
    if (Update-PathRegistryEntry `
        -EnvironmentKey $testEnvironmentKey `
        -Entry "C:\HyperOx\releases\new" `
        -OwnedEntriesToRemove $ownedEntries `
        -OwnedEntryParentToRemove "C:\HyperOx\releases") {
        throw "installer PATH update was not idempotent"
    }

    $options = [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
    $rawPath = $testEnvironmentKey.GetValue("Path", $null, $options)
    $expectedPath = "C:\HyperOx\releases\new;%$pathTestVariable%\bin;`"C:\Program Files\Tool`";C:\existing;C:\HyperOx\releases\old\nested;C:\HyperOx\releases-old\old"
    if ($rawPath -cne $expectedPath) {
        throw "installer changed raw PATH: expected '$expectedPath', got '$rawPath'"
    }
    if ($testEnvironmentKey.GetValueKind("Path") -ne [Microsoft.Win32.RegistryValueKind]::ExpandString) {
        throw "installer changed the PATH registry value kind"
    }
} finally {
    $testEnvironmentKey.Dispose()
    [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($testRegistryPath, $false)
    [Environment]::SetEnvironmentVariable($pathTestVariable, $oldPathTestVariable, "Process")
    [Environment]::SetEnvironmentVariable($ownedPathTestVariable, $oldOwnedPathTestVariable, "Process")
}

$archive = (Resolve-Path -LiteralPath $ArchivePath).Path
$root = Join-Path $env:RUNNER_TEMP ("h0x-installer-test-" + [Guid]::NewGuid().ToString("N"))
$webRoot = Join-Path $root "web"
$h0xHome = Join-Path $root "home"
$installDir = Join-Path $root "bin"
New-Item -ItemType Directory -Force -Path $webRoot | Out-Null
Copy-Item -LiteralPath $archive -Destination (Join-Path $webRoot "h0x-windows-x86_64.zip")
Copy-Item -LiteralPath $installerPath -Destination (Join-Path $webRoot "install.ps1")
$hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
$listener.Stop()
function New-StableManifest {
    param([string]$Version)
    return @{
        version = $Version
        assets = @{
            "windows-x86_64" = "http://127.0.0.1:$port/h0x-windows-x86_64.zip"
        }
        sha256 = @{
            "windows-x86_64" = $hash
        }
    } | ConvertTo-Json -Depth 5
}
$stableManifest = New-StableManifest -Version "0.0.1"
$updatedManifest = New-StableManifest -Version "0.0.2"
$stableManifestPath = Join-Path $webRoot "latest.json"
$stableManifest | Out-File -LiteralPath $stableManifestPath -Encoding utf8

$server = $null
$oldH0xHome = $env:H0X_HOME
$oldInstallerUrl = $env:H0X_INSTALLER_URL
$oldProcessPath = $env:Path
$registryOptions = [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
$realUserEnvironmentKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey("Environment")
if ($null -eq $realUserEnvironmentKey) {
    throw "unable to open the current user's environment registry key"
}
$realUserPathExisted = $realUserEnvironmentKey.GetValueNames() -contains "Path"
$realUserPath = if ($realUserPathExisted) {
    $realUserEnvironmentKey.GetValue("Path", $null, $registryOptions)
} else {
    $null
}
$realUserPathKind = if ($realUserPathExisted) {
    $realUserEnvironmentKey.GetValueKind("Path")
} else {
    $null
}
$realUserEnvironmentKey.Dispose()
try {
    $server = Start-Process python -ArgumentList @("-m", "http.server", "$port", "--bind", "127.0.0.1", "--directory", $webRoot) -PassThru -WindowStyle Hidden
    $env:H0X_HOME = Join-Path $root "unused\..\home"
    $stableManifestUrl = "http://127.0.0.1:$port/latest.json"
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            Invoke-WebRequest -Uri $stableManifestUrl -UseBasicParsing | Out-Null
            break
        } catch {
            if ($attempt -eq 19) { throw }
            Start-Sleep -Milliseconds 100
        }
    }

    $freshStableHome = Join-Path $root "fresh-stable-home"
    $freshStableBin = Join-Path $root "fresh-stable-bin"
    $stableManifest | Out-File -LiteralPath $stableManifestPath -Encoding utf8
    $env:H0X_HOME = $freshStableHome
    $env:H0X_INSTALLER_URL = "http://127.0.0.1:$port/install.ps1"
    $env:Path = $oldProcessPath
    & $bootstrapPath `
        -ManifestUrl $stableManifestUrl `
        -InstallDir $freshStableBin
    if ($LASTEXITCODE -ne 0) {
        throw "CMD bootstrap failed with exit code $LASTEXITCODE"
    }
    $env:H0X_INSTALLER_URL = $oldInstallerUrl
    $freshStableRelease = Get-ChildItem -LiteralPath (Join-Path $freshStableHome "packages\standalone\releases") -Directory |
        Where-Object { $_.Name.StartsWith("0.0.1-") } |
        Select-Object -First 1
    if ($null -eq $freshStableRelease) {
        throw "fresh installer did not default to the stable Windows package"
    }

    $env:H0X_HOME = Join-Path $root "unused\..\home"
    $env:Path = $oldProcessPath
    & $installerPath `
        -ManifestUrl $stableManifestUrl `
        -InstallDir $installDir

    # Keep the existing positional web-installer contract, including Retain in slot four.
    & $installerPath "stable" $stableManifestUrl $installDir 3

    $localInstallDir = Join-Path $root "local-bin"
    $env:H0X_HOME = Join-Path $root "local-home"
    $partialLocalModeRejected = $false
    try {
        & $installerPath `
            -InstallDir $localInstallDir `
            -LocalPackagePath $archive
    } catch {
        if ($_.Exception.Message -notlike "Local package mode requires*") {
            throw
        }
        $partialLocalModeRejected = $true
    }
    if (-not $partialLocalModeRejected) {
        throw "installer accepted partial local-package inputs"
    }

    $badLocalChecksumRejected = $false
    try {
        & $installerPath `
            -ManifestUrl "$stableManifestUrl/unused" `
            -InstallDir $localInstallDir `
            -LocalPackagePath $archive `
            -LocalPackageFormat "zip" `
            -LocalPackageIdentity "0.0.0-local-package" `
            -LocalPackageSha256 ("0" * 64)
    } catch {
        if ($_.Exception.Message -notlike "Downloaded HyperOx checksum did not match.*") {
            throw
        }
        $badLocalChecksumRejected = $true
    }
    if (-not $badLocalChecksumRejected) {
        throw "installer accepted a local package with the wrong checksum"
    }

    & $installerPath `
        -ManifestUrl "$stableManifestUrl/unused" `
        -InstallDir $localInstallDir `
        -LocalPackagePath $archive `
        -LocalPackageFormat "zip" `
        -LocalPackageIdentity "0.0.0-local-package" `
        -LocalPackageSha256 $hash
    if (-not (Test-Path -LiteralPath (Join-Path $localInstallDir "h0x.exe") -PathType Leaf)) {
        throw "installer did not activate the verified local package"
    }
    $env:H0X_HOME = $h0xHome

    $required = @(
        "h0x.exe",
        "conpty\herdr-conpty.json",
        "conpty\conpty.dll",
        "conpty\x64\OpenConsole.exe",
        "conpty\arm64\OpenConsole.exe",
        "THIRD-PARTY-NOTICES\Microsoft.Windows.Console.ConPTY-LICENSE.txt",
        "THIRD-PARTY-NOTICES\Microsoft.Windows.Console.ConPTY-NOTICE.md"
    )
    foreach ($relative in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $installDir $relative) -PathType Leaf)) {
            throw "installer did not activate required file $relative"
        }
    }

    $releasesDir = Join-Path $h0xHome "packages\standalone\releases"
    $releaseDir = Get-ChildItem -LiteralPath $releasesDir -Directory |
        Where-Object { -not $_.Name.StartsWith(".staging.") } |
        Select-Object -First 1
    if ($null -eq $releaseDir) {
        throw "installer did not create a versioned release"
    }

    $pathWithoutH0x = @(
        $env:Path.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries) |
            Where-Object {
                -not (Test-Path -LiteralPath (Join-Path $_ "h0x.exe") -PathType Leaf) -and
                -not (Test-Path -LiteralPath (Join-Path $_ "h0x.cmd") -PathType Leaf)
            }
    ) -join ";"
    $env:Path = $pathWithoutH0x
    if ($null -ne (Get-Command h0x -ErrorAction SilentlyContinue)) {
        throw "test PATH still resolves HyperOx before current-junction migration discovery"
    }
    Remove-Item -LiteralPath (Join-Path $releaseDir.FullName "conpty\conpty.dll") -Force

    $badManifest = $stableManifest | ConvertFrom-Json
    $badManifest.assets."windows-x86_64" = "http://127.0.0.1:$port/missing.zip"
    $badManifest | ConvertTo-Json -Depth 5 | Out-File -LiteralPath $stableManifestPath -Encoding utf8
    $downloadFailed = $false
    try {
        & "$PSScriptRoot\..\distribution\install.ps1" `
            -ManifestUrl $stableManifestUrl `
            -InstallDir $installDir
    } catch {
        if ($_.Exception.Message -notmatch "(?i)(404|not found)") {
            throw
        }
        $downloadFailed = $true
    }
    if (-not $downloadFailed) {
        throw "installer repair unexpectedly accepted a missing archive"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $releaseDir.FullName "h0x.exe") -PathType Leaf)) {
        throw "failed repair removed the existing release"
    }

    $stableManifest | Out-File -LiteralPath $stableManifestPath -Encoding utf8
    $stagedConpty = Join-Path $releasesDir ".staging.$($releaseDir.Name).$PID\conpty\conpty.dll"

    $transientLockState = @{ Handle = $null; Acquired = $false; Released = $false }
    $transientLockTimer = New-Object System.Timers.Timer
    $transientLockTimer.Interval = 300
    $transientLockTimer.AutoReset = $false
    $transientLockSource = "H0xTransientInstallerLock-$PID"
    $transientLockRelease = Register-ObjectEvent `
        -InputObject $transientLockTimer `
        -EventName Elapsed `
        -SourceIdentifier $transientLockSource `
        -MessageData $transientLockState `
        -Action {
            $state = $event.MessageData
            if ($null -ne $state.Handle) {
                $state.Handle.Dispose()
                $state.Handle = $null
            }
            $state.Released = $true
        }
    $lockStagedFileTransiently = {
        if (-not $transientLockState.Acquired) {
            $transientLockState.Handle = [System.IO.File]::Open(
                $stagedConpty,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::Read
            )
            $transientLockState.Acquired = $true
            $transientLockTimer.Start()
        }
    }.GetNewClosure()
    $transientLockBreakpoint = Set-PSBreakpoint -Script $installerPath -Variable "backupDir" -Mode Write -Action $lockStagedFileTransiently
    try {
        & $installerPath `
            -ManifestUrl $stableManifestUrl `
            -InstallDir $installDir
        if (-not $transientLockState.Acquired) {
            throw "installer did not acquire the transient staged-file lock"
        }
        if (-not $transientLockState.Released) {
            throw "installer activated the release before the transient lock was released"
        }
        if (-not (Test-Path -LiteralPath (Join-Path $releaseDir.FullName "conpty\conpty.dll") -PathType Leaf)) {
            throw "installer did not repair the release after the transient lock cleared"
        }
    } finally {
        Remove-PSBreakpoint -Breakpoint $transientLockBreakpoint
        $transientLockTimer.Stop()
        Unregister-Event -SourceIdentifier $transientLockSource -ErrorAction SilentlyContinue
        Remove-Job -Id $transientLockRelease.Id -Force -ErrorAction SilentlyContinue
        if ($null -ne $transientLockState.Handle) {
            $transientLockState.Handle.Dispose()
        }
        $transientLockTimer.Dispose()
    }

    Remove-Item -LiteralPath (Join-Path $releaseDir.FullName "conpty\conpty.dll") -Force
    $lockState = @{ Handle = $null }
    $lockStagedFile = {
        if ($null -eq $lockState.Handle) {
            $lockState.Handle = [System.IO.File]::Open(
                $stagedConpty,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::Read
            )
        }
    }.GetNewClosure()
    $swapBreakpoint = Set-PSBreakpoint -Script $installerPath -Variable "backupDir" -Mode Write -Action $lockStagedFile
    try {
        $swapFailed = $false
        try {
            & $installerPath `
                -ManifestUrl $stableManifestUrl `
                -InstallDir $installDir
        } catch {
            $swapFailed = $true
        }
        if ($null -eq $lockState.Handle) {
            throw "installer did not acquire the staged file handle before the swap"
        }
        if (-not $swapFailed) {
            throw "installer unexpectedly activated a release with a locked staged file"
        }
        if (-not (Test-Path -LiteralPath (Join-Path $releaseDir.FullName "h0x.exe") -PathType Leaf)) {
            throw "failed activation did not restore the prior release"
        }
        if (@(Get-ChildItem -LiteralPath $releasesDir -Force -Directory -Filter ".backup.$($releaseDir.Name).*").Count -ne 0) {
            throw "failed activation stranded a release backup"
        }
        foreach ($junction in @($installDir, (Join-Path $h0xHome "packages\standalone\current"))) {
            if (-not (Test-Path -LiteralPath (Join-Path $junction "h0x.exe") -PathType Leaf)) {
                throw "failed activation left an invalid installer junction at $junction"
            }
        }
    } finally {
        Remove-PSBreakpoint -Breakpoint $swapBreakpoint
        if ($null -ne $lockState.Handle) {
            $lockState.Handle.Dispose()
        }
    }

    & "$PSScriptRoot\..\distribution\install.ps1" `
        -ManifestUrl $stableManifestUrl `
        -InstallDir $installDir
    if (-not (Test-Path -LiteralPath (Join-Path $installDir "conpty\conpty.dll") -PathType Leaf)) {
        throw "installer did not repair an incomplete release"
    }

    $x64HostDir = Join-Path $releaseDir.FullName "conpty\x64"
    $junctionTarget = Join-Path $root "junction-target"
    Move-Item -LiteralPath $x64HostDir -Destination $junctionTarget
    New-Item -ItemType Junction -Path $x64HostDir -Target $junctionTarget | Out-Null
    & "$PSScriptRoot\..\distribution\install.ps1" `
        -ManifestUrl $stableManifestUrl `
        -InstallDir $installDir
    $repairedHostDir = Get-Item -LiteralPath (Join-Path $installDir "conpty\x64") -Force
    if ($repairedHostDir.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "installer accepted a reparse-point ConPTY directory"
    }

    $currentDir = Join-Path $h0xHome "packages\standalone\current"
    $unrelatedPathOne = Join-Path $env:SystemRoot "System32"
    $unrelatedPathTwo = Join-Path $root "unrelated-two"
    New-Item -ItemType Directory -Force -Path $unrelatedPathTwo | Out-Null
    $staleReleasePath = Join-Path $releasesDir "0.0.0-deleted-x86_64-pc-windows-msvc"
    $nestedReleasePath = Join-Path $releaseDir.FullName "nested"
    $similarReleasePath = "$releasesDir-old\old"
    $pathBeforeStable = "$installDir;$staleReleasePath;$($releaseDir.FullName);$currentDir;$unrelatedPathOne;$nestedReleasePath;$similarReleasePath;$installDir;$unrelatedPathTwo"
    $realUserEnvironmentKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey("Environment")
    try {
        $realUserEnvironmentKey.SetValue("Path", $pathBeforeStable, [Microsoft.Win32.RegistryValueKind]::ExpandString)
    } finally {
        $realUserEnvironmentKey.Dispose()
    }
    $env:Path = $pathBeforeStable

    $updatedManifest | Out-File -LiteralPath $stableManifestPath -Encoding utf8
    & "$PSScriptRoot\..\distribution\install.ps1" `
        -Channel stable `
        -ManifestUrl $stableManifestUrl `
        -InstallDir $installDir `
        -Retain 1
    $stableReleaseDir = Get-ChildItem -LiteralPath (Join-Path $h0xHome "packages\standalone\releases") -Directory |
        Where-Object { $_.Name.StartsWith("0.0.2-") } |
        Select-Object -First 1
    if ($null -eq $stableReleaseDir) {
        throw "installer did not install the updated stable Windows package"
    }
    if (Test-Path -LiteralPath $releaseDir.FullName) {
        throw "installer did not prune the old concrete release"
    }
    $expectedPath = "$($stableReleaseDir.FullName);$unrelatedPathOne;$nestedReleasePath;$similarReleasePath;$unrelatedPathTwo"
    $realUserEnvironmentKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment")
    try {
        $actualPath = $realUserEnvironmentKey.GetValue("Path", $null, $registryOptions)
        if ($actualPath -cne $expectedPath -or $env:Path -cne $expectedPath) {
            throw "installer did not replace managed PATH entries while preserving unrelated entries"
        }
        if ($realUserEnvironmentKey.GetValueKind("Path") -ne [Microsoft.Win32.RegistryValueKind]::ExpandString) {
            throw "installer changed the real PATH registry value kind"
        }
    } finally {
        $realUserEnvironmentKey.Dispose()
    }
    $pathDirectory = Get-Item -LiteralPath ($env:Path.Split(";")[0]) -Force
    if (($pathDirectory.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        -not $pathDirectory.FullName.Equals($stableReleaseDir.FullName, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "installer PATH does not start with the active concrete release directory"
    }
    $resolvedH0x = Get-Command h0x -CommandType Application -ErrorAction Stop
    if (-not $resolvedH0x.Source.Equals(
        (Join-Path $stableReleaseDir.FullName "h0x.exe"),
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw "installed command does not resolve through the active concrete release"
    }
    & h0x --version *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "installed command failed from the concrete release PATH entry"
    }
    foreach ($junction in @($installDir, $currentDir)) {
        $junctionItem = Get-Item -LiteralPath $junction -Force
        if (-not ($junctionItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            -not ([string]$junctionItem.Target).Equals($stableReleaseDir.FullName, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "installer did not preserve compatibility junction $junction"
        }
    }
    foreach ($relative in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $installDir $relative) -PathType Leaf)) {
            throw "stable installer did not activate required file $relative"
        }
    }
} finally {
    $env:H0X_HOME = $oldH0xHome
    $env:H0X_INSTALLER_URL = $oldInstallerUrl
    $env:Path = $oldProcessPath
    if ($null -ne $server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    }
    $realUserEnvironmentKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey("Environment")
    try {
        if ($realUserPathExisted) {
            $realUserEnvironmentKey.SetValue("Path", $realUserPath, $realUserPathKind)
        } else {
            $realUserEnvironmentKey.DeleteValue("Path", $false)
        }
    } finally {
        $realUserEnvironmentKey.Dispose()
    }
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
