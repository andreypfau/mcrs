#!/bin/bash
# The one entry point: records the machine, prepares it, builds, runs the benchmarks and zips the results.
# Usage: run.sh [--full] [--label NAME] [--lang en|ru] [--only mob,client,server,worldgen] [--yes] [--accept-eula]
set -u
# shellcheck source=SCRIPTDIR/common.sh
. "$(dirname "$0")/common.sh"

clean_label() { printf '%s' "$1" | tr -cs 'A-Za-z0-9_-' '-' | sed 's/^-*//; s/-*$//'; }

# ru when the shell locale or the first macOS preferred language is Russian, en otherwise.
system_lang() {
    local value
    for value in "${LC_ALL:-}" "${LC_MESSAGES:-}" "${LANG:-}" "$(defaults read -g AppleLanguages 2>/dev/null | sed -n '2s/[ "]//gp')"; do
        case "$value" in ru*) printf 'ru\n'; return ;; esac
    done
    printf 'en\n'
}

profile=quick
label=""
lang=""
only="mob,client,server,worldgen"
yes=0
accept_eula=0
while [ $# -gt 0 ]; do
    case "$1" in
        --full) profile=full; shift ;;
        --yes) yes=1; shift ;;
        --accept-eula) accept_eula=1; shift ;;
        --label) label="$2"; shift 2 ;;
        --only)
            for section in $(printf '%s' "${2:-}" | tr ',' ' '); do
                case "$section" in mob | client | server | worldgen) ;; *) say "--only takes a comma list of mob, client, server, worldgen"; exit 2 ;; esac
            done
            [ -n "${2:-}" ] || { say "--only takes a comma list of mob, client, server, worldgen"; exit 2; }
            only="$2"; shift 2 ;;
        --lang)
            case "${2:-}" in en | ru) lang="$2"; shift 2 ;; *) say "--lang takes en or ru"; exit 2 ;; esac ;;
        *) say "unknown argument: $1"; exit 2 ;;
    esac
done

if [ "$yes" = 0 ] && [ ! -t 0 ]; then
    PERF_KIT_LANG="${lang:-$(system_lang)}"
    msg "Standard input is not a terminal, so run.sh cannot ask questions. Add --yes (with --label and, for the mob section, --accept-eula) to run unattended." \
        "Стандартный ввод не терминал, поэтому run.sh не может задавать вопросы. Добавьте --yes (вместе с --label и, для секции мобов, --accept-eula), чтобы запуск шел без вопросов."
    exit 2
fi

if [ -z "$lang" ] && [ "$yes" = 1 ]; then
    lang="$(system_lang)"
fi
if [ -z "$lang" ]; then
    if [ "$(system_lang)" = ru ]; then default=2; else default=1; fi
    while [ -z "$lang" ]; do
        printf 'Language / Язык: [1] English  [2] Русский (Enter: %s) ' "$default" >&2
        read -r choice || exit 1
        case "${choice:-$default}" in 1) lang=en ;; 2) lang=ru ;; esac
    done
fi
export PERF_KIT_LANG="$lang"

repo="$(repo_of_kit)"
[ -n "$repo" ] || { msg "run.sh must stay inside the mcrs checkout; run bootstrap.sh first." "run.sh должен лежать внутри клона mcrs; сначала запустите bootstrap.sh."; exit 1; }

msg "mcrs benchmark kit, profile: $profile" "Бенчмарки mcrs, профиль: $profile"
if [ "$profile" = full ]; then
    msg "Expected time: about 1.5-3 hours, depending on the machine." "Займет примерно 1,5-3 часа, смотря какая машина."
else
    msg "Expected time: about 30-60 minutes, depending on the machine." "Займет примерно 30-60 минут, смотря какая машина."
fi
msg "While it runs, do not touch the computer. The game window must stay visible, on top and not minimised." "Пока идут замеры, не трогайте компьютер. Окно игры должно быть видно, поверх остальных окон и не свернуто."
say ""

label="$(clean_label "$label")"
if [ "$yes" = 1 ] && [ -z "$label" ]; then
    msg "--yes needs --label NAME, since the run cannot ask for it." "С --yes нужен --label NAME, так как запуск не может его спросить."
    exit 2
fi
while [ -z "$label" ]; do
    printf '%s ' "$(msg "Machine label (for example m2-air or ryzen-3060):" "Метка машины (например, m2-air или ryzen-3060):")" >&2
    read -r label || exit 1
    label="$(clean_label "$label")"
done

sections=()
case ",$only," in
    *,mob,*)
        msg "The mob benchmark runs the Minecraft server, which needs the Minecraft EULA: https://aka.ms/MinecraftEULA" \
            "Бенчмарк мобов запускает сервер Minecraft, а для него нужно принять Minecraft EULA: https://aka.ms/MinecraftEULA"
        if [ "$accept_eula" = 1 ] || { [ "$yes" = 0 ] && ask "$(msg "Do you accept the Minecraft EULA?" "Принять Minecraft EULA?")"; }; then
            sections=(--accept-eula)
        else
            msg "Running without the mob section." "Запуск без секции мобов."
            only="$(printf '%s' ",$only," | sed 's/,mob,/,/; s/^,//; s/,$//')"
            [ -n "$only" ] || exit 0
        fi
        ;;
esac
sections+=(--only "$only")

name="$label-macos-$(date -u +%Y%m%dT%H%M%SZ)"
results="$KIT_DIR/results"
dir="$results/$name"
mkdir -p "$dir"

caffeinate -dims -w $$ &
awake=$!
# shellcheck disable=SC2329 # called by the EXIT trap
cleanup() { kill "$awake" 2>/dev/null; }
trap cleanup EXIT
trap 'say ""; msg "Interrupted; partial results stay in $dir" "Прервано; частичные результаты остались в $dir"; exit 130' INT TERM

"$KIT_DIR/sysinfo.sh" --label "$label" --out "$dir/system.json" || exit 1
prepare_mode=()
[ "$yes" = 1 ] && prepare_mode=(--check-only)
"$KIT_DIR/prepare.sh" ${prepare_mode[@]+"${prepare_mode[@]}"} --out "$dir/hygiene.json" || exit 1
"$KIT_DIR/bootstrap.sh" --build-only || { msg "The build failed; run $KIT_DIR/bootstrap.sh and try again." "Сборка не удалась; запустите $KIT_DIR/bootstrap.sh и попробуйте снова."; exit 1; }

java="$(find_java)" || { msg "Java $JAVA_MIN not found; run $KIT_DIR/bootstrap.sh first." "Java $JAVA_MIN не найдена; сначала запустите $KIT_DIR/bootstrap.sh."; exit 1; }

# The client sizes a --window in physical pixels (scale factor 1), with no title bar, centred and
# on top, so a candidate is compared with the full pixel size of the main display.
size="$(osascript -l JavaScript -e 'ObjC.import("AppKit"); const s = $.NSScreen.screens.objectAtIndex(0); const f = s.frame; const k = s.backingScaleFactor; Math.floor(f.size.width * k) + " " + Math.floor(f.size.height * k)')"
window=""
for candidate in 1920x1080 1600x900 1280x720 960x540; do
    if [ "${candidate%x*}" -le "${size% *}" ] && [ "${candidate#*x}" -le "${size#* }" ]; then
        window="$candidate"
        break
    fi
done
window="${window:-960x540}"

heap=$(($(sysctl -n hw.memsize) / 1073741824 / 2))
[ "$heap" -gt 16 ] && heap=16
msg "Window $window, Java heap ${heap} GB, results in $dir" "Окно $window, куча Java ${heap} ГБ, результаты в $dir"

"$repo/target/release/perf-kit" run --out "$dir" --profile "$profile" --repo "$repo" \
    --window "$window" --heap "${heap}g" --java "$java" "${sections[@]}" 2>&1 | tee "$dir/run.log"
status=${PIPESTATUS[0]}
find "$dir" -type f -exec env LC_ALL=C sed -i '' "s|$HOME|~|g" {} +

"$KIT_DIR/prepare.sh" --check-only --out "$dir/hygiene-after.json"

(cd "$results" && ditto -c -k --keepParent "$name" "$name.zip") || { msg "Could not create the zip; send the folder $dir instead." "Не получилось создать zip; отправьте вместо него папку $dir."; exit 1; }
say ""
if [ "$status" -ne 0 ]; then msg "Some sections failed (exit $status); the zip still holds everything that was measured." "Часть секций упала (код $status); в zip все равно есть все, что успели замерить."; fi
msg "Done. Send this file: $results/$name.zip" "Готово. Отправьте этот файл: $results/$name.zip"
msg "It holds system.json, hygiene.json, hygiene-after.json, report.json, raw/ and run.log." "В нем system.json, hygiene.json, hygiene-after.json, report.json, raw/ и run.log."
exit "$status"
