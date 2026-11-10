# The one entry point: records the machine, prepares it, builds, runs the benchmarks and zips the results.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File run.ps1 [-Full] [-Label NAME] [-Lang en|ru] [-Only mob,client,server,worldgen] [-Yes] [-AcceptEula]
param([switch]$Full, [string]$Label = '', [ValidateSet('en', 'ru')][string]$Lang = '', [string]$Only = 'mob,client,server,worldgen', [switch]$Yes, [switch]$AcceptEula)

# Keeps Cyrillic in redirected output and in the text read from perf-kit.exe.
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. (Join-Path $PSScriptRoot 'common.ps1')

if ((-not $Yes) -and [Console]::IsInputRedirected) {
    $env:PERF_KIT_LANG = $Lang
    if (-not $Lang -and (Get-UICulture).TwoLetterISOLanguageName -eq 'ru') { $env:PERF_KIT_LANG = 'ru' }
    Write-Host (T 'Standard input is redirected, so run.ps1 cannot ask questions. Add -Yes (with -Label and, for the mob section, -AcceptEula) to run unattended.' 'Стандартный ввод перенаправлен, поэтому run.ps1 не может задавать вопросы. Добавьте -Yes (вместе с -Label и, для секции мобов, -AcceptEula), чтобы запуск шел без вопросов.')
    exit 2
}

if ($Yes -and -not $Lang) {
    $Lang = 'en'
    if ((Get-UICulture).TwoLetterISOLanguageName -eq 'ru') { $Lang = 'ru' }
}
if (-not $Lang) {
    $defaultChoice = '1'
    if ((Get-UICulture).TwoLetterISOLanguageName -eq 'ru') { $defaultChoice = '2' }
    while (-not $Lang) {
        $choice = Read-Host "Language / Язык: [1] English  [2] Русский (Enter: $defaultChoice)"
        if (-not $choice) { $choice = $defaultChoice }
        if ($choice -eq '1') { $Lang = 'en' }
        if ($choice -eq '2') { $Lang = 'ru' }
    }
}
$env:PERF_KIT_LANG = $Lang

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
    Write-Host (T "Power plan restored to $previous" "Схема питания возвращена: $previous")
}

$repo = Get-KitRepo
if (-not $repo) { Write-Host (T 'run.ps1 must stay inside the mcrs checkout; run bootstrap.ps1 first.' 'run.ps1 должен лежать внутри клона mcrs; сначала запустите bootstrap.ps1.'); exit 1 }

if ((Test-Path $PowerRestoreFile) -and ($Yes -or (Confirm-Choice (T 'An earlier run stopped before it switched the power plan back. Restore the previous plan now? Otherwise this run restores it when it ends.' 'Прошлый запуск прервался и не вернул схему питания. Вернуть прежнюю схему сейчас? Иначе ее вернет этот запуск, когда закончится.')))) {
    Restore-PowerPlan
}

Write-Host (T "mcrs benchmark kit, profile: $benchProfile" "Бенчмарки mcrs, профиль: $benchProfile")
if ($Full) {
    Write-Host (T 'Expected time: about 1.5-3 hours, depending on the machine.' 'Займет примерно 1,5-3 часа, смотря какая машина.')
} else {
    Write-Host (T 'Expected time: about 30-60 minutes, depending on the machine.' 'Займет примерно 30-60 минут, смотря какая машина.')
}
Write-Host (T 'While it runs, do not touch the computer. The game window must stay visible, on top and not minimised.' 'Пока идут замеры, не трогайте компьютер. Окно игры должно быть видно, поверх остальных окон и не свернуто.')
Write-Host ''

$Label = ($Label -replace '[^A-Za-z0-9_-]+', '-').Trim('-')
if ($Yes -and -not $Label) {
    Write-Host (T '-Yes needs -Label NAME, since the run cannot ask for it.' 'С -Yes нужен -Label NAME, так как запуск не может его спросить.')
    exit 2
}
while (-not $Label) {
    $Label = ((Read-Host (T 'Machine label (for example ryzen-3060 or xps-15)' 'Метка машины (например, ryzen-3060 или xps-15)')) -replace '[^A-Za-z0-9_-]+', '-').Trim('-')
}

# A comma list, not [string[]]: with -File PowerShell passes `mob,server` as one string.
$Only = @($Only.Split(',') | ForEach-Object { $_.Trim() } | Where-Object { $_ })
foreach ($section in $Only) {
    if (@('mob', 'client', 'server', 'worldgen') -notcontains $section) { Write-Host '-Only takes a comma list of mob, client, server, worldgen'; exit 2 }
}
if ($Only.Count -eq 0) { Write-Host '-Only takes a comma list of mob, client, server, worldgen'; exit 2 }
$sections = @()
if ($Only -contains 'mob') {
    Write-Host (T 'The mob benchmark runs the Minecraft server, which needs the Minecraft EULA: https://aka.ms/MinecraftEULA' 'Бенчмарк мобов запускает сервер Minecraft, а для него нужно принять Minecraft EULA: https://aka.ms/MinecraftEULA')
    if ($AcceptEula -or ((-not $Yes) -and (Confirm-Choice (T 'Do you accept the Minecraft EULA?' 'Принять Minecraft EULA?')))) {
        $sections = @('--accept-eula')
    } else {
        Write-Host (T 'Running without the mob section.' 'Запуск без секции мобов.')
        $Only = @($Only | Where-Object { $_ -ne 'mob' })
        if ($Only.Count -eq 0) { exit 0 }
    }
}
$sections += @('--only', ($Only -join ','))

$name = "$Label-windows-$((Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ', [Globalization.CultureInfo]::InvariantCulture))"
$results = Join-Path $KitDir 'results'
$dir = Join-Path $results $name
New-Item -ItemType Directory -Force -Path $dir | Out-Null

$status = 1
try {
    [void][PerfKitAwake]::SetThreadExecutionState($EsContinuous -bor $EsSystemRequired -bor $EsDisplayRequired)

    $systemFile = Join-Path $dir 'system.json'
    & (Join-Path $KitDir 'sysinfo.ps1') -Label $Label -Out $systemFile
    if (-not (Test-Path $systemFile)) { Write-Host (T 'Recording the system failed; see the error above.' 'Не удалось записать сведения о системе; ошибка выше.'); exit 1 }
    $hygieneFile = Join-Path $dir 'hygiene.json'
    if ($Yes) {
        & (Join-Path $KitDir 'prepare.ps1') -CheckOnly -Out $hygieneFile
    } else {
        & (Join-Path $KitDir 'prepare.ps1') -Out $hygieneFile
    }
    if (-not (Test-Path $hygieneFile)) { Write-Host (T 'Preparing the machine failed; see the error above.' 'Подготовка машины не удалась; ошибка выше.'); exit 1 }
    & (Join-Path $KitDir 'bootstrap.ps1') -BuildOnly
    if ($LASTEXITCODE -ne 0) { Write-Host (T "The build failed; run $KitDir\bootstrap.ps1 and try again." "Сборка не удалась; запустите $KitDir\bootstrap.ps1 и попробуйте снова."); exit 1 }

    $java = Find-Java
    if (-not $java) { Write-Host (T "Java $JavaMin not found; run $KitDir\bootstrap.ps1 first." "Java $JavaMin не найдена; сначала запустите $KitDir\bootstrap.ps1."); exit 1 }

    # The client sizes a window in physical pixels (scale factor 1) and keeps it centred and on top,
    # so a candidate is compared with the full pixel size of the primary display.
    $system = Get-Content -Raw -Encoding UTF8 -Path $systemFile | ConvertFrom-Json
    $window = '960x540'
    if ($system.primary_display) {
        $pixels = $system.primary_display.pixels
        foreach ($candidate in @('1920x1080', '1600x900', '1280x720', '960x540')) {
            $size = $candidate.Split('x')
            if ([int]$size[0] -le $pixels.width -and [int]$size[1] -le $pixels.height) { $window = $candidate; break }
        }
    }
    $heap = [math]::Max(1, [math]::Min(16, [math]::Floor($system.memory_bytes / 1GB / 2)))
    Write-Host (T "Window $window, Java heap $heap GB, results in $dir" "Окно $window, куча Java $heap ГБ, результаты в $dir")

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
        Write-Host (T "Could not create the zip; send the folder $dir instead." "Не получилось создать zip; отправьте вместо него папку $dir.")
        exit 1
    }
    Write-Host ''
    if ($status -ne 0) { Write-Host (T "Some sections failed (exit $status); the zip still holds everything that was measured." "Часть секций упала (код $status); в zip все равно есть все, что успели замерить.") }
    Write-Host (T "Done. Send this file: $zip" "Готово. Отправьте этот файл: $zip")
    Write-Host (T 'It holds system.json, hygiene.json, hygiene-after.json, report.json, raw\ and run.log.' 'В нем system.json, hygiene.json, hygiene-after.json, report.json, raw\ и run.log.')
} finally {
    [void][PerfKitAwake]::SetThreadExecutionState($EsContinuous)
    if (Test-Path $PowerRestoreFile) { Restore-PowerPlan }
}
exit $status
