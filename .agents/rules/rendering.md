---
paths:
  - "crates/mcrs_minecraft_{render,render_controller,render_probe,entity_render,light_render,mesh,model}/**"
  - "crates/mcrs_minecraft_{client,client_gui}/**"
  - "crates/mcrs_molang_wgsl/**"
---

# Rendering: what the CPU does, what the GPU does

Minimise GPU commands and per-frame CPU work, in that order, but measure before believing.

- **Evaluate environment attributes on the CPU.** It is on the order of a hundred scalar
  operations once per frame. On the GPU the same work runs per fragment, millions of times, for
  values constant across the whole draw.
- **Split the result in two.** Everything constant while the player stays in one dimension is
  baked into the shader as constants; everything that varies with time, camera or weather goes
  into one uniform written once per frame.
- **Specialize pipelines, do not branch at runtime.** The specialization key is
  `dimension_type.skybox` plus the enabled effects. With `skybox: none` a dimension has no sun,
  moon, stars or clouds, so those draws stop existing at pipeline build time. Recompiling on a
  dimension change is fine; shaders load dynamically.
- **Do not bake time-varying values.** `sky_color` changes every tick; baking it would mean
  rebuilding pipelines twenty times a second for no gain.
- Field names are not looked up by string id during a frame.
- **Frame budget ceilings.** Main world 0.15 ms, extract 0.10 ms, prepare and queue 0.15 ms,
  encode and submit 0.10 ms, GPU cull 0.05 ms, GPU terrain 0.65 ms, GPU sky pass 0.10 ms.
- **Sample the view depth through a nearest sampler, never load it.** GLSL ES holds a depth
  texture incomplete unless its filters are nearest, and a load from it reads zero.
- **Never evaluate a derivative such as `fwidth` behind a short-circuit.** WebGPU uniformity
  analysis rejects it and the whole module fails to validate.
- **The tint window wraps around the world.** A column lands at its position modulo the window
  and the sampler repeats. The window must exceed the view width.
