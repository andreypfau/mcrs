#!/usr/bin/env bash
# Builds the client for the browser and inlines the wasm, the wasm-bindgen glue
# and a canvas into one self-contained HTML file.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/target/bundle/mcrs.html}
profile=${PROFILE:-web}
work=$root/target/bundle/bindgen

cargo build --manifest-path "$root/Cargo.toml" \
    -p mcrs_minecraft_client --bin mcrs_minecraft_client \
    --target wasm32-unknown-unknown --profile "$profile"

case $profile in
release) built=release ;;
dev) built=debug ;;
*) built=$profile ;;
esac

# The bindgen format is unstable: the CLI has to be the exact version the
# lockfile compiled against, so refuse rather than emit a broken bundle.
need=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[",]/, "", $3); print $3; exit }' "$root/Cargo.lock")
have=$(wasm-bindgen --version | awk '{print $2}')
if [ "$have" != "$need" ]; then
    echo "wasm-bindgen CLI is $have, the lockfile needs $need:" >&2
    echo "  cargo install -f wasm-bindgen-cli --version $need" >&2
    exit 1
fi

wasm=$root/target/wasm32-unknown-unknown/$built/mcrs_minecraft_client.wasm
rm -rf "$work"
wasm-bindgen --target no-modules --no-typescript --out-dir "$work" --out-name mcrs "$wasm"

if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
        -o "$work/mcrs_bg.opt.wasm" "$work/mcrs_bg.wasm"
    mv "$work/mcrs_bg.opt.wasm" "$work/mcrs_bg.wasm"
fi

mkdir -p "$(dirname "$out")"
GLUE=$work/mcrs.js WASM=$work/mcrs_bg.wasm OUT=$out python3 - <<'PY'
import base64, os

glue = open(os.environ["GLUE"], encoding="utf-8").read()
wasm = base64.b64encode(open(os.environ["WASM"], "rb").read()).decode()

html = """<!doctype html>
<meta charset="utf-8">
<title>mcrs</title>
<style>
  html, body { margin: 0; height: 100%; background: #000; overflow: hidden; }
  canvas { display: block; width: 100%; height: 100%; outline: none; }
  #boot { position: fixed; inset: 0; display: grid; place-items: center;
          color: #bbb; font: 14px monospace; }
</style>
<div id="boot">loading…</div>
<canvas id="mcrs"></canvas>
<script>@@GLUE@@</script>
<script>
  const bytes = Uint8Array.from(atob("@@WASM@@"), c => c.charCodeAt(0));
  wasm_bindgen({ module_or_path: bytes })
    .then(() => document.getElementById("boot").remove())
    .catch(err => {
      // winit unwinds out of its event loop on wasm; that is not a failure.
      if (String(err).includes("Using exceptions for control flow")) {
        document.getElementById("boot").remove();
        return;
      }
      document.getElementById("boot").textContent = err;
      throw err;
    });
</script>
"""
html = html.replace("@@GLUE@@", glue).replace("@@WASM@@", wasm)

open(os.environ["OUT"], "w", encoding="utf-8").write(html)
print(os.environ["OUT"], round(len(html) / 1e6, 1), "MB")
PY
