#!/bin/bash
# Checks that this Mac is quiet enough to benchmark, offers to fix what it can, writes hygiene.json.
# Usage: prepare.sh [--check-only] [--out FILE]
set -u
# shellcheck source=SCRIPTDIR/common.sh
. "$(dirname "$0")/common.sh"
export LC_ALL=C

mode=interactive
out=""
while [ $# -gt 0 ]; do
    case "$1" in
        --check-only) mode=check-only; shift ;;
        --out) out="$2"; shift 2 ;;
        *) msg "unknown argument: $1" "неизвестный аргумент: $1"; exit 2 ;;
    esac
done

BUSY_LIMIT=10
GATE_SECONDS=300
DAEMON_WAIT_SECONDS=300

checks="$(mktemp -t perfkit-hygiene)"
trap 'rm -f "$checks"' EXIT

# record NAME VALUE OK ACTION
record() {
    printf '%s\t%s\t%s\t%s\n' "$1" "$(printf '%s' "$2" | tr '\t\n' '  ')" "$3" "$4" >>"$checks"
    printf '  %-28s %-40s %s\n' "$1" "$2" "${3:-n/a}${4:+ ($4)}" >&2
}

interactive() { [ "$mode" = interactive ]; }

# One line per app bundle (or bare process): name, CPU %, RSS MB, pids.
app_table() {
    ps -Axo pid=,pcpu=,rss=,comm= | awk '
        {
            pid = $1; cpu = $2; rss = $3
            path = $0
            sub(/^ *[0-9]+ +[0-9.]+ +[0-9]+ +/, "", path)
            name = path
            if (match(path, /\/[^\/]+\.app\//)) {
                name = substr(path, RSTART + 1, RLENGTH - 6)
            } else {
                sub(/.*\//, "", name)
            }
            cpus[name] += cpu; mem[name] += rss; pids[name] = pids[name] " " pid
        }
        END { for (n in cpus) printf "%s\t%.1f\t%d\t%s\n", n, cpus[n], mem[n] / 1024, pids[n] }'
}

is_watched() {
    case "$1" in
        "Google Chrome" | Safari | Firefox | "Microsoft Edge" | Arc | "Brave Browser" | Opera | Vivaldi | Chromium | "Yandex") return 0 ;;
        Slack | Discord | "Microsoft Teams"* | Telegram | WhatsApp | zoom.us | Skype) return 0 ;;
        "Visual Studio Code" | Cursor | Zed | "IntelliJ IDEA"* | RustRover | CLion | PyCharm* | WebStorm | GoLand | "Android Studio" | Xcode | Fleet) return 0 ;;
        Docker | "Docker Desktop" | OrbStack | Spotify | OneDrive | Dropbox | "Google Drive" | Steam | "Epic Games Launcher" | OBS | "Battle.net" | "Minecraft"*) return 0 ;;
    esac
    return 1
}

is_daemon() {
    case "$1" in
        mds | mds_stores | mdworker | mdworker_shared | backupd | bird | cloudd | softwareupdated | photoanalysisd | mediaanalysisd | XProtect*) return 0 ;;
    esac
    return 1
}

# App names of this script's ancestors: the terminal or IDE that runs it is never closed.
ancestor_apps() {
    local pid=$$ list=""
    while [ -n "$pid" ] && [ "$pid" -gt 1 ]; do
        list="$list $pid"
        pid="$(ps -o ppid= -p "$pid" | tr -d ' ')"
    done
    app_table | while IFS="$(printf '\t')" read -r name _ _ pids; do
        for p in $pids; do
            case " $list " in *" $p "*) printf '%s\n' "$name"; break ;; esac
        done
    done
}

pids_of() {
    app_table | awk -F'\t' -v n="$1" '$1 == n { print $4 }'
}

close_app() {
    local name="$1" waited=0 pids
    osascript -e "quit app \"$name\"" >/dev/null 2>&1
    while [ $waited -lt 10 ]; do
        [ -z "$(pids_of "$name")" ] && { echo closed; return; }
        sleep 1
        waited=$((waited + 1))
    done
    if ask "$(msg "  $name did not quit within 10 s. Force quit it? Unsaved work in it is lost." "  Приложение $name не закрылось за 10 с. Завершить принудительно? Несохраненные данные в нем пропадут.")"; then
        pids="$(pids_of "$name")"
        # shellcheck disable=SC2086
        kill $pids 2>/dev/null
        sleep 3
        pids="$(pids_of "$name")"
        # shellcheck disable=SC2086
        [ -n "$pids" ] && kill -9 $pids 2>/dev/null
        echo forced
    else
        echo "kept: did not quit"
    fi
}

check_apps() {
    local own name cpu mem pids action
    own="$(ancestor_apps)"
    app_table | sort -t "$(printf '\t')" -k2,2nr >"$checks.apps"
    while IFS="$(printf '\t')" read -r name cpu mem pids; do
        is_watched "$name" && [ "$mem" -gt 0 ] || continue
        if printf '%s\n' "$own" | grep -qxF "$name"; then
            record "app: $name" "cpu ${cpu}% ram ${mem} MB" false "kept: runs this script"
            continue
        fi
        action=none
        if interactive && ask "$(msg "Close $name (CPU ${cpu}%, RAM ${mem} MB)?" "Закрыть $name (CPU ${cpu}%, RAM ${mem} МБ)?")" </dev/tty; then
            action="$(close_app "$name" </dev/tty)"
        elif interactive; then
            action=kept
        fi
        case "$action" in closed | forced) record "app: $name" "cpu ${cpu}% ram ${mem} MB" true "$action" ;;
            *) record "app: $name" "cpu ${cpu}% ram ${mem} MB" false "$action" ;; esac
    done <"$checks.apps"
    rm -f "$checks.apps"
}

daemon_cpu() {
    local total=0 name cpu
    while IFS="$(printf '\t')" read -r name cpu _ _; do
        is_daemon "$name" && total="$(awk -v a="$total" -v b="$cpu" 'BEGIN { print a + b }')"
    done <<EOF
$(app_table)
EOF
    printf '%s\n' "$total"
}

check_daemons() {
    local cpu waited=0
    cpu="$(daemon_cpu)"
    while interactive && awk -v c="$cpu" 'BEGIN { exit !(c > 20) }' && [ $waited -lt $DAEMON_WAIT_SECONDS ]; do
        msg "  Spotlight, iCloud or backup daemons use ${cpu}% CPU; waiting for them to calm down ($waited s of $DAEMON_WAIT_SECONDS s)" \
            "  Spotlight, iCloud или резервное копирование занимают ${cpu}% CPU; ждем, пока утихнут ($waited с из $DAEMON_WAIT_SECONDS с)"
        sleep 15
        waited=$((waited + 15))
        cpu="$(daemon_cpu)"
    done
    if awk -v c="$cpu" 'BEGIN { exit !(c > 20) }'; then
        record "background daemons" "cpu ${cpu}%" false "waited ${waited} s"
    else
        record "background daemons" "cpu ${cpu}%" true ""
    fi
}

power_source() {
    pmset -g batt | awk 'NR == 1 { if (/AC Power/) print "ac"; else if (/Battery Power/) print "battery"; else print "unknown" }'
}

# Prints "thermal_state low_power_mode".
process_state() {
    osascript -l JavaScript -e 'ObjC.import("Foundation"); const p = $.NSProcessInfo.processInfo; ["nominal", "fair", "serious", "critical"][p.thermalState] + " " + (p.isLowPowerModeEnabled ? "on" : "off")'
}

check_power() {
    local source state
    source="$(power_source)"
    if interactive && [ "$source" = battery ]; then
        msg "  The Mac runs on battery. Plug in the charger, then press Enter." "  Mac работает от батареи. Подключите зарядку и нажмите Enter."
        read -r _ </dev/tty
        source="$(power_source)"
    fi
    if [ "$source" = battery ]; then record "power source" "$source" false "asked to plug in"; else record "power source" "$source" true ""; fi

    state="$(process_state)"
    if interactive && [ "${state#* }" = on ]; then
        msg "  Low Power Mode is on. Turn it off in System Settings > Battery (Low Power Mode: Never), then press Enter." \
            "  Включен режим энергосбережения. Выключите его в Системных настройках > Аккумулятор (Режим энергосбережения: Никогда) и нажмите Enter."
        read -r _ </dev/tty
        state="$(process_state)"
    fi
    if [ "${state#* }" = on ]; then record "low power mode" on false ""; else record "low power mode" off true ""; fi
    if [ "${state% *}" = nominal ]; then record "thermal state" "${state% *}" true ""; else record "thermal state" "${state% *}" false ""; fi
}

# Busy CPU % over a 10 s window, from the second top sample.
cpu_busy() {
    top -l 2 -n 0 -s 10 | awk '/CPU usage/ { idle = $7 } END { sub(/%/, "", idle); printf "%.1f\n", 100 - idle }'
}

check_idle() {
    local busy started now
    started="$(date +%s)"
    msg "  measuring CPU load over 10 s" "  замеряем нагрузку на CPU за 10 с"
    busy="$(cpu_busy)"
    while interactive && awk -v b="$busy" -v l="$BUSY_LIMIT" 'BEGIN { exit !(b >= l) }'; do
        now="$(date +%s)"
        [ $((now - started)) -ge $GATE_SECONDS ] && break
        msg "  CPU is ${busy}% busy (limit ${BUSY_LIMIT}%). Busiest processes:" "  CPU загружен на ${busy}% (предел ${BUSY_LIMIT}%). Самые прожорливые процессы:"
        ps -Arco pcpu=,comm= | head -5 | sed 's/^/    /'
        busy="$(cpu_busy)"
    done
    if awk -v b="$busy" -v l="$BUSY_LIMIT" 'BEGIN { exit !(b < l) }'; then
        record "cpu idle gate" "${busy}% busy" true ""
    else
        record "cpu idle gate" "${busy}% busy" false "$(interactive && echo "gave up after $GATE_SECONDS s")"
    fi
}

check_time_machine() {
    local running action=""
    running="$(tmutil status 2>/dev/null | awk -F'= ' '/Running/ { gsub(/;/, "", $2); print $2 }')"
    if interactive && [ "$running" = 1 ] && ask "$(msg "  A Time Machine backup is running. Stop it now?" "  Идет резервное копирование Time Machine. Остановить его?")" </dev/tty; then
        tmutil stopbackup >/dev/null 2>&1
        action=stopped
        sleep 5
        running="$(tmutil status 2>/dev/null | awk -F'= ' '/Running/ { gsub(/;/, "", $2); print $2 }')"
    fi
    if [ "$running" = 1 ]; then record "time machine" running false "$action"; else record "time machine" idle true "$action"; fi
}

check_memory() {
    local total free_gb
    total="$(sysctl -n hw.memsize)"
    free_gb="$(vm_stat | awk -v total="$total" '
        /page size of/ { size = $8 }
        /Pages free/ || /Pages inactive/ || /Pages speculative/ { gsub(/\./, "", $NF); pages += $NF }
        END { printf "%.1f %d\n", pages * size / 1073741824, total / 1073741824 }')"
    if awk -v f="${free_gb% *}" -v t="${free_gb#* }" 'BEGIN { exit !(f * 2 >= t) }'; then
        record "free memory" "${free_gb% *} of ${free_gb#* } GB" true ""
    else
        record "free memory" "${free_gb% *} of ${free_gb#* } GB" false ""
    fi
}

# manual KEY "english question" "русский вопрос"; the record keeps the key, not the question.
manual() {
    if ask "  $(msg "$2" "$3")" </dev/tty; then record "manual: $1" yes true ""; else record "manual: $1" no false ""; fi
}

msg "== hygiene ($mode)" "== подготовка машины ($mode)"
check_apps
check_daemons
check_power
check_idle
check_time_machine
check_memory
if interactive; then
    msg "Please confirm (answers are recorded, nothing is enforced):" "Подтвердите (ответы попадут в отчет, скрипт их не проверяет):"
    manual "focus" "Is Do Not Disturb or a Focus mode on, so no notification covers the window?" \
        "Включен режим Не беспокоить или другой режим фокусирования, чтобы уведомления не перекрывали окно?"
    manual "display stays on" "Is the lid open and the display set to stay on (no screen saver, no dimming)?" \
        "Крышка открыта, а дисплей не гаснет (без заставки и затемнения)?"
    manual "primary display" "If you use an external display, is it the main display (menu bar on it)?" \
        "Если подключен внешний дисплей, он основной (строка меню на нем)?"
    manual "gpu" "On an Intel Mac with two GPUs: is Automatic graphics switching off? (Answer y on Apple silicon.)" \
        "Intel Mac с двумя GPU: автоматическое переключение графики выключено? (На Apple silicon ответьте y.)"
    manual "nothing else" "Will you leave the computer alone and start nothing else until the run ends?" \
        "До конца замеров компьютер никто не трогает и ничего нового не запускает?"
fi

json="$(checks_to_json "$checks" "mode=$mode" "collected_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)" "os=macos")" || exit 1
if [ -n "$out" ]; then
    printf '%s\n' "$json" >"$out"
    msg "hygiene written to $out" "результаты проверки записаны в $out"
else
    printf '%s\n' "$json"
fi
