# Checks that this PC is quiet enough to benchmark, offers to fix what it can, writes hygiene.json.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File prepare.ps1 [-CheckOnly] [-Out FILE]
param([switch]$CheckOnly, [string]$Out)

. (Join-Path $PSScriptRoot 'common.ps1')
Add-Type -AssemblyName System.Windows.Forms

$BusyLimit = 10
$GateSeconds = 300
$DaemonWaitSeconds = 300
$HighPerformance = '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c'
$UltimatePerformance = 'e9a42b02-d5df-448d-aa00-03f14749eb61'
$Interactive = -not $CheckOnly

$Watched = @(
    'chrome', 'msedge', 'firefox', 'opera', 'brave', 'vivaldi', 'browser',
    'slack', 'Discord', 'ms-teams', 'Teams', 'Telegram', 'WhatsApp', 'Zoom',
    'Code', 'Cursor', 'idea64', 'rustrover64', 'clion64', 'pycharm64', 'webstorm64', 'goland64', 'studio64', 'devenv',
    'Docker Desktop', 'com.docker.backend', 'Spotify', 'OneDrive', 'Dropbox', 'GoogleDriveFS',
    'steam', 'steamwebhelper', 'EpicGamesLauncher', 'obs64', 'Battle.net', 'MinecraftLauncher'
)
$Daemons = @('MsMpEng', 'TiWorker', 'TrustedInstaller', 'SearchIndexer', 'SearchProtocolHost', 'MoUsoCoreWorker', 'CompatTelRunner', 'MpDefenderCoreService')

$checks = New-Object System.Collections.ArrayList
function Add-Check([string]$Name, [string]$Value, $Ok, [string]$Action) {
    if (-not $Action) { $Action = 'none' }
    [void]$checks.Add([pscustomobject][ordered]@{ name = $Name; value = $Value; ok = $Ok; action = $Action })
    $shown = 'n/a'
    if ($null -ne $Ok) { $shown = "$Ok".ToLower() }
    Write-Host ('  {0,-28} {1,-40} {2} ({3})' -f $Name, $Value, $shown, $Action)
}

# Processes of this script and its parents (the terminal or IDE that runs it) are never closed.
function Get-OwnProcesses {
    $byId = @{}
    Get-CimInstance Win32_Process | ForEach-Object { $byId[[int]$_.ProcessId] = $_ }
    $own = @{}
    $id = $PID
    while ($byId.ContainsKey($id) -and -not $own.ContainsKey($id)) {
        $own[$id] = [IO.Path]::GetFileNameWithoutExtension($byId[$id].Name)
        $id = [int]$byId[$id].ParentProcessId
    }
    return $own
}

function Test-AppGone($Ids) {
    return (@(Get-Process -Id $Ids -ErrorAction SilentlyContinue).Count -eq 0)
}

function Close-App([string]$Name, $Processes) {
    $ids = @($Processes | ForEach-Object { $_.Id })
    $Processes | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | ForEach-Object { [void]$_.CloseMainWindow() }
    Wait-Process -Id $ids -Timeout 10 -ErrorAction SilentlyContinue
    if (Test-AppGone $ids) { return 'closed' }
    if (Confirm-Choice (T "  $Name is still running (it may sit in the tray). Force it to stop? Unsaved work in it is lost." "  $Name все еще работает (возможно, свернуто в трей). Завершить принудительно? Несохраненные данные в нем пропадут.")) {
        Stop-Process -Id $ids -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
        if (Test-AppGone $ids) { return 'forced' }
        return 'kept: could not stop'
    }
    return 'kept: did not close'
}

function Test-Apps {
    $own = Get-OwnProcesses
    $logical = [Environment]::ProcessorCount
    $before = @{}
    Get-Process | ForEach-Object { if ($_.TotalProcessorTime) { $before[$_.Id] = $_.TotalProcessorTime.TotalSeconds } }
    Start-Sleep -Seconds 2
    $groups = Get-Process | Where-Object { $Watched -contains $_.ProcessName } | Group-Object ProcessName
    foreach ($group in $groups) {
        $cpuSeconds = 0.0
        foreach ($process in $group.Group) {
            if ($process.TotalProcessorTime -and $before.ContainsKey($process.Id)) {
                $cpuSeconds += $process.TotalProcessorTime.TotalSeconds - $before[$process.Id]
            }
        }
        $cpu = [math]::Round($cpuSeconds / 2 / $logical * 100, 1)
        $ramMb = [math]::Round(($group.Group | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB)
        $value = "cpu $cpu% ram $ramMb MB"
        $isOwn = $own.Values -contains $group.Name
        foreach ($process in $group.Group) { if ($own.ContainsKey($process.Id)) { $isOwn = $true } }
        if ($isOwn) {
            Add-Check "app: $($group.Name)" $value $false 'kept: runs this script'
            continue
        }
        $action = 'none'
        if ($Interactive) {
            $action = 'kept'
            if (Confirm-Choice (T "Close $($group.Name) ($value)?" "Закрыть $($group.Name) ($value)?")) { $action = Close-App $group.Name $group.Group }
        }
        Add-Check "app: $($group.Name)" $value ($action -eq 'closed' -or $action -eq 'forced') $action
    }
}

# Busy % of the named processes, from the performance classes; their names are not localised.
function Get-DaemonCpu {
    $logical = [Environment]::ProcessorCount
    # Formatted rate counters read 0 on the first query, so the second sample counts.
    Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | Out-Null
    Start-Sleep -Seconds 1
    $total = 0.0
    Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -ErrorAction SilentlyContinue | ForEach-Object {
        if ($Daemons -contains ($_.Name -replace '#\d+$', '')) { $total += $_.PercentProcessorTime }
    }
    return [math]::Round($total / $logical, 1)
}

function Test-Daemons {
    $cpu = Get-DaemonCpu
    $waited = 0
    while ($Interactive -and $cpu -gt 20 -and $waited -lt $DaemonWaitSeconds) {
        Write-Host (T "  Defender, Windows Update or the search indexer use $cpu% CPU; waiting for them to calm down ($waited s of $DaemonWaitSeconds s)" "  Defender, Windows Update или индексатор поиска занимают $cpu% CPU; ждем, пока утихнут ($waited с из $DaemonWaitSeconds с)")
        Start-Sleep -Seconds 15
        $waited += 15
        $cpu = Get-DaemonCpu
    }
    $action = ''
    if ($waited -gt 0) { $action = "waited $waited s" }
    Add-Check 'background services' "cpu $cpu%" ($cpu -le 20) $action
}

function Get-PowerLine {
    return "$([System.Windows.Forms.SystemInformation]::PowerStatus.PowerLineStatus)"
}

function Test-Power {
    $line = Get-PowerLine
    if ($Interactive -and $line -eq 'Offline') {
        Read-Host (T '  The PC runs on battery. Plug in the charger, then press Enter' '  Компьютер работает от батареи. Подключите зарядку и нажмите Enter') | Out-Null
        $line = Get-PowerLine
    }
    $value = 'ac'
    if ($line -eq 'Offline') { $value = 'battery' }
    if ($line -eq 'Unknown') { $value = 'unknown' }
    Add-Check 'power source' $value ($line -ne 'Offline') ''

    $scheme = Get-NativeOutput 'powercfg' @('/getactivescheme')
    $guid = ''
    if ($scheme -match '([0-9a-fA-F-]{36})') { $guid = $Matches[1].ToLower() }
    $fast = ($guid -eq $HighPerformance -or $guid -eq $UltimatePerformance)
    $action = ''
    if ($Interactive -and -not $fast -and $guid -and (Confirm-Choice (T '  Switch the power plan to High performance for the run? run.ps1 switches it back at the end.' '  Переключить схему питания на Высокую производительность на время замеров? В конце run.ps1 вернет прежнюю.'))) {
        Get-NativeOutput 'powercfg' @('/setactive', $HighPerformance) | Out-Null
        if ($LASTEXITCODE -eq 0) {
            if (-not (Test-Path $PowerRestoreFile)) {
                New-Item -ItemType Directory -Force -Path $KitHome | Out-Null
                [IO.File]::WriteAllText($PowerRestoreFile, $guid)
            }
            $action = "switched from $guid"
            $fast = $true
            Write-Host (T '  The power plan stays on High performance until run.ps1 finishes; if the run stops early, the next run.ps1 offers to switch it back.' '  Схема Высокая производительность останется до конца run.ps1; если запуск прервется, следующий run.ps1 предложит вернуть прежнюю.')
        } else {
            $action = 'High performance plan not available; set Settings > System > Power > Power mode to Best performance'
        }
    }
    Add-Check 'power plan' $scheme $fast $action
    Add-Check 'thermal state' 'not readable without administrator rights' $null ''
}

# Average busy % of all CPUs over 10 one-second samples.
function Get-CpuBusy {
    $samples = @()
    for ($i = 0; $i -lt 11; $i++) {
        $total = Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'"
        if ($i -gt 0) { $samples += [double]$total.PercentProcessorTime }
        Start-Sleep -Seconds 1
    }
    return [math]::Round(($samples | Measure-Object -Average).Average, 1)
}

function Test-Idle {
    $started = Get-Date
    Write-Host (T '  measuring CPU load over 10 s' '  замеряем нагрузку на CPU за 10 с')
    $busy = Get-CpuBusy
    while ($Interactive -and $busy -ge $BusyLimit -and ((Get-Date) - $started).TotalSeconds -lt $GateSeconds) {
        Write-Host (T "  CPU is $busy% busy (limit $BusyLimit%). Busiest processes:" "  CPU загружен на $busy% (предел $BusyLimit%). Самые прожорливые процессы:")
        Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
            Where-Object { $_.Name -ne '_Total' -and $_.Name -ne 'Idle' } |
            Sort-Object PercentProcessorTime -Descending | Select-Object -First 5 |
            ForEach-Object { Write-Host "    $($_.PercentProcessorTime)% $($_.Name)" }
        $busy = Get-CpuBusy
    }
    $action = ''
    if ($Interactive -and $busy -ge $BusyLimit) { $action = "gave up after $GateSeconds s" }
    Add-Check 'cpu idle gate' "$busy% busy" ($busy -lt $BusyLimit) $action
}

function Test-Memory {
    $os = Get-CimInstance Win32_OperatingSystem
    $free = [math]::Round($os.FreePhysicalMemory / 1MB, 1)
    $total = [math]::Round($os.TotalVisibleMemorySize / 1MB)
    Add-Check 'free memory' "$free of $total GB" ($free * 2 -ge $total) ''
}

function Test-Session {
    $remote = ($env:SESSIONNAME -like 'RDP-*') -or [System.Windows.Forms.SystemInformation]::TerminalServerSession
    Add-Check 'remote desktop' "$remote" (-not $remote) ''
}

# The record keeps the key, not the question.
function Add-Manual([string]$Name, [string]$Question, [string]$QuestionRu) {
    $yes = Confirm-Choice ('  ' + (T $Question $QuestionRu))
    $value = 'no'
    if ($yes) { $value = 'yes' }
    Add-Check "manual: $Name" $value $yes ''
}

$mode = 'interactive'
if ($CheckOnly) { $mode = 'check-only' }
Write-Host (T "== hygiene ($mode)" "== подготовка машины ($mode)")
Test-Apps
Test-Daemons
Test-Power
Test-Idle
Test-Memory
Test-Session
if ($Interactive) {
    Write-Host (T 'Please confirm (answers are recorded, nothing is enforced):' 'Подтвердите (ответы попадут в отчет, скрипт их не проверяет):')
    Add-Manual 'focus' 'Is Do not disturb (Focus assist) on, so no notification covers the window?' `
        'Включен режим Не беспокоить (Помощник по фокусировке), чтобы уведомления не перекрывали окно?'
    Add-Manual 'vsync override' 'In the NVIDIA, AMD or Intel control panel, is Vertical sync left to the application (no override)? Note G-Sync or FreeSync if on.' `
        'В панели управления NVIDIA, AMD или Intel вертикальная синхронизация отдана приложению (без переопределения)? Если включен G-Sync или FreeSync, сообщите об этом вместе с результатами.'
    Add-Manual 'gpu' 'On a laptop with two GPUs: is the discrete GPU set for the mcrs client (Settings > Display > Graphics > High performance)? Answer y on a desktop.' `
        'Ноутбук с двумя GPU: для клиента mcrs выбрана дискретная видеокарта (Параметры > Дисплей > Графика > Высокая производительность)? На настольном ПК ответьте y.'
    Add-Manual 'primary display' 'If you use an external display, is it the main display?' `
        'Если подключен внешний дисплей, он основной?'
    Add-Manual 'display stays on' 'Is the screen saver off and will the display stay on?' `
        'Заставка выключена, и экран не погаснет?'
    Add-Manual 'nothing else' 'Will you leave the computer alone and start nothing else until the run ends?' `
        'До конца замеров компьютер никто не трогает и ничего нового не запускает?'
} else {
    foreach ($key in @('focus', 'vsync override', 'gpu', 'primary display', 'display stays on', 'nothing else')) {
        Add-Check "manual: $key" 'not asked' $false ''
    }
}

$doc = [ordered]@{
    mode = $mode
    collected_at = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ', [Globalization.CultureInfo]::InvariantCulture)
    os = 'windows'
    checks = @($checks)
}
if ($Out) {
    Write-JsonFile $doc $Out
    Write-Host (T "hygiene written to $Out" "результаты проверки записаны в $Out")
} else {
    $doc | ConvertTo-Json -Depth 6
}
