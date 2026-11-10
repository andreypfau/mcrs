# Writes system.json: hardware, displays, power and toolchain of this PC. No user, host or serial data.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File sysinfo.ps1 [-Out FILE] [-Label NAME]
param([string]$Out, [string]$Label = '')

. (Join-Path $PSScriptRoot 'common.ps1')

Add-Type -AssemblyName System.Windows.Forms
if (-not ('PerfKitDisplay' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PerfKitDisplay {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public short dmSpecVersion;
        public short dmDriverVersion;
        public short dmSize;
        public short dmDriverExtra;
        public int dmFields;
        public int dmPositionX;
        public int dmPositionY;
        public int dmDisplayOrientation;
        public int dmDisplayFixedOutput;
        public short dmColor;
        public short dmDuplex;
        public short dmYResolution;
        public short dmTTOption;
        public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public short dmLogPixels;
        public int dmBitsPerPel;
        public int dmPelsWidth;
        public int dmPelsHeight;
        public int dmDisplayFlags;
        public int dmDisplayFrequency;
        public int dmICMMethod;
        public int dmICMIntent;
        public int dmMediaType;
        public int dmDitherType;
        public int dmReserved1;
        public int dmReserved2;
        public int dmPanningWidth;
        public int dmPanningHeight;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplaySettings(string deviceName, int modeNum, ref DEVMODE devMode);

    public static DEVMODE Current(string deviceName) {
        DEVMODE mode = new DEVMODE();
        mode.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE));
        EnumDisplaySettings(deviceName, -1, ref mode);
        return mode;
    }
}
'@
}

function Get-RegistryValue([string]$Key, [string]$Name) {
    $item = Get-ItemProperty -Path $Key -Name $Name -ErrorAction SilentlyContinue
    if ($item) { return $item.$Name }
    return $null
}

$repo = Get-KitRepo
$java = Find-Java
$processors = @(Get-CimInstance Win32_Processor)
$computer = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem

$vram = @{}
$classKey = 'HKLM:\SYSTEM\ControlSet001\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}'
Get-ChildItem -Path $classKey -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -match '^\d{4}$' } | ForEach-Object {
    $props = Get-ItemProperty -Path $_.PSPath -ErrorAction SilentlyContinue
    if ($props -and $props.DriverDesc -and $props.'HardwareInformation.qwMemorySize') {
        $vram[$props.DriverDesc] = [int64]$props.'HardwareInformation.qwMemorySize'
    }
}
$gpus = @(Get-CimInstance Win32_VideoController | ForEach-Object {
    $bytes = $vram[$_.Name]
    if (-not $bytes) { $bytes = $_.AdapterRAM }
    [ordered]@{
        model = $_.Name
        driver_version = $_.DriverVersion
        driver_date = "$($_.DriverDate)"
        vram_bytes = $bytes
        refresh_hz = $_.CurrentRefreshRate
    }
})
$nvidia = $null
if (Get-Command nvidia-smi -ErrorAction SilentlyContinue) {
    $nvidia = Get-NativeOutput 'nvidia-smi' @('--query-gpu=name,driver_version,memory.total', '--format=csv,noheader')
}

$appliedDpi = Get-RegistryValue 'HKCU:\Control Panel\Desktop\WindowMetrics' 'AppliedDPI'
if (-not $appliedDpi) { $appliedDpi = 96 }
$scale = [double]$appliedDpi / 96
$displays = @([System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
    $mode = [PerfKitDisplay]::Current($_.DeviceName)
    [ordered]@{
        device = $_.DeviceName
        primary = $_.Primary
        pixels = [ordered]@{ width = $mode.dmPelsWidth; height = $mode.dmPelsHeight }
        bounds = [ordered]@{ x = $_.Bounds.X; y = $_.Bounds.Y; width = $_.Bounds.Width; height = $_.Bounds.Height }
        refresh_hz = $mode.dmDisplayFrequency
        bits_per_pixel = $mode.dmBitsPerPel
    }
})
# Assumption: AppliedDPI is the scale of the primary display, so its logical size is pixels / scale.
$primary = $displays | Where-Object { $_.primary } | Select-Object -First 1
$primaryDisplay = $null
if ($primary) {
    $primaryDisplay = [ordered]@{
        pixels = $primary.pixels
        scale = $scale
        logical = [ordered]@{
            width = [int][math]::Floor($primary.pixels.width / $scale)
            height = [int][math]::Floor($primary.pixels.height / $scale)
        }
    }
}

$powerStatus = [System.Windows.Forms.SystemInformation]::PowerStatus
$battery = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue | Select-Object -First 1
$scheme = Get-NativeOutput 'powercfg' @('/getactivescheme')
$schemeGuid = $null
$schemeName = $null
if ($scheme -match '([0-9a-fA-F-]{36})\s+\((.+)\)') {
    $schemeGuid = $Matches[1]
    $schemeName = $Matches[2]
}

$where = $env:USERPROFILE
if ($repo) { $where = $repo }
$freeGb = Get-FreeGB $where
$diskFree = $null
if ($null -ne $freeGb) { $diskFree = [int64]$freeGb * 1GB }
$source = 'unknown'
if ("$($powerStatus.PowerLineStatus)" -eq 'Online') { $source = 'ac' }
if ("$($powerStatus.PowerLineStatus)" -eq 'Offline') { $source = 'battery' }

$rustc = $null
$commit = $null
$dirty = $null
if ($repo) {
    Push-Location $repo
    $rustc = Get-NativeOutput 'rustc' @('-Vv')
    $commit = Get-NativeOutput 'git' @('rev-parse', 'HEAD')
    $dirty = [bool](Get-NativeOutput 'git' @('status', '--porcelain'))
    Pop-Location
}
$javaVersion = $null
if ($java) { $javaVersion = Get-NativeOutput $java @('-version') }

$batteryPercent = $null
if ($battery) { $batteryPercent = $battery.EstimatedChargeRemaining }
$repoInfo = $null
if ($repo) { $repoInfo = [ordered]@{ commit = $commit; dirty = $dirty } }

$doc = [ordered]@{
    schema = 1
    label = $Label
    collected_at = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ', [Globalization.CultureInfo]::InvariantCulture)
    os = [ordered]@{ name = 'Windows'; caption = $os.Caption; version = $os.Version; build = $os.BuildNumber; arch = $os.OSArchitecture }
    machine = [ordered]@{ manufacturer = $computer.Manufacturer; model = $computer.Model; laptop = ($computer.PCSystemType -eq 2) }
    cpu = [ordered]@{
        brand = ($processors | Select-Object -First 1).Name.Trim()
        sockets = $processors.Count
        physical_cores = ($processors | Measure-Object -Property NumberOfCores -Sum).Sum
        logical_cores = ($processors | Measure-Object -Property NumberOfLogicalProcessors -Sum).Sum
        max_clock_mhz = ($processors | Select-Object -First 1).MaxClockSpeed
    }
    memory_bytes = [int64]$computer.TotalPhysicalMemory
    gpus = $gpus
    nvidia_smi = $nvidia
    displays = $displays
    primary_display = $primaryDisplay
    applied_dpi = $appliedDpi
    power = [ordered]@{
        source = $source
        has_battery = [bool]$battery
        battery_percent = $batteryPercent
        scheme_guid = $schemeGuid
        scheme_name = $schemeName
    }
    hardware_gpu_scheduling = Get-RegistryValue 'HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers' 'HwSchMode'
    game_mode = Get-RegistryValue 'HKCU:\Software\Microsoft\GameBar' 'AutoGameModeEnabled'
    remote_session = ($env:SESSIONNAME -like 'RDP-*')
    disk_free_bytes = $diskFree
    toolchain = [ordered]@{ rustc = $rustc; java_path = $java; java = $javaVersion }
    repo = $repoInfo
}

if ($Out) {
    Write-JsonFile $doc $Out
    Write-Host "system info written to $Out"
} else {
    $json = $doc | ConvertTo-Json -Depth 6
    $json.Replace($env:USERPROFILE.Replace('\', '\\'), '~')
}
