# Sets up this PC for the benchmarks: winget, Git, Visual Studio Build Tools, rustup, the pinned
# toolchain, Java 25 and the release build. Safe to run again; every step checks before it installs.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File bootstrap.ps1 [-Check | -BuildOnly]
#   -Check      report what is missing, install nothing, build nothing
#   -BuildOnly  only run the release build (used by run.ps1)
param([switch]$Check, [switch]$BuildOnly)

$RepoUrl = 'https://github.com/andreypfau/mcrs'
$CloneDir = 'C:\mcrs'
$MinDiskGb = 40
$ProxyHint = 'check the internet connection; behind a proxy run: $env:HTTPS_PROXY = "http://host:port", then rerun'
$WingetFlags = @('-e', '--source', 'winget', '--accept-package-agreements', '--accept-source-agreements')

[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$ProgressPreference = 'SilentlyContinue'
$NativeArch = $env:PROCESSOR_ARCHITECTURE
if ($env:PROCESSOR_ARCHITEW6432) { $NativeArch = $env:PROCESSOR_ARCHITEW6432 }

# Started outside a checkout (the script was downloaded alone): get Git, clone, continue there.
if (-not (Test-Path (Join-Path $PSScriptRoot 'common.ps1'))) {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($Check) { Write-Host "Not inside an mcrs checkout; run without -Check to clone into $CloneDir."; exit 1 }
    $free = (Get-PSDrive -Name C -PSProvider FileSystem).Free / 1GB
    if ($free -lt $MinDiskGb) { Write-Host ('Only {0:N0} GB free on C:, need {1} GB. Free space and rerun.' -f $free, $MinDiskGb); exit 1 }
    if (-not (Test-Path (Join-Path $CloneDir 'Cargo.toml'))) {
        if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
            if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
                Write-Host 'winget is missing. Install App Installer from the Microsoft Store (ms-windows-store://pdp/?ProductId=9NBLGGH4NNS1), then rerun.'
                exit 1
            }
            winget install --id Git.Git @WingetFlags
            $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
        }
        git config --global core.longpaths true
        git clone $RepoUrl $CloneDir
        if ($LASTEXITCODE -ne 0) { Write-Host "git clone failed; $ProxyHint"; exit 1 }
    }
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $CloneDir 'tools\perf-kit\bootstrap.ps1')
    exit $LASTEXITCODE
}

. (Join-Path $PSScriptRoot 'common.ps1')

$steps = New-Object System.Collections.ArrayList
function Add-Step([string]$Name, [string]$Status, [string]$Hint = '') {
    [void]$steps.Add([pscustomobject][ordered]@{ Step = $Name; Status = $Status; Hint = $Hint })
    $suffix = ''
    if ($Hint) { $suffix = " - $Hint" }
    Write-Host "[$Status] $Name$suffix"
}

function Show-Summary {
    $steps | Format-Table -AutoSize -Wrap | Out-Host
    $bad = @($steps | Where-Object { $_.Status -eq 'FAILED' -or $_.Status -eq 'MISSING' })
    if ($bad.Count -gt 0) { exit 1 }
    exit 0
}

# Runs one command in an elevated PowerShell after the UAC prompt; returns its exit code, -1 if declined.
function Invoke-Elevated([string]$Command) {
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($Command))
    try {
        $process = Start-Process powershell -Verb RunAs -Wait -PassThru -ArgumentList @('-NoProfile', '-EncodedCommand', $encoded)
        return $process.ExitCode
    } catch {
        return -1
    }
}

function Test-Url([string]$Url) {
    try {
        Invoke-WebRequest -Uri $Url -Method Head -UseBasicParsing -TimeoutSec 15 | Out-Null
        return $true
    } catch [System.Net.WebException] {
        return ($null -ne $_.Exception.Response)
    } catch {
        return $false
    }
}

function Test-VcTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $false }
    $component = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
    if ($NativeArch -eq 'ARM64') { $component = 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' }
    return [bool](Get-NativeOutput $vswhere @('-latest', '-products', '*', '-requires', $component, '-property', 'installationPath'))
}

function Test-Toolchain([string]$Channel) {
    $list = Get-NativeOutput 'rustup' @('toolchain', 'list')
    return ($list -match ('(?m)^' + [regex]::Escape($Channel)))
}

function Invoke-Build([string]$Repo) {
    Write-Host "Building the release binaries in $Repo (the first build takes 10-40 minutes)"
    $log = Join-Path $env:TEMP 'mcrs-perf-build.log'
    Push-Location $Repo
    $code = 1
    for ($attempt = 1; $attempt -le 2; $attempt++) {
        & cmd /c 'cargo build --release -p mcrs -p mcrs_minecraft_client -p mcrs_mob_bench -p mcrs_perf_kit 2>&1' | Tee-Object -FilePath $log | Out-Host
        $code = $LASTEXITCODE
        if ($code -eq 0) { break }
        $denied = Select-String -Path $log -Pattern 'os error 5', 'Access is denied' -SimpleMatch -Quiet
        if ($attempt -eq 1 -and $denied) {
            Write-Host 'The build hit "Access is denied" (os error 5), usually an antivirus scan holding a file. Retrying once in 15 s.'
            Start-Sleep -Seconds 15
            continue
        }
        break
    }
    Pop-Location
    if ($code -eq 0) {
        Add-Step 'build' 'OK'
    } elseif ($denied) {
        Add-Step 'build' 'FAILED' 'os error 5 twice: add the Defender exclusion (rerun bootstrap.ps1 and answer y) and retry'
    } else {
        Add-Step 'build' 'FAILED' 'see the cargo output above; rerun bootstrap.ps1 to check the toolchain'
    }
}

if ($BuildOnly) {
    $repo = Get-KitRepo
    if (-not $repo) { Add-Step 'build' 'FAILED' 'run bootstrap.ps1 from inside the mcrs checkout'; Show-Summary }
    Invoke-Build $repo
    Show-Summary
}

# network
$offline = @()
foreach ($hostName in @('static.rust-lang.org', 'piston-data.mojang.com', 'api.adoptium.net')) {
    if (-not (Test-Url "https://$hostName")) { $offline += $hostName }
}
if ($offline.Count -eq 0) { Add-Step 'network' 'OK' } else { Add-Step 'network' 'FAILED' "cannot reach $($offline -join ', '); $ProxyHint" }

# repository and disk
$repo = Get-KitRepo
if (-not $repo) {
    Add-Step 'repo' 'FAILED' "the kit must stay in tools\perf-kit of an mcrs checkout; delete this copy and run bootstrap.ps1 alone to clone into $CloneDir"
    Show-Summary
} elseif ($repo -match '[^\x20-\x7E]') {
    Add-Step 'repo' 'WARN' "$repo has non-ASCII characters, which can break the build; clone into $CloneDir instead"
} else {
    Add-Step 'repo' 'OK' $repo
}
$free = Get-FreeGB $repo
if ($free -ge $MinDiskGb) {
    Add-Step 'disk' 'OK' "$free GB free"
} else {
    Add-Step 'disk' 'FAILED' "$free GB free on the drive of $repo, need $MinDiskGb GB; free space and rerun"
    Show-Summary
}

# winget
$hasWinget = [bool](Get-Command winget -ErrorAction SilentlyContinue)
if ($hasWinget) {
    Add-Step 'winget' 'OK'
} else {
    Add-Step 'winget' 'FAILED' 'install App Installer from the Microsoft Store (ms-windows-store://pdp/?ProductId=9NBLGGH4NNS1), then rerun'
}

# Git
if (Get-Command git -ErrorAction SilentlyContinue) {
    Add-Step 'git' 'OK'
} elseif ($Check) {
    Add-Step 'git' 'MISSING' 'run bootstrap.ps1 without -Check'
} elseif ($hasWinget) {
    winget install --id Git.Git @WingetFlags
    Update-SessionPath
    if (Get-Command git -ErrorAction SilentlyContinue) { Add-Step 'git' 'FIXED' } else { Add-Step 'git' 'FAILED' 'run: winget install --id Git.Git' }
} else {
    Add-Step 'git' 'FAILED' 'install winget first'
}
if ((-not $Check) -and (Get-Command git -ErrorAction SilentlyContinue)) {
    git config --global core.longpaths true
}

# Visual Studio Build Tools (link.exe and the Windows SDK)
if (Test-VcTools) {
    Add-Step 'build tools' 'OK'
} elseif ($Check) {
    Add-Step 'build tools' 'MISSING' 'Visual Studio Build Tools with C++; run bootstrap.ps1 without -Check'
} elseif (-not $hasWinget) {
    Add-Step 'build tools' 'FAILED' 'install winget first'
} else {
    Write-Host 'Rust needs the Visual Studio C++ Build Tools (link.exe), about 5 GB. Installing them needs administrator rights; Windows shows a UAC prompt.'
    if (Confirm-Choice 'Install Visual Studio Build Tools now?') {
        $command = "winget install --id Microsoft.VisualStudio.2022.BuildTools $($WingetFlags -join ' ') --override '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'; exit `$LASTEXITCODE"
        [void](Invoke-Elevated $command)
    }
    if (Test-VcTools) { Add-Step 'build tools' 'FIXED' } else { Add-Step 'build tools' 'FAILED' 'without them the build stops with "link.exe not found"; rerun and answer y' }
}

# rustup, under C:\rust when the profile path has spaces or non-ASCII characters
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Add-Step 'rustup' 'OK'
} elseif ($Check) {
    Add-Step 'rustup' 'MISSING' 'run bootstrap.ps1 without -Check'
} else {
    if ((Test-NeedsShortPaths) -and -not $env:CARGO_HOME) {
        [Environment]::SetEnvironmentVariable('RUSTUP_HOME', 'C:\rust\rustup', 'User')
        [Environment]::SetEnvironmentVariable('CARGO_HOME', 'C:\rust\cargo', 'User')
        Update-SessionPath
        Add-Step 'short paths' 'FIXED' 'RUSTUP_HOME=C:\rust\rustup, CARGO_HOME=C:\rust\cargo'
    }
    $rustArch = 'x86_64'
    if ($NativeArch -eq 'ARM64') { $rustArch = 'aarch64' }
    $init = Join-Path $env:TEMP 'rustup-init.exe'
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "https://static.rust-lang.org/rustup/dist/$rustArch-pc-windows-msvc/rustup-init.exe" -OutFile $init
        & $init -y --default-toolchain none --profile minimal
    } catch {
        Write-Host "rustup download failed: $($_.Exception.Message)"
    }
    Update-SessionPath
    if (Get-Command rustup -ErrorAction SilentlyContinue) { Add-Step 'rustup' 'FIXED' } else { Add-Step 'rustup' 'FAILED' "rustup download failed; $ProxyHint" }
}

# the toolchain pinned by rust-toolchain.toml
$channel = (Select-String -Path (Join-Path $repo 'rust-toolchain.toml') -Pattern '^channel\s*=\s*"(.*)"').Matches[0].Groups[1].Value
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Add-Step 'toolchain' 'FAILED' 'install rustup first'
} elseif (Test-Toolchain $channel) {
    Add-Step 'toolchain' 'OK' $channel
} elseif ($Check) {
    Add-Step 'toolchain' 'MISSING' "$channel; run bootstrap.ps1 without -Check (about 1.5 GB)"
} else {
    Write-Host "Installing $channel with clippy, rustfmt and the wasm targets (about 1.5 GB)"
    Push-Location $repo
    rustup toolchain install
    if ($LASTEXITCODE -ne 0) { rustup show }
    Pop-Location
    if (Test-Toolchain $channel) { Add-Step 'toolchain' 'FIXED' $channel } else { Add-Step 'toolchain' 'FAILED' $ProxyHint }
}

# Java 25, from the system or a kit-local Temurin
$java = Find-Java
if ($java) {
    Add-Step 'java' 'OK' $java
} elseif ($Check) {
    Add-Step 'java' 'MISSING' "Java $JavaMin or newer; run bootstrap.ps1 without -Check"
} else {
    $javaArch = 'x64'
    if ($NativeArch -eq 'ARM64') { $javaArch = 'aarch64' }
    $url = "https://api.adoptium.net/v3/binary/latest/$JavaMin/ga/windows/$javaArch/jdk/hotspot/normal/eclipse"
    $zip = Join-Path $KitHome 'jdk.zip'
    Write-Host "Downloading Temurin $JavaMin into $KitHome\jdk"
    New-Item -ItemType Directory -Force -Path (Join-Path $KitHome 'jdk') | Out-Null
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip
        Expand-Archive -Path $zip -DestinationPath (Join-Path $KitHome 'jdk') -Force
        Remove-Item $zip
    } catch {
        Write-Host "Java download failed: $($_.Exception.Message)"
    }
    $java = Find-Java
    if ($java) { Add-Step 'java' 'FIXED' $java } else { Add-Step 'java' 'FAILED' "download from $url failed; $ProxyHint" }
}

# optional Defender exclusion: real-time scans of fresh build outputs cause "os error 5"
$marker = Join-Path $KitHome 'defender-exclusion.txt'
if (Test-Path $marker) {
    Add-Step 'defender' 'OK' "exclusion for $repo"
} elseif ($Check) {
    Add-Step 'defender' 'SKIPPED' 'optional exclusion not set'
} else {
    Write-Host "Microsoft Defender scans every file the build writes. That slows the build and can fail it with ""os error 5"" (Access is denied)."
    Write-Host "An exclusion for $repo stops those scans for this folder only. It needs administrator rights."
    if ((Confirm-Choice "Add a Defender exclusion for $repo?") -and (Invoke-Elevated "Add-MpPreference -ExclusionPath '$($repo.Replace("'", "''"))'") -eq 0) {
        New-Item -ItemType Directory -Force -Path $KitHome | Out-Null
        [IO.File]::WriteAllText($marker, $repo)
        Add-Step 'defender' 'FIXED' "exclusion for $repo; remove it later with: Remove-MpPreference -ExclusionPath '$repo'"
    } else {
        Add-Step 'defender' 'SKIPPED' 'no exclusion; the build retries once on os error 5'
    }
}

# release build
if ($Check) {
    if (Test-Path (Join-Path $repo 'target\release\perf-kit.exe')) { Add-Step 'build' 'OK' 'binaries present' } else { Add-Step 'build' 'MISSING' 'run bootstrap.ps1 without -Check' }
} elseif (Get-Command cargo -ErrorAction SilentlyContinue) {
    Invoke-Build $repo
} else {
    Add-Step 'build' 'FAILED' 'install rustup and the toolchain first'
}

Show-Summary
