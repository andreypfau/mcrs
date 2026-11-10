# Shared helpers for the perf-kit PowerShell scripts. Dot-sourced, not run. Windows PowerShell 5.1 compatible.

$KitDir = $PSScriptRoot
$JavaMin = 25

function Test-NeedsShortPaths {
    return ($env:USERPROFILE -match '[^\x21-\x7E]')
}

if ($env:MCRS_PERF_HOME) {
    $KitHome = $env:MCRS_PERF_HOME
} elseif (Test-NeedsShortPaths) {
    $KitHome = 'C:\mcrs-perf'
} else {
    $KitHome = Join-Path $env:USERPROFILE '.mcrs-perf'
}
$PowerRestoreFile = Join-Path $KitHome 'power-scheme-restore.txt'

function Update-SessionPath {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
    $cargoHome = [Environment]::GetEnvironmentVariable('CARGO_HOME', 'User')
    $rustupHome = [Environment]::GetEnvironmentVariable('RUSTUP_HOME', 'User')
    if ((-not $cargoHome) -and (Test-NeedsShortPaths) -and (Test-Path 'C:\rust\cargo')) {
        # bootstrap.ps1 put Rust here without saving the variables in the user environment.
        $cargoHome = 'C:\rust\cargo'
        $rustupHome = 'C:\rust\rustup'
    }
    if ($cargoHome) {
        $env:CARGO_HOME = $cargoHome
        $env:RUSTUP_HOME = $rustupHome
        $env:Path = (Join-Path $cargoHome 'bin') + ';' + $env:Path
    } else {
        $env:Path = (Join-Path $env:USERPROFILE '.cargo\bin') + ';' + $env:Path
    }
}

function Confirm-Choice([string]$Question) {
    $reply = Read-Host "$Question [y/N]"
    return ($reply -match '^(y|yes)$')
}

# The checkout that contains this kit, or $null when the kit was copied out of one.
function Get-KitRepo {
    $root = [IO.Path]::GetFullPath((Join-Path $KitDir '..\..'))
    if ((Test-Path (Join-Path $root 'Cargo.toml')) -and (Test-Path (Join-Path $root 'rust-toolchain.toml'))) {
        return $root
    }
    return $null
}

# Runs a native command and returns its stdout and stderr as one string. PS 5.1 wraps stderr lines
# in error records, so they are turned back into text here.
function Get-NativeOutput([string]$Exe, [string[]]$Arguments) {
    $old = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $lines = & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
        return ($lines -join "`n").Trim()
    } catch {
        return $null
    } finally {
        $ErrorActionPreference = $old
    }
}

function Get-JavaMajor([string]$Java) {
    $text = Get-NativeOutput $Java @('-version')
    if ($text -match 'version "(\d+)') { return [int]$Matches[1] }
    return $null
}

# The path of a java.exe >= $JavaMin: $env:JAVA, the kit JDK, JAVA_HOME, PATH.
function Find-Java {
    $candidates = @()
    if ($env:JAVA) { $candidates += $env:JAVA }
    $candidates += @(Get-ChildItem -Path (Join-Path $KitHome 'jdk\*\bin\java.exe') -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
    if ($env:JAVA_HOME) { $candidates += (Join-Path $env:JAVA_HOME 'bin\java.exe') }
    $candidates += @(Get-Command java -CommandType Application -ErrorAction SilentlyContinue | ForEach-Object { $_.Source })
    foreach ($candidate in $candidates) {
        if (-not (Test-Path $candidate)) { continue }
        $major = Get-JavaMajor $candidate
        if ($major -and $major -ge $JavaMin) { return $candidate }
    }
    return $null
}

function Get-FreeGB([string]$Path) {
    $root = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($Path))
    $drive = Get-PSDrive -Name $root.Substring(0, 1) -PSProvider FileSystem -ErrorAction SilentlyContinue
    if ($drive) { return [int][math]::Floor($drive.Free / 1GB) }
    return $null
}

# Writes UTF-8 without BOM; PS 5.1 Out-File would write UTF-16. The profile path is replaced by ~.
function Write-JsonFile($Object, [string]$Path) {
    $json = $Object | ConvertTo-Json -Depth 6
    $json = $json.Replace($env:USERPROFILE.Replace('\', '\\'), '~')
    [IO.File]::WriteAllText($Path, $json, (New-Object Text.UTF8Encoding $false))
}

Update-SessionPath
