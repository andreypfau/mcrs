# Decisions

One line each, with the numbers that justified it. Newest last.

- **Frame time on macOS is quoted two ways.** The wall frame includes the swapchain acquire, which
  blocks until the display recycles a drawable: an empty scene measured 3.5 ms wall with 2.8 ms
  inside the acquire, and a deeper swapchain queue (`MCRS_LATENCY=8`) changed nothing (3.50 ms).
  The budget therefore binds on the engine figure, wall minus acquire, and on the GPU pass
  timestamps; the wall figure is reported next to it with the display it was presented on.
- **No exclusive 2560x1440 display mode.** Switching the video mode through winit resized the
  surface in two runs out of four, once left the window offset by a black band, and a crashed run
  left the display captured. Headline numbers are taken fullscreen at the display's native size
  (3840x2160 on the primary monitor) and every line states its pixel count; `MCRS_RESOLUTION`
  opens a windowed 2560x1440 surface instead, which macOS composites at 60 Hz.
- **The F3 overlay refreshes ten times a second and the stats log collects with it hidden.** The
  entries used to run every frame while shown, and the log used to force the overlay on, so no
  headline number could be taken with it hidden.
- **Engine time is measured with the performance cluster held at speed (`MCRS_HOT=1`).** The
  frame sleeps in the swapchain acquire and the cluster clocks down while it does, so the same
  work then reads two to three times longer and the tail is the clock ramp, not the code: the
  empty frame measured 1.37 ms median / 4.0 p99 / 9.2 max cold and 0.60 / 1.12 / 1.36 with a
  spinning helper thread. The helper is a measurement knob and not on by default: keeping a
  core busy is a power decision for the product: taken as no, the client does not spin, and a
  player at vsync pays the clock ramp until the CPU floor is small enough not to matter.
- **Sprite atlases grow in place.** Every bake used to re-upload every atlas (54 times in 40 s
  of flight, 2.4 ms mean, 4.8 ms max, for under a megabyte); now a bake writes only its new
  layers, 8 to 90 KB, into fixed-capacity arrays that regrow by a GPU copy. Screenshots at
  frozen noon match the previous build pixel for pixel outside the animated water, whose phase
  follows wall-clock time.
- **Column packets decode on the compute pool, in arrival order.** Decoding on the main thread
  inside command application cost 1.3 to 1.4 ms on arrival frames; the main-world p99 in flight
  went from 2.44 to 2.01 ms and its max from 3.5 to 2.9 ms (cold).
- **Geometry uploads go through a staging belt inside the frame's encoder.** At maximum flight
  speed with the 4 MB budget saturated, the render stage p99 fell from 0.94 to 0.49 ms and the
  engine p99 from 2.10 to 1.80 ms against `Queue::write_buffer`.
- **Not kept:** raising the main thread's QoS class (p99 3.9 vs 3.8 ms, inside the spread) and a
  deeper swapchain (no change; Metal caps drawables at three).
- **Bevy's UI systems run only while a panel is shown**, one frame longer so a hidden panel is
  extracted as hidden: hot floor engine 0.60 to 0.51 ms, main world 0.29 to 0.20 ms. The
  overlay and the chunk map still cost their layout while up (chunk map: 0.4 ms).
- **The state transition schedule runs only when a transition is pending** (it left the main
  schedule order; a `PreUpdate` system runs it on demand): 20 to 6 µs a frame.
- **The frame is one render system**: uploads, timestamp resolve, cull dispatch and a single
  render pass holding sky, opaque terrain, clouds and blended terrain. Five encoders and six
  command buffers became one and two; encode 198 to 150 µs, submit 52 to 38 µs, and the GPU
  pass timestamps lost their sky slot (the world pass reads 1.0 to 1.15 ms where sky plus
  terrain read 1.26). The picture is unchanged.
- **Dropped:** the light plugin (no lights here, 31 systems, 9 µs), the per-frame cave-bit
  upload when the walk changed nothing (11 µs), and a per-frame stat of the screenshot trigger
  file (3 µs; polled four times a second instead).
- **GPU pass times are taken with the GPU held at speed (`MCRS_GPU_HOT=16384`).** The governor
  clocks to the display's deadline: the same cull read 0.248 ms in a composited 60 Hz window,
  0.114 fullscreen at 120 Hz and 0.058 saturated, and it read 0.058 at 16384 and 32768 heater
  workgroups alike. The heater is a compute pass written against the draw args so the driver
  orders it between frames; a first version with its own buffer overlapped the next frame's cull
  and read it at 3.9 ms. Like `MCRS_HOT` it is a measurement knob and off by default.
- **Fullscreen goes to the primary monitor, and a sized window is centred on it and kept on
  top.** Winit could not find the current monitor and fell back to the built-in display, and a
  covered window stopped being presented: its frames got no swapchain texture, the frame system
  never ran, and 24 000 uploads waited while the stats kept reporting stale medians.
- **A blended draw covers the range from its first surviving slot to its last.** Its instance
  count used to run from slot zero, so a flight at maximum speed over ocean spent 0.65 ms of a
  0.74 ms world pass on degenerate water quads behind the camera (world pass 0.743 to 0.144 ms
  at 50 000 resident sections); the static view is unchanged at 0.305 ms and the draw order is
  the order the list held, so the picture is the same. The draw keeps a zero first instance and
  the vertex shader adds the start, which needs no optional feature on the web.
- **The drawn-triangle stat counts survivors.** It used to count every slot a blended draw
  spanned: 283 192 against 226 784 in the base view, 1 569 034 against 26 652 in flight.
- **`MCRS_LOOK` outlives the join teleport.** The server answered a join with the saved look and
  the knob had no effect under the integrated server.
- **A column is forgotten from the same view diff that sent it.** The area-of-interest system
  wrote the forgets, aimed at the dimension-local entity so the host dropped them (nothing was
  ever evicted: 6703 columns resident after 50 s at maximum speed against 1110 with forgets
  arriving, main-world median 0.63 to 0.43 ms), and once they arrived its radius differed from
  the column view's, so a turn left columns the client had forgotten and the server counted as
  sent: 6% of the view's sections meshed after turning round, 89% with the forget sent where the
  column view clears its sent set, as vanilla's chunk map does.
- **A section lists its distinct block states when it is decoded.** The loader scanned every
  block of an arriving column for unbaked states on the frame; reading the palette's list
  instead took `stream adopt` from 248 to 107 µs mean and 549 to 281 max.
- **Frame-time quantiles are selected, not sorted.** Sorting the 4096-frame window seven times
  cost 500 µs in the frame that read it; selecting costs 113.
- **The admission bounds stay at 32 sections a frame, 128 in flight and 4 MB of upload.** The
  pool returns about 32 meshes a frame and placing them costs 350 µs at worst; a settled flight
  uploads 300 to 450 KB a frame.
- **The chunk pipeline runs inside one tick and drains between ticks.** From ticket to sent a
  saved column crossed six systems on the 20 Hz schedule, 378 ms at the median and none of it
  work (73 µs to read, 87 to decode). Chaining the hops inside the tick took it to 164 ms, a
  `ColumnDrain` run every 2 ms of idle loop time to 55, and spawning and dispatching the rest
  of a column inside the drain to 7. Vanilla's `waitUntilNextTick` is the precedent.
- **Sends are capped at 64 columns and a quarter of the socket's byte cap a pass, light
  counted.** Ten a tick cost a row of 27 three ticks; 64 alone made a 4.2 MB blob that the
  bridge answered by closing the socket, and 4.6 MB once the light arrays were left out of the
  count. Vanilla starts at nine and ramps to 64 on the client's acknowledgements, which nothing
  sends here.
- **The drain rebuilds the column index before it sends.** The light packet walks that index,
  which the tick rebuilt once; a column sent within the tick had no sections in it and went out
  unlit, and the whole world drew black.
- **The tint window wraps around the world instead of sitting on spawn.** Grass 3000 blocks out
  sampled texels nothing had written and drew black; a column's square lands at its position
  modulo the window and the sampler repeats. The window must exceed the view's width.
- **The frame figures stand on the last second of frames.** The 4096-frame window behind the fps
  line and the engine spread lagged the picture by four to twelve seconds on the overlay; vanilla
  counts its fps over one second and its chart holds 240 frames, so the fps line is now that count
  and every quantile is taken over the frames of the last second. Reports take the median of the
  lines a settled run wrote instead of leaning on one long window.
- **The default render distance is 96 columns, and the server honours what the client asks
  for.** Vanilla's option stops at 32; the desktop asks for three times that and the browser
  keeps 12. At 96 the settled frame reads 3.1 ms engine and 10.9 ms GPU against the 1.0 ms
  goal (see PERF.md), so the budget binds long before the distance does; the figures are the
  starting point for the work, not a claim that it fits.
- **The client opts out of App Nap.** A window that was not the frontmost app dropped to the
  background scheduling band about a minute in, and the same frame read 0.7 ms before and 4 ms
  after; a latency-critical activity held for the life of the process keeps the priority.
- **The sight-line walk runs on the compute pool, without the frustum, when its inputs change.**
  On the frame it cost 1.6 ms at 96 columns (flood plus a scan of the laid box to project the
  bits); the loader now queues table edits and the walk runs on the pool whenever the camera's
  section or the topology changed, 3.1 ms of pool time for the whole box. Main world 2.2 to
  0.32 ms, engine 3.1 to 0.99 ms at 96 columns. A section laid while a walk is out is drawn
  until the next walk lands, which vanilla's occlusion graph also accepts.
- **The group table is written incrementally.** A stream keeps a block with room to grow, an
  arrival appends, an eviction leaves a record with no quads, and a rebuild into a fresh block
  happens only when a stream outgrows its block or half its records are dead. The whole-table
  rebuild was 56 MB on every arrival frame at 96 columns.
- **The translucent draws are packed by a prefix sum instead of spanning holes.** At 96
  columns their ordered draws walked 6.9 M triangles' worth of slots for 2.6 M survivors; a
  count, a scan over the batches and a scatter pack the survivors in list order. GPU world
  10.4 to 7.94 ms, cull 0.43 to 0.45. The picture is unchanged: the order the blend sees is
  the order the list held, as before.
- **Terrain quads are drawn as an indexed triangle list, not as instanced strips.** One draw
  of six vertices a quad with the quad read from the vertex index: GPU world 7.94 to 5.90 ms at
  96 columns for the same 15.3 M triangles; indexed through a fixed index buffer, so a quad is
  four vertices again, 4.27 ms. The index buffer follows the visible list's size, 24 MB per
  million slots. The picture is unchanged.
- **A blob the writer has no room for waits; it is never dropped.** The bridge handed each
  pass's encoded bytes to a four-deep channel and ignored a full one, so a client slower than
  the server's sends lost whole batches of columns that the server then counted as sent: 5 385
  of 38 025 columns never arrived at 96 columns with two clients loading on one machine, and
  the ring they left was drawn empty. Unsent blobs now queue in order ahead of anything newer,
  and a writer holding more than sixteen blobs at the socket's cap is a dead socket and kicked.
- **Column sends are paid out at vanilla's ceiling of 64 a tick as a continuous rate.** The cap
  was per pass, and the drain between ticks runs every 2 ms, so a tick could send thirty
  times vanilla's maximum and overrun the socket. At 1 280 columns a second a 96-column view
  arrives in 30 s instead of 15, and the meshing that follows takes 160 s either way.
- **Terrain is occlusion culled against a depth pyramid, in two passes.** Proven first as a
  counter: half of the drawn triangles at 96 columns (7.96 M of 15.3 M) were behind the last
  frame's terrain. The first cull tests against the last frame's pyramid and leaves what that
  test alone hides as candidates; the pyramid is rebuilt from this frame's depth and a second
  cull revives the candidates into a second set of draws, so nothing pops a frame late. GPU
  world 4.27 to 2.32 ms, the pyramid 0.07 and the second cull 0.30. The second pass packs the
  translucent quads it revives after the first pass's ordered ones, unordered among themselves;
  they are the few a turn uncovers, and the first pass keeps its order.
- **Dead section slots are swept through a set.** Every group was compared against a vector of
  dead slots, and a row leaving at once is 243 slots against 40 000 groups: the main world's
  worst settled frame at maximum speed was 10.7 ms, and is 0.95 with the set.
