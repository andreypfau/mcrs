# The one entry point: records the machine, prepares it, builds, runs the benchmarks and zips the results.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File run.ps1 [-Full] [-Label NAME]
param([switch]$Full, [string]$Label = '')

. (Join-Path $PSScriptRoot 'common.ps1')

if (-not ('PerfKitAwake' -as [type])) {
    Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public static class PerfKitAwake {
    [DllImport("kernel32.dll")]
    public static extern uint SetThreadExecutionState(uint flags);
}
'@
}
$EsContinuous = [uint32]2147483648
$EsSystemRequired = [uint32]1
$EsDisplayRequired = [uint32]2

$benchProfile = 'quick'
if ($Full) { $benchProfile = 'full' }

$repo = Get-KitRepo
if (-not $repo) { Write-Host 'run.ps1 must stay inside the mcrs checkout; run bootstrap.ps1 first.'; exit 1 }

Write-Host "mcrs benchmark kit, profile: $benchProfile"
if ($Full) { Write-Host 'Expected time: about 3-5 hours, depending on the machine.' } else { Write-Host 'Expected time: about 45-90 minutes, depending on the machine.' }
Write-Host 'While it runs, do not touch the computer. The game window must stay visible, on top and not minimised.'
Write-Host ''

$Label = ($Label -replace '[^A-Za-z0-9_-]+', '-').Trim('-')
while (-not $Label) {
    $Label = ((Read-Host 'Machine label (for example ryzen-3060 or xps-15)') -replace '[^A-Za-z0-9_-]+', '-').Trim('-')
}

Write-Host 'The mob benchmark runs the Minecraft server, which needs the Minecraft EULA: https://aka.ms/MinecraftEULA'
if (Confirm-Choice 'Do you accept the Minecraft EULA?') {
    $sections = @('--accept-eula')
} else {
    Write-Host 'Running without the mob section.'
    $sections = @('--only', 'client,server,worldgen')
}

$name = "$Label-windows-$((Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ'))"
$results = Join-Path $KitDir 'results'
$dir = Join-Path $results $name
New-Item -ItemType Directory -Force -Path $dir | Out-Null

$status = 1
try {
    [void][PerfKitAwake]::SetThreadExecutionState($EsContinuous -bor $EsSystemRequired -bor $EsDisplayRequired)

    & (Join-Path $KitDir 'sysinfo.ps1') -Label $Label -Out (Join-Path $dir 'system.json')
    & (Join-Path $KitDir 'prepare.ps1') -Out (Join-Path $dir 'hygiene.json')
    & (Join-Path $KitDir 'bootstrap.ps1') -BuildOnly
    if ($LASTEXITCODE -ne 0) { Write-Host "The build failed; run $KitDir\bootstrap.ps1 and try again."; exit 1 }

    $java = Find-Java
    if (-not $java) { Write-Host "Java $JavaMin not found; run $KitDir\bootstrap.ps1 first."; exit 1 }

    # Assumption: the client takes -window in logical pixels and applies the display scale itself,
    # so the size is compared with the logical size of the primary display, minus taskbar and title bar.
    $system = Get-Content -Raw -Path (Join-Path $dir 'system.json') | ConvertFrom-Json
    $window = '960x540'
    if ($system.primary_display) {
        $logical = $system.primary_display.logical
        foreach ($candidate in @('1920x1080', '1600x900', '1280x720', '960x540')) {
            $size = $candidate.Split('x')
            if ([int]$size[0] -le $logical.width - 40 -and [int]$size[1] -le $logical.height - 100) { $window = $candidate; break }
        }
    }
    $heap = [math]::Min(16, [math]::Floor($system.memory_bytes / 1GB / 2))
    Write-Host "Window $window, Java heap $heap GB, results in $dir"

    $log = New-Object IO.StreamWriter((Join-Path $dir 'run.log'), $false, (New-Object Text.UTF8Encoding $false))
    $userProfile = $env:USERPROFILE
    try {
        $old = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        & (Join-Path $repo 'target\release\perf-kit.exe') run --out $dir --profile $benchProfile --repo $repo --window $window --heap "${heap}g" --java $java @sections 2>&1 |
            ForEach-Object {
                $line = "$_"
                Write-Host $line
                $log.WriteLine($line.Replace($userProfile, '~'))
            }
        $status = $LASTEXITCODE
        $ErrorActionPreference = $old
    } finally {
        $log.Close()
    }

    & (Join-Path $KitDir 'prepare.ps1') -CheckOnly -Out (Join-Path $dir 'hygiene-after.json')

    $zip = Join-Path $results "$name.zip"
    Compress-Archive -Path $dir -DestinationPath $zip -Force
    Write-Host ''
    if ($status -ne 0) { Write-Host "Some sections failed (exit $status); the zip still holds everything that was measured." }
    Write-Host "Done. Send this file: $zip"
    Write-Host 'It holds system.json, hygiene.json, hygiene-after.json, report.json, raw\ and run.log.'
} finally {
    [void][PerfKitAwake]::SetThreadExecutionState($EsContinuous)
    if (Test-Path $PowerRestoreFile) {
        $previous = ([IO.File]::ReadAllText($PowerRestoreFile)).Trim()
        Get-NativeOutput 'powercfg' @('/setactive', $previous) | Out-Null
        Remove-Item $PowerRestoreFile
        Write-Host "Power plan restored to $previous"
    }
}
exit $status
