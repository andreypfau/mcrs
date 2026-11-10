# Shared helpers for the perf-kit shell scripts. Sourced, not run. bash 3.2 compatible.
# shellcheck shell=bash

KIT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KIT_HOME="${MCRS_PERF_HOME:-$HOME/.mcrs-perf}"
JAVA_MIN=25

# A terminal opened before bootstrap.sh installed rustup has no cargo on PATH yet.
if [ -f "${CARGO_HOME:-$HOME/.cargo}/env" ]; then
    # shellcheck source=/dev/null
    . "${CARGO_HOME:-$HOME/.cargo}/env"
fi

say() { printf '%s\n' "$*"; }

# msg "english" "русский" -> prints the line in the language of PERF_KIT_LANG (default en).
msg() {
    if [ "${PERF_KIT_LANG:-en}" = ru ]; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi
}

# ask "question" -> 0 on y/yes/д/да, 1 otherwise (default No).
ask() {
    local reply
    printf '%s [y/N] ' "$1" >&2
    read -r reply || return 1
    case "$reply" in [yY] | [yY][eE][sS] | д | Д | да | Да | ДА | дА) return 0 ;; *) return 1 ;; esac
}

# The checkout that contains this kit, or empty when the kit was copied out of one.
repo_of_kit() {
    local root
    root="$(cd "$KIT_DIR/../.." && pwd)"
    if [ -f "$root/Cargo.toml" ] && [ -f "$root/rust-toolchain.toml" ]; then
        printf '%s\n' "$root"
    fi
}

# Prints the major version of the given java binary, or nothing.
java_major() {
    "$1" -version 2>&1 | awk -F'"' '/version/ { split($2, part, "."); print part[1]; exit }'
}

# Prints the path of a java >= JAVA_MIN: $JAVA, the kit JDK, JAVA_HOME, java_home, PATH.
find_java() {
    local candidate major
    for candidate in \
        "${JAVA:-}" \
        "$KIT_HOME"/jdk/*/Contents/Home/bin/java \
        "${JAVA_HOME:+$JAVA_HOME/bin/java}" \
        "$(/usr/libexec/java_home -v "$JAVA_MIN+" 2>/dev/null)/bin/java" \
        "$(command -v java 2>/dev/null)"; do
        [ -n "$candidate" ] && [ -x "$candidate" ] || continue
        major="$(java_major "$candidate")"
        case "$major" in '' | *[!0-9]*) continue ;; esac
        if [ "$major" -ge "$JAVA_MIN" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

# Free space in GB on the volume holding $1.
free_gb() {
    df -k "$1" 2>/dev/null | awk 'NR == 2 { print int($4 / 1048576) }'
}

# Converts a TSV file of checks (name, value, ok, action) to a JSON document on stdout.
# ok is "true", "false" or empty for null. Extra top-level string fields come as key=value args.
checks_to_json() {
    osascript -l JavaScript - "$@" <<'JXA'
ObjC.import('Foundation');
function run(argv) {
    const path = argv[0];
    const text = $.NSString.stringWithContentsOfFileEncodingError(path, $.NSUTF8StringEncoding, null).js || '';
    const doc = {};
    argv.slice(1).forEach(function (pair) {
        const at = pair.indexOf('=');
        doc[pair.slice(0, at)] = pair.slice(at + 1);
    });
    doc.checks = text.split('\n').filter(function (line) { return line.length > 0; }).map(function (line) {
        const cols = line.split('\t');
        const ok = cols[2] === 'true' ? true : cols[2] === 'false' ? false : null;
        return { name: cols[0], value: cols[1] || '', ok: ok, action: cols[3] || 'none' };
    });
    return JSON.stringify(doc, null, 2);
}
JXA
}
