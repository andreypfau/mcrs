#!/bin/bash
# Writes system.json: hardware, displays, power and toolchain of this Mac. No user, host or serial data.
# Usage: sysinfo.sh [--out FILE] [--label NAME]
set -u
# shellcheck source=SCRIPTDIR/common.sh
. "$(dirname "$0")/common.sh"

out=""
label=""
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="$2"; shift 2 ;;
        --label) label="$2"; shift 2 ;;
        *) say "unknown argument: $1"; exit 2 ;;
    esac
done

repo="$(repo_of_kit)"
java="$(find_java || true)"
# shellcheck disable=SC1091
[ -f "${CARGO_HOME:-$HOME/.cargo}/env" ] && . "${CARGO_HOME:-$HOME/.cargo}/env"

collect() {
    osascript -l JavaScript - "$@" <<'JXA'
ObjC.import('Foundation');
ObjC.import('AppKit');
const app = Application.currentApplication();
app.includeStandardAdditions = true;

function sh(cmd) {
    try { return app.doShellScript(cmd, { alteringLineEndings: false }).trim(); } catch (e) { return null; }
}
function quote(s) { return "'" + s.replace(/'/g, "'\\''") + "'"; }
function int(s) { const n = parseInt(s, 10); return isNaN(n) ? null : n; }
function sysctl(key) { return sh('/usr/sbin/sysctl -n ' + key + ' 2>/dev/null'); }

function run(argv) {
    const label = argv[0], repo = argv[1], java = argv[2];
    const profile = JSON.parse(sh('/usr/sbin/system_profiler SPHardwareDataType SPDisplaysDataType -json') || '{}');
    const hw = (profile.SPHardwareDataType || [{}])[0];

    const perflevels = [];
    const levels = int(sysctl('hw.nperflevels')) || 0;
    for (let i = 0; i < levels; i++) {
        perflevels.push({
            name: sysctl('hw.perflevel' + i + '.name'),
            physical_cores: int(sysctl('hw.perflevel' + i + '.physicalcpu')),
        });
    }

    const gpus = [], displays = [];
    (profile.SPDisplaysDataType || []).forEach(function (gpu) {
        gpus.push({ model: gpu.sppci_model || gpu._name || null, cores: int(gpu.sppci_cores), vram: gpu.spdisplays_vram || null });
        (gpu.spdisplays_ndrvs || []).forEach(function (d) {
            const res = /(\d+)\s*x\s*(\d+)(?:\s*@\s*([\d.]+)\s*Hz)?/.exec(d._spdisplays_resolution || '') || [];
            const px = /(\d+)\s*x\s*(\d+)/.exec(d._spdisplays_pixels || '') || [];
            displays.push({
                name: d._name || null,
                gpu: gpu.sppci_model || null,
                main: d.spdisplays_main === 'spdisplays_yes',
                internal: d.spdisplays_connection_type === 'spdisplays_internal',
                pixels: { width: int(px[1]), height: int(px[2]) },
                scaled: { width: int(res[1]), height: int(res[2]) },
                refresh_hz: res[3] ? parseFloat(res[3]) : null,
            });
        });
    });

    const screens = $.NSScreen.screens;
    const primary = screens.count > 0 ? screens.objectAtIndex(0) : null;
    const points = primary ? { width: primary.frame.size.width, height: primary.frame.size.height } : null;
    const visible = primary ? { width: primary.visibleFrame.size.width, height: primary.visibleFrame.size.height } : null;

    const batt = sh('/usr/bin/pmset -g batt') || '';
    const pmset = sh('/usr/bin/pmset -g') || '';
    const pmValue = function (key) { const m = new RegExp('^\\s*' + key + '\\s+(\\d+)', 'm').exec(pmset); return m ? int(m[1]) : null; };
    const percent = /(\d+)%/.exec(batt);
    const thermal = ['nominal', 'fair', 'serious', 'critical'][$.NSProcessInfo.processInfo.thermalState] || 'unknown';
    const where = repo || app.pathTo('home folder').toString();
    const freeKb = int(sh('/bin/df -k ' + quote(where) + " | awk 'NR == 2 { print $4 }'"));
    const inRepo = function (cmd) { return repo ? sh('cd ' + quote(repo) + ' && ' + cmd + ' 2>&1') : null; };

    const doc = JSON.stringify({
        schema: 1,
        label: label,
        collected_at: new Date().toISOString(),
        os: { name: 'macOS', version: sh('/usr/bin/sw_vers -productVersion'), build: sh('/usr/bin/sw_vers -buildVersion'), arch: sh('/usr/bin/uname -m') },
        machine: { model: hw.machine_model || null, name: hw.machine_name || null, chip: hw.chip_type || null },
        cpu: {
            brand: sysctl('machdep.cpu.brand_string'),
            physical_cores: int(sysctl('hw.physicalcpu')),
            logical_cores: int(sysctl('hw.logicalcpu')),
            perflevels: perflevels,
        },
        memory_bytes: int(sysctl('hw.memsize')),
        gpus: gpus,
        displays: displays,
        primary_screen_points: points,
        primary_screen_visible_points: visible,
        power: {
            source: /AC Power/.test(batt) ? 'ac' : /Battery Power/.test(batt) ? 'battery' : 'unknown',
            has_battery: /InternalBattery/.test(batt),
            battery_percent: percent ? int(percent[1]) : null,
            powermode: pmValue('powermode'),
            lowpowermode: pmValue('lowpowermode'),
            low_power_mode_enabled: !!$.NSProcessInfo.processInfo.isLowPowerModeEnabled,
        },
        thermal_state: thermal,
        disk_free_bytes: freeKb === null ? null : freeKb * 1024,
        toolchain: {
            rustc: inRepo('rustc -Vv'),
            java_path: java || null,
            java: java ? sh(quote(java) + ' -version 2>&1') : null,
        },
        repo: repo ? { commit: inRepo('git rev-parse HEAD'), dirty: (inRepo('git status --porcelain') || '').length > 0 } : null,
    }, null, 2);
    return doc.split($.NSHomeDirectory().js).join('~');
}
JXA
}

json="$(collect "$label" "$repo" "$java")" || { say "sysinfo: osascript failed"; exit 1; }

if [ -n "$out" ]; then
    printf '%s\n' "$json" >"$out"
    say "system info written to $out"
else
    printf '%s\n' "$json"
fi
