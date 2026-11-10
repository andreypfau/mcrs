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

function Restore-PowerPlan {
    $previous = ([IO.File]::ReadAllText($PowerRestoreFile)).Trim()
    Get-NativeOutput 'powercfg' @('/setactive', $previous) | Out-Null
    Remove-Item $PowerRestoreFile
    Write-Host "Power plan restored to $previous"
}

$repo = Get-KitRepo
if (-not $repo) { Write-Host 'run.ps1 must stay inside the mcrs checkout; run bootstrap.ps1 first.'; exit 1 }

if ((Test-Path $PowerRestoreFile) -and (Confirm-Choice 'An earlier run stopped before it switched the power plan back. Restore the previous plan now? Otherwise this run restores it when it ends.')) {
    Restore-PowerPlan
}

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

$name = "$Label-windows-$((Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ', [Globalization.CultureInfo]::InvariantCulture))"
$results = Join-Path $KitDir 'results'
$dir = Join-Path $results $name
New-Item -ItemType Directory -Force -Path $dir | Out-Null

$status = 1
try {
    [void][PerfKitAwake]::SetThreadExecutionState($EsContinuous -bor $EsSystemRequired -bor $EsDisplayRequired)

    $systemFile = Join-Path $dir 'system.json'
    & (Join-Path $KitDir 'sysinfo.ps1') -Label $Label -Out $systemFile
    if (-not (Test-Path $systemFile)) { Write-Host 'Recording the system failed; see the error above.'; exit 1 }
    $hygieneFile = Join-Path $dir 'hygiene.json'
    & (Join-Path $KitDir 'prepare.ps1') -Out $hygieneFile
    if (-not (Test-Path $hygieneFile)) { Write-Host 'Preparing the machine failed; see the error above.'; exit 1 }
    & (Join-Path $KitDir 'bootstrap.ps1') -BuildOnly
    if ($LASTEXITCODE -ne 0) { Write-Host "The build failed; run $KitDir\bootstrap.ps1 and try again."; exit 1 }

    $java = Find-Java
    if (-not $java) { Write-Host "Java $JavaMin not found; run $KitDir\bootstrap.ps1 first."; exit 1 }

    # The client sizes a window in physical pixels (scale factor 1), so it is compared with the
    # primary display's pixels, minus the taskbar and the title bar.
    $system = Get-Content -Raw -Encoding UTF8 -Path $systemFile | ConvertFrom-Json
    $window = '960x540'
    if ($system.primary_display) {
        $pixels = $system.primary_display.pixels
        foreach ($candidate in @('1920x1080', '1600x900', '1280x720', '960x540')) {
            $size = $candidate.Split('x')
            if ([int]$size[0] -le $pixels.width - 40 -and [int]$size[1] -le $pixels.height - 160) { $window = $candidate; break }
        }
    }
    $heap = [math]::Max(1, [math]::Min(16, [math]::Floor($system.memory_bytes / 1GB / 2)))
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
    $profilePattern = [regex]::Escape($userProfile) + '|' + [regex]::Escape($userProfile.Replace('\', '\\'))
    Get-ChildItem -Path $dir -Recurse -File | ForEach-Object {
        $text = [IO.File]::ReadAllText($_.FullName)
        [IO.File]::WriteAllText($_.FullName, ($text -replace $profilePattern, '~'), (New-Object Text.UTF8Encoding $false))
    }

    & (Join-Path $KitDir 'prepare.ps1') -CheckOnly -Out (Join-Path $dir 'hygiene-after.json')

    $zip = Join-Path $results "$name.zip"
    try {
        if (Test-Path $zip) { Remove-Item $zip }
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        [IO.Compression.ZipFile]::CreateFromDirectory($dir, $zip, [IO.Compression.CompressionLevel]::Optimal, $true)
    } catch {
        Write-Host "Could not create the zip; send the folder $dir instead."
        exit 1
    }
    Write-Host ''
    if ($status -ne 0) { Write-Host "Some sections failed (exit $status); the zip still holds everything that was measured." }
    Write-Host "Done. Send this file: $zip"
    Write-Host 'It holds system.json, hygiene.json, hygiene-after.json, report.json, raw\ and run.log.'
} finally {
    [void][PerfKitAwake]::SetThreadExecutionState($EsContinuous)
    if (Test-Path $PowerRestoreFile) { Restore-PowerPlan }
}
exit $status
