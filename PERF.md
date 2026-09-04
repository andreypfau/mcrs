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
(no sky draws), `MCRS_LOOK=<yaw>,<pitch>` (aim the camera, kept through the join teleport),
`MCRS_GPU_HOT=<workgroups>` (hold the GPU at speed, see the GPU frame section), `MCRS_FULLSCREEN=0`.
Fullscreen lands on the primary monitor; a sized window is centred on it and stays on top, because
a covered window is not presented and a frame that gets no swapchain texture is never drawn.
Tracy: build with `--features telemetry-tracy`, capture with
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
| GPU cull | 0.05 ms | | 0.14 ms at the display's clock, 0.058 at full clock |
| GPU terrain | 0.65 ms | | 1.1 ms at 8.3 Mpx at the display's clock; 0.25 ms at 3.7 Mpx at full clock |
| GPU sky pass | 0.10 ms | 0.27 ms at 8.3 Mpx | 0.11 ms at the display's clock; 0.016 at full clock |
| GPU clouds (in the terrain pass) | | 0.63 ms at 8.3 Mpx | 0.08 ms for a full screen at 3.7 Mpx at full clock |

## Stalls

Native, 3840x2160 fullscreen, vsync off, overlay hidden, save `two`. `MCRS_HOT=1` keeps the
performance cluster clocked (see DECISIONS.md); the cold column is the same build without it.
Engine time is per frame; p99 and max are over the last 4096 frames.

| scenario | engine cold: median / p99 / max | engine hot: median / p99 / max |
|---|---|---|
| floor (no world) | 1.37 / 4.0 / 9.2 ms | 0.60 / 1.12 / 1.36 ms |
| base, static | 1.45 / 3.5 / 4.8 ms | 0.70 / 1.31 / 1.66 ms |
| flight, vanilla sprint (`MCRS_FLY=0.05`) | 1.53 / 3.7 / 5.5 ms | 0.76 / 1.47 / 1.72 ms |
| flight, maximum wheel speed (`MCRS_FLY=0.2`) | | 0.89 / 1.74 / 2.11 ms |
| base, vsync on | 3.90 / 5.6 / 42 ms | 0.82 / 1.73 / 51 ms |

Hot stage medians in sprint flight: main 0.38 ms, extract 0.09, prepare without acquire 0.08,
render 0.20, cleanup 0.01; p99 of the main world 0.79 ms and of render 0.41 ms.

What was found and what it cost, in the order it was taken:

- The sprite atlas swap was the render-world stall: 54 whole-atlas re-uploads in 40 s of
  flight, 2.4 ms mean and 4.8 ms max, all of it in `write_texture` (220 calls, 0.58 ms mean)
  for under a megabyte. Bakes now write only new layers: 8 to 90 KB each, and the render
  stage max in flight went from 6.6 ms to 2.0 ms.
- Column decoding on the main thread cost 1.3 to 1.4 ms on arrival frames (command
  application after `receive_packets`) with `stream::advance` adding 0.5 to 0.7 ms. Decoding
  moved to the compute pool: main-world p99 in flight 2.44 to 2.01 ms, max 3.5 to 2.9 ms (cold).
- Geometry uploads through a staging belt: render stage p99 at maximum speed 0.94 to 0.49 ms.
- Everything left in the tail was uniform: in a slow frame every system ran two to four times
  slower. A vsync-on run, where the thread sleeps every frame, put the engine median at 3.9 ms
  for the same work, and a spinning helper thread brought the floor from 1.37 / 4.0 / 9.2 ms to
  0.60 / 1.12 / 1.36 ms. That is the CPU's clock ramp after the acquire sleep. A higher QoS
  class and a deeper swapchain (Metal allows three drawables at most) changed nothing.
- Pipeline warm-up needs no work yet: every terrain variant, wireframe included, is queued when
  the view appears, and the sky pipelines are per dimension.

Screenshot: `docs/perf/m2-flight-2560x1440.png`, taken in sprint flight with `MCRS_HOT=1` (the
3840x2160 frame scaled to 1440p); the 34 ms max in its line is the screenshot readback itself,
which waits on the GPU.

Gate: the engine p99 is under 2.0 ms in every scenario hot; the max still sits about 1 ms over
the median rather than 0.5 ms. Cold, neither holds, and cannot until the sleep is dealt with.

Open: the client keeps every column the server ever sent (5932 resident at maximum speed, 0
evicted, 44k sections, GPU terrain 4.9 ms) although the server emits chunk-forget packets;
that is streaming work still to come and it inflates every GPU number taken in flight.
Bevy's window screenshot comes back black on some frames on this build and the previous one
alike; captures are retried until one is lit.

## CPU frame

Native, 3840x2160 fullscreen, vsync off, overlay hidden, `MCRS_HOT=1`. Engine time median /
p99 / max over 4096 frames, stage medians in ms.

| build | floor engine | floor main / extract / render | base engine | base main / extract / render |
|---|---|---|---|---|
| after the stalls pass | 0.60 / 1.12 / 1.36 | 0.285 / 0.089 / 0.151 | 0.70 / 1.31 / 1.66 | 0.336 / 0.090 / 0.194 |
| UI gated | 0.51 / 0.92 / 1.07 | 0.199 / 0.073 / 0.155 | | |
| one frame system, transitions gated, no lights, cave and screenshot polls | 0.44 / 0.83 / 0.99 | 0.171 / 0.055 / 0.119 | 0.53 / 1.0 / 1.25 | 0.216 / 0.058 / 0.164 |

Per zone, hot floor, microseconds a frame (Tracy, before and after this pass): main world 248
to 208 (PostUpdate 99 to 80, state transitions 20 to 6), extract 74 to 54, encode 198 to 150
(Core3d 123 to 90, of which the frame system is 51 and Bevy's upscaling blit 16), submit 52 to
38 with two command buffers instead of six.

In flight, hot, after this pass: sprint 0.68 / 1.28 / 1.60 ms with 16k resident sections
(main 0.29 ms, GPU world 1.22), maximum speed 0.83 / 1.77 / 2.63 ms with 55k resident sections
(main 0.35 ms, GPU cull 0.38, GPU world 4.3). The engine stays under 1.0 ms but is not flat:
both the main world and the GPU grow with residency, and residency grows without bound because
nothing is evicted in flight (the open streaming item), so flatness cannot be judged until
columns behind the player leave.

What is left, hot floor: main world 208 µs across 300-odd systems where the named work is a
few µs (PreUpdate 57, PostUpdate 80, Update 41), extract 54 µs across 70 systems, the frame
system 51 µs (one compute and one render pass; opening a pass on Metal is ~15 µs), upscaling
16 µs, submit 38 µs, and `bevy_asset` at 22 µs a frame across 184 per-asset-type systems.

## GPU frame

Every GPU pass figure above this section was taken at whatever clock the GPU's governor chose,
and it chooses the lowest clock that still meets the display's deadline. The same cull dispatch
over the same 6170 sections read 0.248 ms in the composited 1440p window at 60 Hz, 0.114 ms
fullscreen on the 120 Hz built-in display, and 0.058 ms once the GPU was saturated. So the
numbers here are taken with `MCRS_GPU_HOT=16384`: a compute pass of arithmetic ahead of the
frame's own passes, written against the draw args so the driver orders it between one frame's
world pass and the next frame's cull instead of overlapping either. The cull read 0.163 / 0.108
/ 0.058 / 0.058 ms with 4096 / 8192 / 16384 / 32768 workgroups, and the heater itself read
12.4 / 13.0 / 12.7 / 25.4 ms: below 16384 the governor slows the whole frame to fit 16.6 ms,
at 16384 it cannot and the passes read their own cost. Two runs of the same scenario agree to
0.001 ms on the cull and 0.002 ms on the world pass at that clock.

Scenario unless stated: save `two`, spawn 0/100/0 facing south, overworld evening, 6170 resident
sections, 2560x1440 window, `MCRS_HOT=1`, `MCRS_GPU_HOT=16384`, overlay hidden. Medians of 256
frames. The world pass holds the sky, the opaque terrain, the clouds and the blended terrain.

| view | GPU cull | GPU world | drawn triangles |
|---|---|---|---|
| base | 0.058 ms | 0.305 ms | 226 784 |
| base, no sky draws (`MCRS_SKY=`) | 0.058 | 0.288 | |
| base, no clouds | 0.058 | 0.289 | |
| base, solid greedy stream only | 0.012 | 0.117 | 67 840 |
| base, opaque streams only | 0.039 | 0.273 | 222 526 |
| base, terrain viewport at half size (`MCRS_RASTER=0.5`) | 0.058 | 0.248 | |
| base, terrain viewport at a twentieth | 0.058 | 0.229 | |
| camera straight up, sky and clouds only | 0.047 | 0.080 | 0 |
| camera straight down, full-screen ground | 0.061 | 0.216 | 52 922 |
| base at 3840x2160 fullscreen | 0.059 | 0.460 | 226 784 |

Read together: the pass floor with the sky and a full screen of clouds is 0.08 ms, of which
the sky draws are 0.016 in the base view; the terrain is 0.25 ms, about 0.08 of it fill (the
half-size viewport saves 0.056) and 0.17 vertex work for 141 000 quad slots; the blended
streams are 0.019 of the cull and 0.031 of the world pass. Against the brief's split the cull
is at its 0.05 ms line and the terrain is well inside 0.65. Going to 3840x2160 adds 0.155 ms,
which is the fill and the tile traffic scaling with pixels.

The drawn-triangle stat used to count every slot an ordered (blended) draw spanned, holes
included: 283 192 in this view, where 226 784 quads' worth actually survived the cull. It now
counts survivors.

**Residency.** Flying south at maximum wheel speed (`MCRS_FLY=0.2`) for 50 s, nothing evicted:

| build | sections resident | GPU cull | GPU world | drawn triangles |
|---|---|---|---|---|
| before, all streams | 50 403 | 0.076 ms | 0.743 ms | 1 569 034 (holes counted) |
| before, opaque streams only | 49 366 | 0.051 | 0.098 | 24 036 |
| after, all streams | 49 491 | 0.076 | 0.144 | 26 652 |

The blended draw keeps every group in the slot the mesher gave it and set its instance count
to the last survivor's end, so the vertex shader walked every resident blended quad up to it:
the flight is over ocean, and 0.65 ms of the world pass was degenerate water quads behind the
camera. The cull now also records the first surviving slot, a finalize dispatch turns the end
into a count from there, and the vertex shader adds that start, so the draw covers the range
between the first and last survivor. The static view is unchanged (0.305 ms both ways: its
survivors are scattered over the list) and the picture is the same, since the order the range
is drawn in is the order the list held. What is left in flight over the base view is the cull
growing 0.058 to 0.076 ms with 8x the resident groups, and the hole-writing in the ordered cull,
which is still per resident blended group. Survivors scattered across a long-resident list would
still pay for the holes between them; a compaction that preserves the order (a prefix sum over
batches) is the general fix and waits for a scenario that shows the cost.

Screenshot: `docs/perf/m4-base-2560x1440.png`, the base view at 2560x1440 as drawn.

Gate: the GPU frame is 0.36 ms in the base view and 0.22 ms in flight at 50 000 resident
sections, and the world pass now follows what is drawn. The empty-frame GPU floor cannot be
read with the heater (with no terrain draw nothing ties the world pass to the draw args and the
two overlap); the camera-up view stands in for it at 0.080 ms.

## Streaming

Flying south at maximum wheel speed (`MCRS_FLY=0.2`, about 75 blocks a second) into terrain the
server generates as it goes: save `two`, 2560x1440 window, `MCRS_HOT=1`, overlay hidden, stats
every five seconds. Engine time median / p99 / max over 4096 frames, all of them in flight (the
run is 85 s so the window holds nothing of the start), and the stage figures from the same line.
Run-to-run spread over three settled flights of the same build: 0.04 ms on the median, 0.1 on
the p99, 0.2 on the max.

| build | engine | main median / p99 / max | render median / p99 / max | resident | upload per frame |
|---|---|---|---|---|---|
| nothing evicted | 1.28 / 1.98 / (start in window) | 0.63 / 1.21 / | 0.37 / 0.54 / | 6703 columns, 50 640 sections | 4096 KB, capped |
| columns evicted | 1.07 / 1.65 / (start in window) | 0.43 / 0.95 / | 0.33 / 0.48 / | 1110 columns, 4763 sections | 317 KB |
| states listed at decode | 1.08 / 1.49 / 1.83 | 0.45 / 0.81 / 1.12 | 0.34 / 0.46 / 0.74 | 1501 columns, 4863 sections | 437 KB |
| quantiles selected, not sorted | 1.09 / 1.45 / 2.03 | 0.45 / 0.78 / 0.90 | 0.35 / 0.46 / 0.52 | 1492 columns, 4730 sections | 399 KB |
| same build, two more runs | 1.12 / 1.56 / 1.87 and 0.97 / 1.39 / 1.90 | | | | |

What was found, in order, with Tracy means over the settled part of a 70 s flight:

- **The server never took a column back.** It computed the unload set and wrote the packets,
  but aimed them at the dimension-local player entity, and the host stamps a session onto a
  single-player packet by the player's host anchor and drops one it cannot stamp. Chunk loads
  already used the anchor. With unloads arriving, residency in flight holds at about 1100 to
  1500 columns instead of growing without bound (44 000 to 86 000 sections evicted per run),
  the main-world median fell 0.63 to 0.43 ms, and the group-table re-upload that had saturated
  the 4 MB belt every frame fell to 300 to 450 KB.
- **`stream::advance`** was 115 µs mean, 547 p99, 752 max, and `stream adopt` (once per
  arriving batch, every fourth frame) 248 mean, 461 p99, 549 max: it walked every block of
  every arriving column looking for states the catalog had not baked, 40 000 reads a column.
  The decode task now lists a section's distinct states from its palette (a direct-palette
  section is scanned on the compute pool), and adopt reads that: 107 mean, 240 p99, 281 max;
  `stream::advance` 72 mean, 377 p99, 488 max.
- **The stats collection sorted the 4096-frame window seven times** to answer three quantiles,
  500 µs in the frame that wrote a line, and the overlay would pay it ten times a second when
  shown. The quantiles are selected instead: 113 µs mean, 132 max.

What is left in a flight frame, Tracy means: main world 424 µs (PostUpdate 167 of which
`cave_cull` 51, Update 137 of which `stream::advance` 72, PreUpdate 74, extract 97), render
world 321 (`draw_frame` encoding 132, submit 70). In `stream::advance`: `stream flush` 25 mean
and 153 p99, because a frame with an arrival rebuilds and re-uploads the whole group table
(about 40 000 records at this residency); `stream adopt` 107 mean, which is the store clone,
two set scans over the resident columns and the enqueue of the neighbours; `stream place` 12
mean and 352 max for a frame that lands 32 sections at once.

**Admission bounds.** `SECTIONS_PER_FRAME` is 32 and `SECTIONS_IN_FLIGHT` 128; at maximum
speed the compute pool returns about 32 meshes a frame at 60 Hz and never holds more than 32
in flight, so the in-flight bound is slack and the per-frame bound is what the pool delivers.
Placing 32 sections costs 350 µs at worst and 12 on average, which fits. The 4 MB upload budget
is a cap, not a target: a settled flight uploads 300 to 450 KB a frame, and a full 4 MB frame
costs about 130 µs of staging memcpy. Neither bound was changed; the numbers that justify
leaving them are the ones above.

At maximum speed the store holds about 12 000 sections of which 5000 are meshed: a section
is not meshed until all eight neighbouring columns have arrived, and columns are taken back at
radius 13 before the mesher reaches them, which reads as terrain filling in behind the horizon.
That is throughput, not a frame cost.

Screenshot: `docs/perf/m5-flight-2560x1440.png`, 82 s into the flight, over ocean.

Gate: no settled flight frame over 2.0 ms in two of three runs (max 1.83, 1.87, 1.90); the
third run's single worst frame read 2.03, which is inside the 0.2 ms spread on a maximum.
Breaking and placing blocks cannot be measured: the client has no block-edit path, no handler
for block-update packets and no input for it.

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
- The ordered cull still writes a hole for every resident blended group, and survivors scattered
  over a long-resident list still pay for the holes between them.
- The web build's start-up blocks the page for minutes and re-runs the render start-up schedule.
- A frame with an arrival rebuilds and re-uploads the whole group table (`stream flush`,
  153 µs p99 at 5000 resident sections), which grows with render distance.
- The client has no block-edit path, so remeshing on a block change cannot be measured.
- Bevy's window screenshot is black on some frames.
