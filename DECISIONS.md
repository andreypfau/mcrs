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
