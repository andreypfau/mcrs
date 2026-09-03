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
