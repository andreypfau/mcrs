# Performance

Every number carries its scenario. Medians are over a window, never a mean and never one frame:
the wall frame and the engine time over the frames of the last second, as vanilla counts its fps,
and the CPU stages and GPU passes over the last 256 frames. A reported figure is the median of
the lines a settled run wrote, not one line. The first ten seconds after launch are discarded. Run-to-run spread on the settled figures, from two runs of
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
`MCRS_TURN=<seconds>` (turn a scripted flight round once),
`MCRS_GPU_HOT=<workgroups>` (hold the GPU at speed, see the GPU frame section), `MCRS_FULLSCREEN=0`.
Fullscreen lands on the primary monitor; a sized window is centred on it and stays on top, because
a covered window is not presented and a frame that gets no swapchain texture is never drawn.
Tracy: build with `--features telemetry-tracy`, capture with
`tracy-capture -o trace.tracy -f`, aggregate with `tracy-csvexport`. Apple's Metal HUD
(`MTL_HUD_ENABLED=1`) shows the presented rate, the GPU time per frame and whether the window is
composited or direct to display.

## What one line reads

```
290 fps (3.447 ms median, 17.628 p99, 31.10 max over the last second)
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
| forget from the column view | 1.08 / 1.53 / 2.20 | 0.44 / 0.80 / 0.91 | 0.33 / 0.46 / 0.51 | 675 columns, 5236 sections | 0 to 450 KB |
| chunk pipeline drained between ticks, dead slots swept through a set | 1.16 / 1.54 / 2.01 | 0.42 / 0.74 / 0.95 | 0.30 / 0.68 / 1.21 | 729 columns, 5650 sections | 320 to 620 KB |

What was found, in order, with Tracy means over the settled part of a 70 s flight:

- **The server never took a column back.** The area-of-interest system computed an unload set
  and wrote forget packets, but aimed them at the dimension-local player entity, and the host
  stamps a session onto a single-player packet by the player's host anchor and drops one it
  cannot stamp. Aiming them at the anchor made the forgets arrive, and then a flight that turned
  round came back to holes: that system unloads at the view radius while the column view sends
  at one more and only clears its sent set when a column leaves its own view, so a column
  between the two radii at the turn was forgotten by the client and never sent again, and the
  eight around it could never be meshed. Vanilla's chunk map sends and drops from one view
  diff, and so does this now: the forget goes out from the column view's unload path, the
  area-of-interest system keeps its subscriptions only. Out and back at maximum speed
  (`MCRS_TURN=30`), the client holds the view's 729 columns with 89% of their sections meshed,
  where before the turn left 6% meshed. In a straight flight residency holds at 700 to 1500
  columns instead of growing without bound (44 000 to 86 000 sections evicted per run), the
  main-world median fell 0.63 to 0.43 ms, and the group-table re-upload that had saturated the
  4 MB belt every frame fell to 300 to 450 KB.
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

Screenshots: `docs/perf/m5-flight-2560x1440.png`, 82 s into the flight, over ocean;
`docs/perf/m5-return-2560x1440.png`, 33 s after turning round, where the holes used to be.

Gate: the settled flight's worst frame read 1.83, 1.87, 1.90, 2.03 and 2.20 ms over five runs,
with the main world's own worst frame under 0.95 in each; the frames over 2.0 carry about 0.5 ms
outside the main world, the extract and the render stages, which the stage marks put in the
prepare stage and Tracy has not yet named.
Breaking and placing blocks cannot be measured: the client has no block-edit path, no handler
for block-update packets and no input for it.

## Chunk loading

The stats line now carries `Hops p50 ms`: for the columns on screen, the median time each spent
getting into every stage of the column trace from the one before, server and client alike.
Scenario: save `two`, flying south from spawn at vanilla sprint speed (`MCRS_FLY=0.05`) over
saved terrain, 2560x1440 window, 60 s, the last line.

| build | spawn | queue | gen | load | ready | sent | ticket to sent | recv | mesh |
|---|---|---|---|---|---|---|---|---|---|
| before | 55 | 55 | 0 | 54 | 163 | 51 | 378 | 79 | 767 |
| the tick chained | 0 | 0 | 0 | 55 | 109 | 0 | 164 | 62 | 760 |
| drained between ticks | 0 | 0 | 0 | 3 | 51 | 1 | 55 | 24 | 784 |
| the drain spawns and dispatches too | 0 | 0 | 0 | 3 | 3 | 1 | 7 | 28 | 786 |
| and rebuilds the column index, so the light goes with it | 0 | 0 | 0 | 4 | 4 | 0 | 8 | 24 | 787 |

Reading a saved column was never the cost: over saved terrain a column read costs 73 µs and its
decode 87 µs (Tracy means), against 1.3 ms to generate one. The 378 ms was the pipeline: every
hop from ticket to sent was a system on the 20 Hz fixed schedule, so each cost a tick, "ready"
cost three because the loading queue stopped at the first column still loading and a column's
sections straddled the 512-section spawn cap, and sends were capped at ten columns a tick where
a row is 27. Vanilla runs its chunk tasks in the idle time before the next tick
(`MinecraftServer.waitUntilNextTick`), and its chunk map sends and drops from one view diff.

What changed: the view diff, its tickets, the chunk spawn and the dispatch are chained inside
`FixedUpdate`; completed columns, the load requests they raise, the forced tickets for the rest
of the column, their spawn and dispatch, readiness, the send and the flush to the host are one
`ColumnDrain` schedule run at the tick's end and every 2 ms while the loop would otherwise
sleep, and the host bridges and writes the sockets right after each pump (`OutboundFlush`).
Both queues skip past a column still loading. Sends are capped at vanilla's 64 a tick and at
a quarter of the socket's byte cap with the light arrays counted, since 64 columns of 65 KB
overran it when the count alone was raised, and 4.6 MB more when the light was left out of
the count. The spawn cap is 4096 sections and the ticket cap 4096 a tick. The drain also
rebuilds the column index the light packet walks: sent before that, a column went out with
every section unlit and the world drew black.

The 786 ms on the client is the leading edge at sprint speed: a column is meshed once the row
beyond it has arrived, as vanilla renders a chunk once its neighbours are loaded, and at 11
blocks a second the next row is 1.5 s away. At maximum speed it reads 207 to 217 ms. The 25 to
28 ms from sent to received is the client's own socket read and decode, and is the next hop
to look at.

The tint window wraps. The biome colours lived in a fixed 1024-block square around spawn and
the sampler clamped to its edge; 3000 blocks out, grass off to the sides multiplied by texels
nothing had written and drew black. A column now writes its square at its position modulo the
window, the sampler repeats, and the camera passes its own section's place in the window, so
the coordinates stay exact wherever the player is. The window only has to be wider than what
is resident: 1024 blocks holds a view of up to 31 columns.

## Render distance

`MCRS_VIEW=<columns>` is what the client asks for and the server now honours it (it used to
send 13 whatever the client said); the default is 96, three times vanilla's maximum. Native,
2560x1440 window, `MCRS_HOT=1`, save `two-relight` (the same terrain as `two`, whose spawn
chunks were since rewritten by 26.3 Pre-Release 1), spawn view, settled. Engine medians are
the quietest settled line of a 100 s run; the GPU world pass is at the display's clock.

| columns | resident columns / sections | engine ms | main world ms | sight-line walk ms | drawn triangles | GPU world ms |
|---|---|---|---|---|---|---|
| 8 | 361 / 2 867 | 0.68 | 0.25 | 0.03 | 150 530 | 0.61 |
| 12 | 729 / 6 170 | 0.71 | 0.28 | 0.06 | 226 784 | 0.76 |
| 16 | 1 225 / 10 800 | 0.66 | 0.28 | 0.09 | 340 010 | 0.97 |
| 20 | 1 849 / 16 599 | 0.89 | 0.42 | 0.18 | 587 460 | 2.5 |
| 24 | 2 601 / 23 367 | 0.91 | 0.46 | 0.27 | 870 496 | 3.4 |
| 28 | 3 481 / 31 566 | 1.07 | 0.61 | 0.34 | 1 249 770 | 1.1 to 1.7 |
| 30 | 3 969 / 36 157 | 1.35 | 0.81 | 0.49 | 1 463 882 | 3.1 to 4.1 |
| 96 | 38 025 / 295 511 | 3.09 (p99 5.9) | 2.2 | 1.6 | 14 382 144 | 10.9 (cull 0.46) |

The CPU frame is flat to 16 columns and grows with residency past it, most of it the cave
sight-line walk (bounded by its 96x32x96 box, so it stops growing past 48 columns) and the
rest spread over the main world. At 96 columns the frame is GPU-bound at 9.4 ms wall: 14.4 M
triangles reach the rasterizer through 7 draws, the arenas sit at 50% of 4.3 GB (quads 768 MB,
models 2.5 GB, faces 1 GB), the process holds 12 GB, and the world settles 80 s after launch
with a 32 MB upload budget (the 4 MB default drains 124 000 waiting meshes at 360 a second and
never settles inside four minutes). To hold 96 columns the tint window is 4096 blocks, the
section table 2^19 rows and the group table 2^22.

**The CPU frame at 96 columns**, spawn now at 0/128/0 so the view holds 15.3 M triangles,
`MCRS_HOT=1`, settled, median of three lines:

| build | engine median / p99 | main world | sight-line walk |
|---|---|---|---|
| before | 3.09 / 5.9 ms | 2.2 ms | 1.6 ms a frame, on the frame |
| the walk off the frame, the group table written incrementally | 0.99 / 1.30 ms | 0.32 ms | 3.1 ms a walk, on the compute pool |

The sight-line walk used to flood from the camera every frame, pruned by the frustum, and then
scan the whole laid box to project its bits; at 96 columns that was 1.6 of the main world's
2.2 ms. It now runs on the compute pool, without the frustum (the GPU cull tests that per group
anyway), only when the camera's section or the topology changed, from a queue of table edits
the loader hands it; between walks a newly laid section is drawn until told otherwise. In the
open it reaches the whole box, 85 794 sections, in 3.1 ms of pool time. The group table used
to be rebuilt and re-uploaded whole on any arrival, 2.8 M records or 56 MB at this residency;
each stream now keeps its own block with room to grow, a landing section appends its records
and an evicted one leaves records with no quads, and a stream is rebuilt into a fresh block
only when it outgrows its block or half of it is dead.

**The GPU frame at 96 columns**, GPU held at clock (`MCRS_GPU_HOT=16384`), same view:

| streams drawn | GPU cull | GPU world | drawn triangles |
|---|---|---|---|
| all | 0.43 ms | 10.4 ms | 15 310 688 |
| all, terrain viewport at a twentieth (`MCRS_RASTER=0.05`) | 0.41 | 9.0 | 15 310 688 |
| solid greedy only | 0.19 | 4.15 | 8 129 316 |
| the four opaque streams | 0.31 | 6.86 | 12 755 936 |
| cutout and model streams only | 0.12 | 2.93 | 4 626 620 |
| the two translucent streams only | 0.10 | 3.32 | 2 554 752 |

Fill is 1.4 ms of the 10.4; the rest is vertex work, at 0.51 ns a triangle for the solid
stream, 0.63 for cutout and models, and 1.3 for the two translucent streams. Their ordered
draws spanned 6 936 798 triangles' worth of slots for the 2 554 752 that survived, so about two
thirds of what their vertex shader walked was holes.

**Packing the ordered draws.** The translucent streams are now compacted by a prefix sum over
batches of groups (count, scan, scatter: three dispatches in place of the hole-writing cull and
its finalize), which keeps the list order the blend depends on and leaves no holes. Same view:

| build | GPU cull | GPU world | drawn triangles |
|---|---|---|---|
| ordered draws spanning holes | 0.43 ms | 10.4 ms | 15 310 688 |
| ordered draws packed | 0.45 | 7.94 | 15 310 688 |
| and drawn as a triangle list, no instancing | 0.45 | 5.90 | 15 310 688 |
| and indexed, four vertices a quad | 0.45 | 4.27 | 15 310 688 |

**No instancing.** A quad was an instance of a four-vertex strip; Metal pays per instance, and
a plain triangle list of six vertices a quad, with the vertex shader reading its quad from the
vertex index, reads 5.90 ms against 7.94 for the same triangles, though it runs the vertex
shader six times a quad instead of four. Indexing the list through a fixed index buffer (six
indices a visible slot, grown with the visible list) gives the four vertices back: 4.27 ms.
Of that, 1.35 ms is fill (2.93 ms at a twentieth of the viewport) and 2.9 ms vertex work:
30 M vertices in 2.9 ms, about ten billion a second, which is the machine's vertex rate rather
than anything left in the shader. The solid stream alone is 2.3 ms for 8.1 M triangles.

**Occlusion.** The brief asked for proof before a depth pyramid was built, so the pyramid was
built first as a counter: the last frame's depth reduced to the farthest surface per footprint
(0.07 to 0.11 ms), and the cull projecting each surviving group's box onto it and counting the
quads it would have hidden, still drawing them. Settled at spawn the count read 7.96 M of
15.3 M triangles, half of what was drawn. So the test culls now, in two passes: the first
against the last frame's pyramid, with groups that test alone hides left as candidates; the
frame's own depth then rebuilds the pyramid and a second cull revives the candidates it no
longer hides into a second set of draws, so a turn or a step never leaves a hole for a frame.
A box reaching behind the camera or past the screen's edge is never hidden.

| pass | GPU ms |
|---|---|
| cull | 0.495 |
| world | 2.315 |
| pyramid | 0.073 |
| second cull | 0.302 |
| second world | 0.007 with the camera still |
| **frame** | **3.19**, from 4.27 without the test and 10.4 at the start of the day |

Drawn triangles 7.35 M, 7.96 M hidden. The second cull walks every group again to find the
candidates, which is where its 0.3 ms goes; a compact candidate list would make it scale with
what the first pass left rather than with what is resident.

**The frame at 96 columns, end of the day**, hot clocks, GPU at clock, settled, overlay hidden:
engine 1.0 to 1.5 ms (main world 0.33 to 0.55), GPU 3.2 ms. The CPU side sits at the goal; the
GPU side is three times over it with every quad the cull cannot remove still costing its
vertices.

Screenshot: `docs/perf/m6-96-2560x1440.png`, the packed build at spawn. A world pass under 1 ms needs about
ten times fewer quads in the vertex shader than this view sends, which no cull can deliver from
a view where most of the terrain is in front of the camera; that is the size of the distant
geometry problem at 96 columns.

## Chunk delivery at 96 columns

With two clients loading on the machine at once, 5 385 of a view's 38 025 columns never
reached the client and the ring they should have filled stayed empty: the bridge dropped an
encoded blob when its four-deep writer channel was full and marked the columns sent. Blobs now
wait in order, and column sends are rated at 1 280 a second (vanilla's 64 a tick) instead of
64 per 2 ms drain pass, so a view of 96 columns arrives in 30 s rather than 15 and nothing is
lost. The client meshes it in 160 s either way.

## Web

Not measurable yet. `scripts/build-web.sh` produces a 40 MB single-file bundle that Chrome runs
over WebGPU (`http://localhost:<port>/mcrs.html?stats=3`) and it draws the sky, but the start-up
blocks the page's main thread: on the first load 87 s passed between the adapter log and the
first asset log, the stats line was written for four frames reading zero everywhere, and after a
reload the page answered nothing for more than eight minutes. The render world's start-up
schedule also logged itself once per frame in those four frames, which on native runs once.
Until the browser build reaches a steady frame there is no floor to state for it; the numbers
that will count there are the CPU stages and the GPU pass timestamps, never the FPS line.

## Hardening

Everything above re-measured on one build, in one session, in the 2560x1440 window, overlay
hidden, `MCRS_HOT=1`, save `two-relight`. The default render distance is 96 columns and every
headline figure is taken there; 12 columns and the empty frame are kept for the trend. A run
is 60 s at the floor, 90 s at 12 columns, 300 s static at 96 (the view settles 160 s after
launch with `MCRS_UPLOAD=32`) and 120 s in flight; figures are medians of the settled lines,
p99 and max the median of the per-second p99 and max. The build at the start of the day is
`e52647d5`.

| scenario | engine median / p99 / max | main | extract | render | GPU cull / world / pyramid / second cull / second world | drawn tris (hidden) |
|---|---|---|---|---|---|---|
| floor, start of day | 0.88 / 1.09 / 1.09 | 0.31 | 0.11 | 0.29 | none / 0.31 / 0.22 / none / 0.45 | 0 |
| floor, build before occlusion (`8026a767`) | 0.70 / 1.02 | 0.26 | 0.09 | 0.20 | none / 0.31 | 0 |
| **floor, end of day** | **0.80 / 0.94 / 0.94** | 0.30 | 0.11 | 0.21 | none / 0.30 / none / none / none | 0 |
| 12 columns, start of day, GPU at clock | 1.02 / 1.23 / 1.23 | 0.32 | 0.11 | 0.40 | 0.086 / 0.258 / 0.072 / 0.050 / 0.007 | 201 000 (2 400) |
| 12 columns, build before occlusion | 1.00 / 1.40 | 0.40 | 0.11 | 0.31 | 0.058 / 0.302 | 227 000 |
| 96 columns static, start of day, GPU at clock | 1.12 / 1.34 / 1.35 | 0.33 | 0.12 | 0.45 | 0.494 / 3.20 / 0.074 / 0.304 / 0.007 | 7 350 000 (7 960 000) |
| **96 columns static, end of day** | **1.12 / 1.33 / 1.33** | 0.33 | 0.12 | 0.45 | 0.48 / 3.3 / 0.074 / 0.29 / 0.007 | 7 380 000 (8 020 000) |
| 96 columns static, `MCRS_OCCLUSION=0` | 1.00 / 1.20 / 1.20 | 0.33 | 0.12 | 0.34 | 0.46 / 6.0 | 15 400 000 |
| 96 columns, flight at maximum speed, start of day | 1.97 / 15.5 / 15.5 | 0.48 / 14.6 p99 | 0.18 | 0.58 | at the display's clock | 280 000 |
| 96 columns, flight at maximum speed, queue bucketed | 2.36 / 5.6 / 6.2 | 0.52 / 3.8 p99 / 4.9 max | 0.18 | 0.59 | at the display's clock | 184 000 |
| **96 columns, flight at maximum speed, end of day** | **1.56 / 5.0 / 5.9** | 0.47 / 4.0 p99 / 4.5 max | 0.17 | 0.58 | at the display's clock | 208 000 |
| 96 columns, out and back (`MCRS_TURN=45`), end of day | 1.44 / 5.0 / 5.5 | 0.44 / 4.0 p99 | 0.17 | 0.55 | at the display's clock | 436 000 |

What was found, in the order it was taken:

- **The tree did not build.** `HEAD` referenced the depth pyramid, its shader and the App Nap
  opt-out, and none of the three was committed. They are now.
- **The empty frame paid for occlusion.** With nothing resident the frame still built the
  pyramid, ran the second cull and opened the second world pass: 0.88 ms of engine time
  against 0.70 on the build before occlusion, measured back to back in the same window, and
  0.7 ms of GPU passes at the display's clock. The passes now need resident groups:
  0.80 / 0.94 ms, render stage 0.29 to 0.21. What is left over the older build, 0.1 ms, sits
  inside the 0.2 ms spread on the main world. `MCRS_OCCLUSION=0` prices the test at any
  distance: at 12 columns it hides 1% of the triangles for 0.11 ms of GPU and 0.09 of CPU; at
  96 it takes the GPU frame from 6.0 to 3.7 ms.
- **The world pass at 96 columns reads 3.2 ms where 2.3 was recorded.** The indexed draw
  went out in `2d943bd4` (quads are six vertices again, the index buffer with them), after the
  2.3 ms was taken; every other GPU figure in that view is unchanged to 0.01 ms. See the open
  question in DECISIONS.md.
- **Tracy could not capture a 96-column run.** The server named a span per column position,
  and Tracy allots one source location per distinct name and field set: 32K ran out eight
  seconds into a flight and the capture ended. The span lost its fields.
- **The meshing queue was sorted whole on every change.** In flight at 96 columns it held up
  to 900 000 positions, since nothing dropped a section whose column the server had taken
  back, and any arrival or eviction re-sorted it by distance: `stream admit` ran over 4 ms in
  2 603 of the 85 s capture's frames, 18 ms at most, and the main world read 10 to 20 ms once
  a second. The queue is now bucketed by how many sections a position lies from a reference
  that follows the camera in steps of eight, a departed column's sections leave it, and an
  admission pops the nearest bucket: engine p99 15.5 to 5.6 ms, main-world p99 14.6 to 3.8.
  A first version kept each position in the bucket it was queued in and only moved it when
  it surfaced; after a long flight the near sections sat behind the far edge, nothing near
  the camera was meshed and the ground drew as scattered fragments for 40 s.
- **The Metal capture trigger scanned the queue every frame** to know whether the world had
  settled, whichever the run had asked for. It scans only with a trace path set.
- **Adopting an arrival cost what was resident.** With the sort gone, `stream adopt` was the
  flight's next zone: 1.74 ms mean and 10.9 max on every other frame, since it diffed the whole
  store against the columns it knew and then cloned the store, 38 000 columns each way, for the
  mesh tasks. The store now journals each arrival and departure and the loader drains that; a
  mesh or tint task takes the column it works on and the eight around it by `Arc` instead of a
  snapshot of the store. Adopt 0.38 ms mean, engine median in flight 2.36 to 1.56 ms.
- **What is left in the flight frame**, Tracy over 85 s of it after all of the above: the queue
  rebucket, 3.4 ms at the p99 of `stream admit` and 4.7 at most, once every eight sections of
  travel; and adopt at 9.8 ms at most, 24 frames over 1.5 ms, when a batch of hundreds of
  columns lands in one frame and each brings 216 sections to queue and hundreds to evict. The
  first wants the rebucket off the frame or per bucket; the second wants the adoption bounded
  per frame the way admission is. Neither was taken today.
- **The static view hitches once.** A settled 96-column run shows one main-world frame of 19 to
  20 ms about four minutes in, in two runs out of three; no capture covered it and it is not
  named.
- **The web build quit on its first terrain pipeline.** Chrome rejected the terrain shader
  module: the cutout finish evaluated the wireframe test behind a short-circuit `||`, which put
  the `fwidth` inside it in non-uniform control flow, and since the three fragment entries share
  one module every wireframe pipeline failed and Bevy quit the app on the validation error.
  Native naga accepts it. The test is now taken before the condition, and the page draws the
  sky and the clouds; it then stops at that frame, with no stats line in 45 s and the same
  clouds in three screenshots, which is the start-up stall recorded under Web. The web target
  is still not measurable.

Chunk delivery at 96 columns, `Hops p50 ms` on the last line of the flight: spawn 1, queue 0,
gen 32, load 19, ready 21, sent 1, received 29, meshed 272 over 8 167 columns on screen, where
the build at the start of the day read 12 905 ms to mesh over 7 192.

Screenshots: `docs/perf/m7-96-2560x1440.png`, the settled static view 280 s in, at dusk since
the clock ran from the save's evening; `docs/perf/m7-flight-2560x1440.png`, 110 s into the
flight at maximum speed.

Gates, as they stand at 96 columns: the static engine frame is 1.12 ms at the median and
1.33 at the worst second, so the CPU sits at the goal and inside the ceiling, with the one
unnamed 20 ms frame above; the GPU frame is 3.7 ms, over the ceiling as the render distance
section already records, and 0.9 of that is the indexed draw's absence; the flight's worst second is 5 to
6 ms of engine time with the main world at 4 to 4.5, over the ceiling, where it was 15 to 20
at the start of the day, and the two zones that remain are named above.

## Column staging

The three stages of the `Filled → Run → Merged` ladder, taken on their own rather than through
a running server:

```
cargo test --release -p mcrs_minecraft_worldgen_generator the_ladder_costs -- --ignored --nocapture
```

Scenario: the overworld router at seed 4242, forest everywhere, and the biome's own
`trees_birch_and_oak_leaf_litter` as the only feature. That is the widest consumer there is —
a tree's crown and its decorators reach past the column that seeds them, so it is the one that
reads the ring. A 9x9 region is merged, its runs cover one ring more and its fills two; the two
outermost rings of each are the warm-up and are discarded, leaving 96 columns. Figures are the
median over those columns, and the range is over three runs of the whole thing. The dimension
has no carvers, so `Filled` here is the fill, the biomes and the material surface and nothing
else; a carved overworld column costs more than this.

| figure | median | over three runs |
|---|---|---|
| `Filled` | 2.17 ms | 2.165 to 2.211 |
| of it, the pre-carve descent | 0.034 ms | 0.033 to 0.035 |
| `Run` | 0.373 ms | 0.373 to 0.374 |
| of it, the live centre maps, over 841 updates | 0.013 ms | 0.013 to 0.014 |
| `Merged` | 0.176 ms | 0.176 to 0.180 |
| the whole column | 2.72 ms | |
| unpacking one column into a dense buffer | 0.097 ms | 0.096 to 0.097 |
| reads per run | 14 440, of them 3385 from the ring | exact |
| live centre map updates per run | 841 | exact |
| one staged snapshot | 44 KB over 24 sections | |

The two counts are exact rather than sampled: `Window` counts its own reads and map updates and
publishes them per run, and the same region gives the same numbers every time. Counting costs
about 2% of the stage: with the three counters removed the run reads 0.366 ms against 0.373.

The pre-carve descent is 1.6% of the stage that pays for it and the live centre map updates are
3.5% of theirs. Neither is worth a second look. `Run` is 14% of the column, which is what had to be known
before the tree consumer was committed to: the parallel `Run` buys little next to
the fill, and the halo of Q1 costs nothing extra because the fills it forces are fills the view
was going to want anyway.

The live maps cannot be measured by switching them off. Running the same window with the
predicate table removed places 1658 writes where the live path places 1205: the four final maps
are read back within the run, not merely written. The figure above is a replay instead — the
write list one run produced, applied to the same column and the same starting maps.

**D9 is settled: the ring stays paletted and is not unpacked.** A flat ring means unpacking the
eight neighbours into dense buffers, at 0.097 ms each: 0.78 ms per run, more than twice the whole
`Run` stage it would be speeding up, against 3385 reads that are 23% of the run's reads inside a
stage that is 14% of the column. There is no arrangement of those numbers where it wins, and the
memory (eight more buffers of 98 304 cells per worker) is on top. The gate set on the flat ring is
therefore closed against it.

The last-section cache the same decision mentions is not needed either, and the earlier claim
that one section list per dimension is what makes it pointless holds up. A ring read is pure
arithmetic: `window_slot` gives the 3x3 slot as an array index, `FilledSnapshot::slot` turns the
world y into a `Vec` index with a shift and a subtraction, and the palette answers from there.
Nothing on that path hashes a section position, which is the whole reason `BulkSectionAccess`
exists. What the path does still do on every ring read is probe an `FxHashMap` — the unit's own
ring writes, which Wn2 describes as a bit per cell and the code holds as a map. A run that wrote
into no neighbour pays one emptiness branch for it; one that did pays a hash. At 3385 reads that
is tens of microseconds of a 330 microsecond stage, so it is recorded and not acted on.
`FilledSnapshot::block` also recomputes the slot its caller just computed.

**The staging store under a moving player** is bounded by the view and by nothing else.
`cancel_stale_columns` keeps every column within two of a wanted one, and a merged column is kept
whole past its delivery so that a section ticketed late is answered from the same blocks rather
than from the undecorated snapshot the neighbours still read. Both squares are resident at once:

| view distance | filled | merged | store |
|---|---|---|---|
| 8 | 441 | 289 | 31 MB |
| 13 (what the server sends) | 961 | 729 | 72 MB |
| 32 | 4761 | 4225 | 386 MB |
| 96 (`MCRS_VIEW=96`) | 38 809 | 37 249 | 3270 MB |

The counts are the halo rule, exact; the bytes are the measured 44 KB a snapshot owns. The 24
sections are this dimension's; a taller one scales with them.

## Carving, per column

What the carvers cost today: every column walks the 17 x 17 source chunks around it and runs the
carvers of each source's biome, reading no neighbour. Taken at commit `84b151e1f` before any
change to how carving is organised, so it is the figure a later change is compared with.

Machine: Apple M4 Max, 16 logical CPUs, 128 GiB, macOS 26.6 (Darwin 25.6.0). Build: the `bench`
profile (optimized), nightly `rustc 1.100.0-nightly (5db7f4be8 2026-09-01)`. The machine was not
idle: desktop applications were open and the one-minute load average read between 4.7 and 7.7
at the start and end of the runs. Three runs of each command, one
after the other, the compiled bench binaries started directly from the crate directory with
`BEVY_ASSET_ROOT` set to the repository root (the same environment `cargo bench` gives them).

| command | per column or chunk, mean of 3 runs | run-to-run spread |
| --- | --- | --- |
| `beta_chunks -- 32 1 12345` (1024 chunks, 1 thread) | 0.4287 ms per chunk, 2328 chunks/s (2312 to 2342) | 1.3% |
| the same, `caves` stage | 0.0773 ms (0.077 to 0.078) | 1.3% |
| `overworld_pipeline -- 64 777 0` (4096 columns), whole column | 3.567 ms (3.564 to 3.571) | 0.2% |
| the same, `carve` (the mask build alone) | 0.104 ms (0.103 to 0.106) | 2.9% |
| `overworld_pipeline -- 64 845 0` (4096 columns), whole column | 2.985 ms (2.969 to 2.995) | 0.9% |
| the same, `carve` | 0.0967 ms (0.096 to 0.098) | 2.1% |

Per-column distribution of the whole column, p50 / p90 / max over the 3 runs: seed 777 3.32 /
5.12 / 7.1 to 8.9 ms, seed 845 2.83 / 3.81 / 5.2 to 5.6 ms. The maximum is a single column and
moves by 22% and 6% between runs; the mean and the percentiles do not. Seed 845 at the origin
is almost all frozen ocean, so its columns carry water over a carved floor. The carver share of
the whole column is 2.9% (seed 777) and 3.2% (seed 845).

The spread is the largest minus the smallest of the three runs over their mean. An earlier
measurement on this machine saw noise up to 13% under heavier load; a difference smaller than
the spread above is not a finding.

## Carving, region width and cache capacity

The modern carvers walk each source once per region of `REGION_WIDTH` x `REGION_WIDTH` columns
and keep up to `REGION_CAPACITY` regions. Chosen from the sweep below: width 2, capacity 256.

Machine and conditions as above, and worse: the desktop was busy throughout. The one-minute load
average read 5.4 to 17 at the starts of the bench runs and 11 to 33 around the sweeps, which is
more than the 16 logical CPUs, so every number is a mean of repeated runs with its spread beside
it. The 8 workers are the pool of the chunk scheduler on this machine (`(16 / 2).max(4)`).

**Sweep** (`measure_carve_regions`, release build, mask per column over a 64 x 64 block at columns
-35..29, seeds 777 and 845, three processes of three rounds each, widths alternating inside a
round, caches cold and the source-biome tiles warm). Cells read seed 777 / seed 845, with the
largest-minus-smallest spread over the nine samples in brackets.

| width | entry, cache bound at the capacity | 1 thread, distance order: mean ms, p99 ms | 8 workers, distance order: mean ms, columns/s | 8 workers with 3.4 ms of other work per column: columns/s | 8 workers, row order: columns/s |
| --- | --- | --- | --- | --- | --- |
| 1 | 12,032 B, none | 0.094 / 0.088 (4%), 0.22 / 0.20 | 0.095 / 0.090 (2%), 84,300 / 88,500 (2%) | 2283 / 2287 (1%) | 80,600 / 87,300 (21%, 2%) |
| 2 | 48,128 B, 256 = 12,320,768 B | 0.053 / 0.049 (6%), 0.42 / 0.39 | 0.061 / 0.058 (3%), 131,200 / 136,500 (3%) | 2310 / 2314 (1%, 0%) | 74,000 / 78,000 (2%, 3%) |
| 4 | 192,512 B, 128 = 24,641,536 B | 0.040 / 0.038 (5%, 3%), 0.90 / 0.83 | 0.080 / 0.071 (58%, 2%), 102,000 / 112,800 | 2317 / 2321 (1%, 0%) | 50,400 / 53,600 (4%, 2%) |
| 8 | 770,048 B, 64 = 49,283,072 B | 0.040 / 0.038 (4%, 5%), 1.94 / 1.93 | 0.126 / 0.123 (13%, 4%), 63,700 / 65,200 | 2318 / 2317 (0%) | 27,400 / 28,600 (3%, 5%) |

The capacity is the smallest of 4, 8, ..., 256 with no region built twice over the distance-ordered
columns of the block, doubled: 128, 64 and 32 for widths 2, 4 and 8 (row order needs half of
that), so 256, 128 and 64. At 4 capacity the width 2 cache rebuilds 2,939 of 4,028 builds.

What the sweep says. A shared walk cuts the mask time per column for one thread by 44% at width 2
and by 57% at widths 4 and 8, and the first column of a region pays for it: p99 and the maximum
grow in proportion to the width (maximum about 0.4 ms at width 1, 0.7 at 2, 1.5 at 4, 3.8 at 8).
With 8 workers asking the same table at once, a second asker of a region being built blocks its
pool thread, and the harm grows with the width: with nothing but mask calls, width 2 completes
1.55 times the columns per second of width 1, width 4 completes 1.2 to 1.3 times and width 8 0.75
times; in row order, where neighbouring columns go to neighbouring workers, widths 2, 4 and 8 all
fall behind width 1. Once the rest of a column (3.4 ms here) separates the workers, the
contention costs nothing visible and what is left is the mask time saved: 2283 to 2310 and 2287
to 2314 columns per second at width 2 (+1.2%), 2317 and 2321 at width 4, 2318 and 2317 at width
8. The rule picked the lowest mask time per column with 8 workers in distance order, which is
width 2, ahead of width 1 by 35 to 36% (spread 2 to 3%) and of width 4 by 17 to 24%; widths 4 and 8 are not
ahead of width 2 at the throughput the pool reaches with real work per column, and cost two and
four times its memory and a longer wait for the first asker.

**Bench, whole column** (`overworld_pipeline -- 64 <seed> 0`, bench profile, compiled binaries
started directly with no build running, width 1 and the chosen configuration alternated, three
runs each, load 5.4 to 17 at the starts):

| seed | width 1: column ms (spread), carve ms | width 2: column ms (spread), carve ms | column | carve |
| --- | --- | --- | --- | --- |
| 777 | 3.554 (0.4%), 0.1043 (1.0%) | 3.514 (0.2%), 0.0593 (1.7%) | -1.1% | -43.1% |
| 845 | 2.968 (0.1%), 0.0977 (1.0%) | 2.922 (0.7%), 0.0553 (1.8%) | -1.5% | -43.3% |

The carve line settles; the whole column moves by about one percent in the same direction in
every pair, which is the size of the spread in one of them. The 90th percentile of the whole column is the same within the spread (5.11 against
5.08 ms and 3.79 against 3.74 ms) and its maximum is one column that moves by 20 to 40% between
runs, so no tail claim is made from it. Beta is unchanged: `beta_chunks -- 32 1 12345`, three
runs, 2298 chunks/s (spread 5.4%) and the `caves` stage 0.0773 ms (1.3%), against 2328 and
0.0773 ms before; the Beta carver does not take a region.

**What it costs.** The cache holds at most 12,320,768 bytes (11.75 MiB) in the overworld and
4 x 3,840 x 256 = 3,932,160 bytes in the Nether, each owned by its dimension's table and freed
with it. Up to 8 evicted regions can stay alive in the hands of the columns that were reading
them, 48,128 bytes each, and a build in progress holds one entry. A capacity too small for the
columns in flight costs rebuilds at 324 sources per four columns against 289 per column, never a
different block. 256 holds a view of 32 columns in every direction with the margin of two; several
players in different places share it and evict each other.

**The width-1 path.** Measured in one process, alternating, over the same block, the region path
at width 1 takes 0.094 and 0.088 ms per column against 0.086 and 0.081 ms for the per-column
sources (+9% and +8%, about 0.007 ms). No production table takes that path at the chosen width;
the cost is accepted for the configuration a test builds with `with_region(1, 0)`, and a return
to width 1 in production would pay it.

## Server start

Scenario: the release `mcrs` binary on the reference machine, started with no world folder (the
server generates), listening on the default port, `BEVY_ASSET_ROOT` set to the repository root.
The time runs from the process spawn to the moment the `entering Playing state` line reaches the
parent through the stderr pipe, on a monotonic clock. The loader's duration and entry count are
the fields of its own `loaded registries` line. Three runs, each a fresh process.

| run | start to Playing | loader duration | registries | entries |
|---|---|---|---|---|
| 1 | 1.007 s | 77.6 ms | 147 | 9079 |
| 2 | 0.769 s | 28.4 ms | 147 | 9079 |
| 3 | 0.769 s | 29.1 ms | 147 | 9079 |
| median | 0.769 s | 29.1 ms | 147 | 9079 |

This is the baseline the work that moves the world registries into the loader compares against:
a later start time or loader duration on the same scenario is a regression, and a later entry
count is the data it now carries.

The machine was not idle: the load average read 48 over the last minute, 39 over five and 38 over
fifteen on its 16 cores, from other builds running alongside, so the figures are an upper bound
and the first run, which also read the data pack cold, shows it. Three earlier attempts to
measure failed before the server reached Playing and are not counted: three with an empty
directory as the world folder panicked for want of saved world generation settings, and six
without `BEVY_ASSET_ROOT` (three from another directory, three from the repository root)
panicked for want of `version.json` beside the executable.

### World registries on the loader

The same scenario on the tree where the world registries are carried by the registry loader and
every dimension world holds the host's registries by shared `Arc`. The load average is the
1, 5 and 15 minute reading taken as the run started.

| run | load average | start to Playing | loader duration | registries | entries |
|---|---|---|---|---|---|
| 1 | 77.0, 45.2, 30.3 | 2.178 s | 71.6 ms | 147 | 9079 |
| 2 | 71.3, 44.5, 30.2 | 0.771 s | 39.8 ms | 147 | 9079 |
| 3 | 71.3, 44.5, 30.2 | 0.768 s | 40.5 ms | 147 | 9079 |
| median | | 0.771 s | 40.5 ms | 147 | 9079 |

The spread of the start time is 1409.4 ms over the three runs, 2.5 ms over the last two; the
first run read the binary and the data pack cold. Every load was far above 4 on the machine's 16
cores, so all of these figures are upper bounds. Against the table above the median start time
is unchanged (0.769 s then, 0.771 s now), the entry count is unchanged, and the loader's median
duration reads 40.5 ms where it read 29.1 ms; the earlier table was taken at a load of 48 and
this one at 71, so the difference cannot be told from the load and is not called a regression.

This is the baseline the work that moves the worldgen registries onto the loader compares
against.

### Synced registry column

The load encodes the network form of every synced registry entry once and keeps it as a column of
the registry set; every connection's registry packets borrow that column. Measured with
`the_registry_packets_carry_the_synced_registries_in_declared_order`, which encodes the 32
`registry_data` packets of the shipped data pack for a client that knows no packs, so every entry
carries its data: 160260 bytes on the wire, on the tree of the change that adds the column over
`484cf4f5d`. A client that selects `minecraft:core` is sent the entries of the vanilla pack without
data, so this is the largest the packets get. The in-memory size of the column and the added load
time were not measured.

## Keys crate

Scenario: `mcrs_minecraft_keys`, the checked-in crate of generated registry markers, static ids and
data pack keys (about 1.2 MB of source in 153 files), on the reference machine's 16 cores, dev
profile, `cargo check`. Each row is one run, not a median. The crate alone is `cargo clean -p
mcrs_minecraft_keys` followed by `cargo check -p mcrs_minecraft_keys` with its dependencies already
checked. The workspace rows are `cargo check --workspace` into an empty `CARGO_TARGET_DIR`, once at
`3b6f10631`, the last commit before the crate existed (455 crates), and once at the tree that adds
the built-in worldgen change (456 crates). Cargo 1.100 nightly keeps the metadata under
`target/debug/build/<crate>/<hash>/out/`, not `target/debug/deps/`.

| measurement | result | 1-minute load average at start / end |
|---|---|---|
| crate alone, clean check | 1.14 s wall, 4.08 s user | 33.25 / 31.15 |
| crate alone, first run (also re-checked `mcrs_minecraft_registry`) | 1.56 s wall, 3.75 s user | 47.52 / 48.50 |
| clean workspace check without the crate | 66 s | 20.02 / 28.84 |
| clean workspace check with the crate | 59 s | 28.84 / 25.12 |
| `libmcrs_minecraft_keys-*.rmeta` | 6,161,686 bytes (6,164,226 in the empty target directory) | not applicable |
| `mcrs_minecraft_keys` after touching `mcrs_minecraft_world/src/lib.rs`, `cargo build -v -p mcrs_minecraft_world` | `Fresh` | 19.32 / 57.09 |

The machine was not idle at any point: every load average above is well over 4, from other builds
running alongside, so every time is an upper bound and a rerun on an idle machine is the way to get
the real figure. The two workspace rows are not evidence about the crate: the run with the crate
was the faster one only because the load happened to be lower, and a one-second crate sits far
inside that spread. What they do show is that adding it does not move a clean workspace check
outside the noise, and that a change in a dependent crate does not rebuild it.

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
- The client takes 25 to 28 ms from a column being sent to receiving it, mostly its own socket
  read and decode cadence.
- The tint window must exceed the view's width; a render distance past 31 columns needs it
  widened.
- Bevy's window screenshot is black on some frames.
- The staging store holds a merged column whole past its delivery, only so that a section
  ticketed late can be answered from it; that is half of the 72 MB it holds at 13 columns and
  half of the 3.3 GB it would hold at 96.
- A ring read probes the unit's own write map before it reads the snapshot, on every read
  whether the unit wrote into that neighbour or not.
