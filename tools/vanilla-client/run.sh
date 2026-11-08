#!/usr/bin/env bash
# Provisions the vanilla client of the corpus version under target/vanilla-client, verified
# against the package descriptor pinned in release.json, and starts it with an offline name and
# the input agent of this directory. The client joins SERVER at once. Usage:
#   MINECRAFT_EULA=true tools/vanilla-client/run.sh SERVER INPUT_FILE [MAX_FPS]
# INPUT_FILE is the script that the agent plays; see the README. The window stays in the
# background and never takes the keyboard focus. Without MAX_FPS, the window is minimised, and the
# client then draws 10 frames each second.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
# shellcheck source=../vanilla-server/run.sh
source "$root/tools/vanilla-server/run.sh"

if [ "${MINECRAFT_EULA:-}" != true ]; then
    echo "The Minecraft EULA is not accepted in this environment." >&2
    echo "Read https://aka.ms/MinecraftEULA; setting MINECRAFT_EULA=true states the acceptance." >&2
    exit 3
fi
[ $# -ge 2 ] || { echo "usage: $0 SERVER INPUT_FILE [MAX_FPS]" >&2; exit 2; }
server=$1 input=$2 max_fps=${3:-260} minimize=$([ $# -ge 3 ] && echo false || echo true)

release="$root/crates/mcrs_minecraft_client_jar/src/release.json"
id=$(jq -r '.id' "$release")
work="$root/target/vanilla-client/$id"
libraries="$root/target/vanilla-client/libraries"
assets="$root/target/vanilla-client/assets"
mkdir -p "$work" "$libraries" "$assets/indexes" "$assets/objects"

package="$work/$id.json"
fetch_verified "$package" "$(jq -r '.json.url' "$release")" "$(jq -r '.json.sha1' "$release")" || exit 2
fetch_verified "$work/client.jar" \
    "$(jq -r '.downloads.client.url' "$package")" \
    "$(jq -r '.downloads.client.sha1' "$package")" \
    "$(jq -r '.downloads.client.size' "$package")" || exit 2

# Each rule of the package names one operating system; a library without a rule is for all.
classpath=
while IFS=$'\t' read -r path url sha1 size; do
    mkdir -p "$(dirname "$libraries/$path")"
    fetch_verified "$libraries/$path" "$url" "$sha1" "$size" || exit 2
    classpath="$classpath$libraries/$path:"
done < <(jq -r '.libraries[]
    | select(.rules == null or any(.rules[]; .action == "allow" and .os.name == "osx"))
    | .downloads.artifact | [.path, .url, .sha1, (.size | tostring)] | @tsv' "$package")
classpath="$classpath$work/client.jar"

index_id=$(jq -r '.assetIndex.id' "$package")
index="$assets/indexes/$index_id.json"
fetch_verified "$index" "$(jq -r '.assetIndex.url' "$package")" "$(jq -r '.assetIndex.sha1' "$package")" || exit 2
jq -r '.objects[] | .hash' "$index" | sort -u | while read -r hash; do
    object="$assets/objects/${hash:0:2}/$hash"
    [ -f "$object" ] || echo "$hash"
done | xargs -P 16 -n 1 sh -c '
    hash=$1 dir="$0/objects/$(echo "$1" | cut -c1-2)"
    mkdir -p "$dir"
    curl -fsSL --retry 3 -o "$dir/$hash" "https://resources.download.minecraft.net/${dir##*/}/$hash" &&
        [ "$(shasum -a 1 "$dir/$hash" | cut -d" " -f1)" = "$hash" ] ||
        { rm -f "$dir/$hash"; echo "bad asset $hash" >&2; exit 255; }
' "$assets"

agent="$root/target/vanilla-client/input-agent.jar"
classes="$root/target/vanilla-client/agent-classes"
rm -rf "$classes"
mkdir -p "$classes"
javac --release 25 -cp "$classpath" -d "$classes" "$here"/agent/mcrs/input/*.java
printf 'Premain-Class: mcrs.input.Agent\n' >"$classes/manifest.txt"
jar --create --file "$agent" --manifest "$classes/manifest.txt" -C "$classes" mcrs

game="$work/game"
mkdir -p "$game"
# The options keep the client from pausing and from showing the first-start screens, and set
# the frame rate of C2. Vsync off lets the frame rate reach maxFps.
cat >"$game/options.txt" <<EOF
version:5127
onboardAccessibility:false
skipMultiplayerWarning:true
joinedFirstServer:true
pauseOnLostFocus:false
tutorialStep:none
narrator:0
enableVsync:false
maxFps:$max_fps
inactivityFpsLimit:minimized
renderDistance:2
simulationDistance:5
soundCategory_master:0.0
fullscreen:false
EOF

cd "$game"
# SDL takes these hints from the environment: an accessory application that never activates.
SDL_MAC_BACKGROUND_APP=1 SDL_WINDOW_ACTIVATE_WHEN_SHOWN=0 exec java \
    -XstartOnFirstThread -Xmx2G -XX:+UseZGC \
    --enable-native-access=ALL-UNNAMED --add-exports java.base/jdk.internal.misc=ALL-UNNAMED \
    -Dorg.lwjgl.system.SharedLibraryExtractPath="$work/natives/lwjgl" \
    -Djna.tmpdir="$work/natives/jna" -Dio.netty.native.workdir="$work/natives/netty" \
    -Dmcrs.minimize="$minimize" -javaagent:"$agent"="$input" \
    -cp "$classpath" "$(jq -r '.mainClass' "$package")" \
    --username Recorder --version "$id" --gameDir "$game" --assetsDir "$assets" \
    --assetIndex "$index_id" --uuid 00000000-0000-3000-8000-000000000000 --accessToken 0 \
    --clientId 0 --xuid 0 --versionType snapshot --width 427 --height 240 \
    --quickPlayMultiplayer "$server"
