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
  core busy is a power decision for the product, and one still to be taken.
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
