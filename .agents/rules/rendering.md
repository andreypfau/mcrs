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
- Rendering depends on neither the network, the client nor the server. It never owns the
  window, the camera or the render graph of its host.
