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
else
    echo "wasm-opt is not installed; the bundle is unoptimised" >&2
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
  #perf { position: fixed; right: 8px; top: 8px; z-index: 9; padding: 8px 10px;
          background: rgba(0,0,0,.72); color: #eee; font: 12px/1.5 monospace;
          border-radius: 6px; white-space: pre; user-select: none; }
  #perf b { color: #8f8; font-weight: normal; }
  #perf a, #perf button { color: #9cf; background: none; border: 1px solid #567;
          border-radius: 4px; font: 11px monospace; padding: 1px 5px; margin: 1px 2px 1px 0;
          cursor: pointer; text-decoration: none; display: inline-block; }
  #perf a.on { border-color: #9cf; color: #fff; }
  #perf hr { border: none; border-top: 1px solid #345; margin: 6px 0; }
</style>
<div id="boot">loading…</div>
<canvas id="mcrs"></canvas>
<div id="perf" hidden></div>
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
<script>
  // A profiling overlay: frame times measured off requestAnimationFrame, plus
  // the two knobs that separate a fill-rate limit from a CPU one - which sky
  // passes are drawn, and how many pixels they cover. Toggle with P.
  const perf = document.getElementById("perf");
  const canvas = document.getElementById("mcrs");
  const url = new URL(location.href);
  const SETS = [
    ["everything", null],
    ["no clouds", "disc,twilight,celestial,stars"],
    ["disc only", "disc"],
    ["clouds only", "clouds"],
  ];
  const SCALES = [["100%", 1], ["71%", 0.71], ["50%", 0.5], ["35%", 0.35]];
  let scale = 1;

  const applyScale = value => {
    scale = value;
    canvas.style.width = value === 1 ? "100%" : Math.round(innerWidth * value) + "px";
    canvas.style.height = value === 1 ? "100%" : Math.round(innerHeight * value) + "px";
    draw();
  };
  const linkFor = (label, list) => {
    const next = new URL(location.href);
    if (list) next.searchParams.set("sky", list); else next.searchParams.delete("sky");
    const on = (url.searchParams.get("sky") || null) === list ? " class=on" : "";
    return `<a href="${next}"${on}>${label}</a>`;
  };

  let frames = [], last = performance.now(), shown = "measuring…";
  const tick = now => {
    frames.push(now - last);
    last = now;
    if (frames.length >= 120) {
      const sorted = frames.slice(3).sort((a, b) => a - b);
      const at = q => sorted[Math.floor(sorted.length * q)];
      shown = `<b>${(1000 / at(0.5)).toFixed(0)} fps</b>  median ${at(0.5).toFixed(1)}ms`
            + `\n  p99 ${at(0.99).toFixed(1)}ms   worst ${sorted[sorted.length - 1].toFixed(1)}ms`;
      frames = [];
      draw();
    }
    requestAnimationFrame(tick);
  };
  function draw() {
    const px = (canvas.width * canvas.height / 1e6).toFixed(1);
    perf.innerHTML = shown
      + `\n${canvas.width}x${canvas.height}  ${px} Mpx  dpr ${devicePixelRatio}`
      + "<hr>passes: " + SETS.map(([l, v]) => linkFor(l, v)).join("")
      + "<br>render scale: "
      + SCALES.map(([l, v]) => `<button data-scale="${v}"${v === scale ? " style=color:#fff" : ""}>${l}</button>`).join("");
  }
  perf.addEventListener("click", e => {
    const value = e.target.dataset && e.target.dataset.scale;
    if (value) applyScale(parseFloat(value));
  });
  addEventListener("keydown", e => {
    if (e.code === "KeyP") { perf.hidden = !perf.hidden; e.preventDefault(); }
  });
  if (url.searchParams.get("perf") !== "0") {
    perf.hidden = false;
    draw();
    requestAnimationFrame(tick);
  }
</script>
"""
html = html.replace("@@GLUE@@", glue).replace("@@WASM@@", wasm)

open(os.environ["OUT"], "w", encoding="utf-8").write(html)
print(os.environ["OUT"], round(len(html) / 1e6, 1), "MB")
PY
