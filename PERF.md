# Performance

Every number carries its scenario. Medians are over a window, never a mean and never one frame:
frame time over the last 4096 frames, CPU stages and GPU passes over the last 256. The first ten
seconds after launch are discarded. Run-to-run spread on the settled figures, from two runs of
each scenario: 0.1 ms on the wall frame, 0.2 ms on the main-world stage, 0.05 ms on the other
stages and on the GPU passes. A delta inside that is noise.

## Reference machine

Apple M4 Max, 40 GPU cores, macOS, Metal. Displays: built-in 3456x2234 at 120 Hz (scale 2) and a
3840x2160 external at 60 Hz (scale 2), which is the primary and where the fullscreen window lands.
Release profile, nightly toolchain. Bevy 0.19.1, wgpu 29.

## How to take a reading

```
MCRS_STATS=5 ./target/release/mcrs_minecraft_client "<world folder>"
```

writes the whole F3 line to the log every five seconds with the overlay hidden. Knobs that
shape a scenario: `MCRS_SERVER=127.0.0.1:9` (no world, the floor), `MCRS_RESOLUTION=2560x1440`
(windowed surface of that size), `MCRS_LATENCY=<frames>` (swapchain depth), `MCRS_SKY=`
(no sky draws), `MCRS_FULLSCREEN=0`. Tracy: build with `--features telemetry-tracy`, capture with
`tracy-capture -o trace.tracy -f`, aggregate with `tracy-csvexport`. Apple's Metal HUD
(`MTL_HUD_ENABLED=1`) shows the presented rate, the GPU time per frame and whether the window is
composited or direct to display.

## What one line reads

```
290 fps (3.447 ms median, 17.628 p99, 31.10 max over 4096 frames)
CPU: main 0.674 ms, extract 0.175, prepare 2.123 (acquire 2.004), render 0.345, cleanup 0.012
Draws: 7 terrain, 2 sky | Upload: 0 KB of 4096 KB | GPU cull: 0.142 ms | GPU terrain: 1.085 ms
3840x2160 physical, scale 2.00
Sections: 6170/7200 in 729 columns | Mesh: 0 queued, 0 in flight, 0 uploads waiting
Arena: 7.1% quads, 2.7% models, 8.0% faces | Tris: 283192
```

`main` is the main-world schedule, `extract` the copy into the render world, `prepare` everything
from extract commands to the end of the prepare set, `acquire` the swapchain acquire inside it,
`render` the graph run plus submit and present, `cleanup` the rest. Without pipelined rendering
these run back to back and add up to the frame; what is left over is winit and the OS.

**Engine time** is the wall frame minus the acquire wait. On macOS the acquire blocks until the
display recycles a drawable whatever the present mode says, so the wall frame measures the
display once the engine is faster than it. See DECISIONS.md.

## Scenarios

- **floor**: no server reachable, no world, the overworld sky drawn, overlay hidden, fullscreen.
- **base**: save `two`, spawned by the integrated server at 0/100/0 facing south, overworld,
  evening. The client asks for a view distance of 8 columns; the server sends a radius of 13,
  which is 729 columns and 6170 resident sections once settled. Overlay hidden, fullscreen,
  vsync off. Screenshot: `docs/perf/m1-base-2560x1440.png` (the 3840x2160 frame scaled to 1440p).

## Native, 3840x2160 fullscreen, vsync off, overlay hidden

| figure | floor | base |
|---|---|---|
| wall frame median | 3.3 to 3.5 ms | 3.3 to 3.4 ms |
| wall frame p99 / max | 18 / 40 to 56 ms | 18 / 26 to 31 ms |
| acquire wait (median of 256) | 1.7 to 2.8 ms | 1.6 to 2.2 ms |
| **engine CPU** (wall minus acquire) | **0.8 to 1.1 ms** | **1.3 to 1.5 ms** |
| main world | 0.37 to 0.60 ms | 0.63 to 0.73 ms |
| extract | 0.11 to 0.18 ms | 0.17 to 0.18 ms |
| prepare without acquire | 0.11 to 0.18 ms | 0.13 to 0.15 ms |
| render (encode + submit + present) | 0.21 to 0.32 ms | 0.33 to 0.39 ms |
| cleanup | 0.01 ms | 0.01 ms |
| GPU cull | none | 0.14 to 0.15 ms |
| GPU terrain pass | 0.63 ms (clouds only) | 1.15 ms |
| GPU sky pass | 0.27 ms (day) | 0.11 ms (evening) |
| GPU whole frame (Metal HUD) | 0.91 ms | not read |
| draw calls | 1 terrain + 4 sky | 7 terrain + 2 sky |
| drawn triangles | 0 | 283 192 |
| resident sections / columns | 0 | 6170 / 729 |
| upload bytes per frame, settled | 0 | 0 |
| meshing queued / in flight, settled | 0 / 0 | 0 / 0 |
| arena quads / models / faces | 0 | 7.1% / 2.7% / 8.0% |

The floor at 2560x1440 windowed reads 0.48 ms GPU per frame on the Metal HUD and 0.31 ms in the
terrain pass, against 0.91 and 0.63 at 3840x2160: the sky and cloud fill scale with pixels.

Where the steady-state p99 comes from (Tracy, base, after 32 s): the acquire is bimodal, median
0.01 ms and above 8 ms in 917 of 8169 calls, which is the 60 Hz display taking a drawable back
about once in five frames. The main-world schedule never exceeded 8 ms (median 0.51, p99 4.5,
max 5.3 ms); that 4.5 ms tail is the engine's own hitch to chase.

Per-frame main-world systems that show at all (Tracy means): `cave_cull` 60 µs, `ui_layout`
86 µs, `ui_stack` 20 µs, `ui_focus` 15 µs, `update_clipping` 11 µs, `stream::advance` 6 µs
settled. Everything else is dispatch: 346 systems in the main world (PostUpdate 115, PreUpdate
56, Update 46, StateTransition 16, Last 14) and 195 in the render world (Render 96,
ExtractSchedule 70, Core3d 17).

Each submit carries four or five command buffers and takes about 70 µs.

## Budget

The starting split from the brief, next to what was measured. The engine figure is what the
1.0 ms goal binds on; the wall frame on this machine is the display's.

| stage | budget | floor | base |
|---|---|---|---|
| engine floor (empty frame CPU) | measured | 0.8 ms | |
| main world | 0.15 ms | 0.37 ms | 0.63 ms |
| extract | 0.10 ms | 0.11 ms | 0.17 ms |
| prepare + queue | 0.15 ms | 0.11 ms | 0.15 ms |
| encode + submit | 0.10 ms | 0.21 ms | 0.33 ms |
| GPU cull | 0.05 ms | | 0.14 ms |
| GPU terrain | 0.65 ms | | 1.1 ms at 8.3 Mpx |
| GPU sky pass | 0.10 ms | 0.27 ms at 8.3 Mpx | 0.11 ms |
| GPU clouds (in the terrain pass) | | 0.63 ms at 8.3 Mpx | |

## Web

Not measurable yet. `scripts/build-web.sh` produces a 40 MB single-file bundle that Chrome runs
over WebGPU (`http://localhost:<port>/mcrs.html?stats=3`) and it draws the sky, but the start-up
blocks the page's main thread: on the first load 87 s passed between the adapter log and the
first asset log, the stats line was written for four frames reading zero everywhere, and after a
reload the page answered nothing for more than eight minutes. The render world's start-up
schedule also logged itself once per frame in those four frames, which on native runs once.
Until the browser build reaches a steady frame there is no floor to state for it; the numbers
that will count there are the CPU stages and the GPU pass timestamps, never the FPS line.

## Findings not yet acted on

- The client is built without Bevy's `multi_threaded` feature: the ECS runs on the
  single-threaded executor and rendering is not pipelined, so every stage above is serial on one
  thread. The cheapest win in the CPU frame is likely turning it on and measuring.
- UI layout and its siblings cost about 130 µs per frame with nothing visible.
- The client requests a view distance of 8 but the server sends 13; the baseline is taken at 13.
- The web build's start-up blocks the page for minutes and re-runs the render start-up schedule.
