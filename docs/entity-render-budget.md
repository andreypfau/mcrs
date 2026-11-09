# Entity render budget

This note gives a workload model for the entity passes of `docs/entity-render.md`. It computes
the cost of each pass for the pig and the frog, for 10,000, 100,000 and 300,000 mobs, on three
hardware tiers. It states the limit that each tier reaches first, and the design decisions that
the numbers force. The behaviour of a mob is in `docs/spec/`. This note does not change it.

Each number has one of three marks:

- **measured**: the probe of appendix A measured it on the reference machine;
- **source**: a document of section 0.3 states it;
- **estimate**: this note computes it from other numbers, and the arithmetic is next to it. An
  estimate for a tier that this note cannot measure also has the mark **not measured**.

## 0. Conventions

### 0.1 Terms

- **Tier**: a class of adapter with one frame target. Section 1 defines the tiers `M`, `D` and
  `W`.
- **Slot**: the persistent GPU record of one mob (`docs/entity-render.md`, section 4).
- **Visible mob**: a mob that the entity cull keeps for this frame.
- **Full rig**: a visible mob that the pose pass, the expand pass and the draw process with all
  its bones and cuboids.
- **Impostor**: a visible mob that the draw shows as one quad with a texture that the loader
  renders in advance.
- **Face**: one rectangle of a cuboid. The draw shows a face as two triangles with six vertices.
- **Face cost**: the GPU time to draw one face that covers almost no pixel. It contains the
  vertex shader, the primitive setup and the rasterizer.
- **Fill rate**: the number of gbuffer pixels that a tier writes in one second with the entity
  fragment shader: three colour targets of 20 B per pixel and a 32-bit depth target.
- **Projected height**: the height of a mob on the screen, in pixels:
  `h_px = h · H / (2 · d · tan(fov_y / 2))`. `h` is the height of the mob in blocks, `H` is the
  height of the viewport in pixels, and `d` is the distance in blocks.
- **Effective bandwidth**: the bytes per second that a copy kernel reads and writes in the
  memory of the GPU.
- **Full-rig cap**: the largest number of full rigs in one frame. The tier sets it.
- **Moving mob**: a mob whose tick record changes in a tick.
- **Packing**: the conversion of the ECS values of a mob into the bytes of its tick record.
- **Vertex pulling**: a vertex shader reads its data from storage buffers with the vertex index,
  not from vertex buffers.

### 0.2 Constants of this note

- The vertical field of view is 70°. Thus `H / (2 · tan 35°) = 1028` for `H = 1440`, and `771`
  for `H = 1080`. One block at the distance `d` is `1028 / d` pixels high at 1440p.
- The mob height `h` is 0.9 block for the pig and 0.55 block for the frog.
- The average projected area of one mob is 0.6 block² for the pig and 0.15 block² for the frog.
  The overdraw inside one mob is 1.3, for example a leg behind the body. These values are
  estimates from the cuboid sizes of MPIG-GEO-001 and MFROG-GEO-001.
- The tick rate is 20 Hz.
- A distance distribution that is uniform in area on a disk of radius `R` gives
  `E[1/d²] = 2 · ln(R / d0) / (R² − d0²)`, with `d0 = 2` blocks. The mean pixel count of one mob
  is `A · 1.3 · (H / (2 · tan 35°))² · E[1/d²]`.

### 0.3 Sources

| Id | Source |
|---|---|
| S1 | WebGPU, W3C Candidate Recommendation Draft of 15 September 2026, section "Limits", <https://www.w3.org/TR/webgpu/#limits> |
| S2 | WebGPU compatibility mode proposal, <https://github.com/gpuweb/gpuweb/blob/main/proposals/compatibility-mode.md>; Chrome 146 release note, <https://developer.chrome.com/blog/new-in-webgpu-146>; <https://webgpufundamentals.org/webgpu/lessons/webgpu-compatibility-mode.html> |
| S3 | web3dsurvey, WebGPU limits and features, <https://web3dsurvey.com/webgpu>, pages `limits/<name>` and `features/<name>`. The pages give no sample size and no date. They show core adapters only. |
| S4 | webgpureport.org, `index.js` lines 806–810 and 964–968, <https://github.com/webgpu/webgpureport.org/blob/main/index.js> |
| S5 | Haar, Aaltonen, "GPU-Driven Rendering Pipelines", SIGGRAPH 2015, <https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf> |
| S6 | Wihlidal, "Optimizing the Graphics Pipeline with Compute", GDC 2016, <https://archive.org/details/GDC2016Wihlidal> |
| S7 | Karis, Stubbe, Wihlidal, "A Deep Dive into Nanite Virtualized Geometry", SIGGRAPH 2021, <https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf> |
| S8 | wgpu 29.0.4, `wgpu-types/src/limits.rs`: `Limits::defaults`, `downlevel_defaults`, `downlevel_webgl2_defaults` |
| S9 | `PERF.md` of this repository, sections "GPU frame" and "Render distance" |
| S10 | Apple, MacBook Pro (M4 Max) technical data, <https://support.apple.com/en-us/121554>; WWDC20 session 10632, <https://developer.apple.com/videos/play/wwdc2020/10632/> |
| S11 | Intel, "The Compute Architecture of Intel Processor Graphics Gen9", section 5.8; Intel Core i5-8250U data, <https://www.intel.com/content/www/us/en/products/sku/124967/> |
| S12 | NVIDIA GeForce RTX 3060 data, <https://en.wikipedia.org/wiki/GeForce_30_series> |
| S13 | Chrome WebGPU developer features, <https://developer.chrome.com/docs/web-platform/webgpu/developer-features> |

## 1. Tiers and targets

| Tier | Adapter | Viewport | Frame target | Entity GPU share | Entity CPU share |
|---|---|---|---|---|---|
| `M` | Apple M4 Max, 40 GPU cores, Metal, native. The reference machine of S9. | 2560 × 1440 | 120 Hz, 8.33 ms | 2.0 ms (24%) | 1.0 ms |
| `D` | a mid-range discrete GPU, NVIDIA RTX 3060 class, native, Vulkan or DX12 | 2560 × 1440 | 144 Hz, 6.94 ms | 2.0 ms (29%) | 1.0 ms |
| `W` | Intel UHD Graphics 620 (Gen9.5 GT2, 24 EU), Chrome, WebGPU compatibility mode or core | 1920 × 1080 | 60 Hz, 16.7 ms | 4.0 ms (24%) | 2.0 ms, one thread |

The entity shares of `D` and `W` are the targets of `docs/entity-render.md`, section 8. The CPU
share of `W` is for one thread, because a wasm build without threads has one.

The capacity of each tier:

| Value | `M` | `D` | `W` |
|---|---|---|---|
| bandwidth, peak | 546 GB/s (source S10) | 360 GB/s (source S12) | 37.5 GB/s, shared with the CPU (source S11) |
| effective bandwidth | 407–425 GB/s copy, 436–520 GB/s read (measured, A.3) | 280 GB/s (estimate: 78% of the peak, the ratio of `M`; not measured) | 25 GB/s (estimate: 67% of the peak; the CPU uses the same bus; not measured) |
| FP32 arithmetic | 3.9 TFLOP/s achieved by the probe kernel (measured, A.3); about 16 TFLOP/s claimed by third parties, which is unverified | 12.7 TFLOP/s (source S12) | 0.42 TFLOP/s: 384 FLOP per clock at 1.1 GHz (source S11) |
| face cost | 0.60 ns at 6.3 M faces; 1.1–1.7 ns at 0.6–0.9 M faces (measured, A.5) | 0.4 ns (estimate: 4 triangles per clock is the setup maximum of current GPUs (source S7, p. 80), so 2 triangles at 1.78 GHz is 0.28 ns, plus the vertex shader; not measured) | 3.0 ns (estimate: 1 triangle per clock at 1.1 GHz is 1.8 ns per face; the vertex shader needs 6 × 60 lane operations of 192 per clock, 1.7 ns; not measured) |
| fill rate | 25–55 Gpx/s for faces of 8 to 16 px; 3.7 Gpx/s for faces of 2 px (measured, A.5) | 10 Gpx/s (estimate: 28 B per pixel at 280 GB/s; not measured) | 0.9 Gpx/s (estimate: 28 B per pixel at 25 GB/s; not measured) |

The tier `W` has no tile memory, so each gbuffer pixel goes to the shared memory. The 28 B per
pixel are the 20 B of the gbuffer, plus a depth read and a depth write. The tier `M` keeps the
gbuffer of a tile in the GPU, so its fill rate does not depend on the bandwidth.

A small triangle wastes the rasterizer. A rasterizer of 16 pixels per clock has 75% efficiency
for a triangle of 12 pixels and 6.25% for a triangle of 1 pixel (source S6, p. 51). The probe
shows the same effect on `M`: faces of 2 px fill at 3.7 Gpx/s, faces of 16 px at 52 Gpx/s.

## 2. Cost of one mob in each pass

### 2.1 Shapes

The counts come from `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_models.json`.

| Model | Bones | Cuboids | Cuboids of zero thickness | Faces after the expand pass | Vertices after the expand pass | Vertices of an instanced cuboid mesh |
|---|---|---|---|---|---|---|
| pig (`minecraft:pig`, `main`) | 6 | 7 | 0 | 7 × 3 = 21 | 126 | 7 × 24 = 168, with 84 triangles |
| pig with saddle | 6 + 6 | 7 + 7 | 0 | 42 | 252 | 336 |
| frog (`minecraft:frog`, `main`) | 16 | 16 | 7 | 9 × 3 + 7 × 1 = 34 | 204 | 9 × 24 + 7 × 8 = 272 |

A box shows at most three faces to a distant camera. A plane of zero thickness shows one face,
or two when the selected face has a cutout texel (`docs/entity-render.md`, section 5.3). The
frog figures assume no cutout texel in the planes. With a cutout texel in each plane, the frog
has 41 faces.

Over all 225 `main` layers of the fixture, the median model has 8 bones and 9 cuboids, the 90th
percentile has 12 bones and 12 cuboids, and the largest is the ender dragon with 36 bones and 65
cuboids (measured from the fixture, without its outer root node). The pig is a small model and
the frog is a large one.

### 2.2 Bytes and operations per pass

The record sizes are those of `docs/entity-render.md`: tick record 64 B, state record 48 B,
input record 96 B, bone matrix 48 B, face record 8 B.

| Pass | Unit | Reads | Writes | Arithmetic |
|---|---|---|---|---|
| CPU tick write | moving mob, per tick | the ECS components | 64 B tick record into the mirror | packing |
| upload | moving mob, per tick | 64 B | 64 B | none |
| cull | slot, per frame | 64 B tick + 32 B of the state record = 96 B | 8 B for a visible mob | 5 planes, one pyramid read, about 60 operations |
| pose, pig | visible mob | 64 + 48 = 112 B | 6 × 48 = 288 B | 6 bones × (one channel + 3 sines + 3 cosines + 27 for the matrix) ≈ 450 operations |
| pose, frog | visible mob | 64 + 48 + 96 = 208 B | 16 × 48 = 768 B | 16 bones × (3 curves × about 35 + 70 for the matrix + 36 for the parent) ≈ 3,400 operations |
| expand, pig | visible mob | 6 × 48 + 48 = 336 B | 21 × 8 = 168 B | 7 cuboids × about 25 |
| expand, frog | visible mob | 16 × 48 + 48 = 816 B | 34 × 8 = 272 B | 16 cuboids × about 25 |
| draw, pig | visible mob | 21 × 8 + 6 × 48 + 48 = 504 B | the pixels | 126 vertices × about 60 |
| draw, frog | visible mob | 34 × 8 + 16 × 48 + 48 = 1,088 B | the pixels | 204 vertices × about 60 |

A pass is limited by the bandwidth when `bytes / bandwidth` is larger than
`operations / arithmetic rate`. On `W`, the pig pose is `400 B / 25 GB/s = 16 ns` against
`450 / 0.25 TFLOP/s = 1.8 ns`. Thus the bandwidth limits each compute pass of each tier. The
frog pose on `M` is the exception: the probe measures 3.2 ns per mob, against 2.4 ns from the
bandwidth (A.4).

### 2.3 Time per mob

`M` is measured with the probe kernels of A.4 and A.5, at 300,000 mobs. `D` and `W` are
estimates from section 1 and are not measured: bytes divided by the effective bandwidth, and
faces times the face cost.

| Pass, ns per mob | `M` pig | `M` frog | `D` pig | `D` frog | `W` pig | `W` frog |
|---|---|---|---|---|---|---|
| cull (each slot) | 0.25 | 0.25 | 0.34 | 0.34 | 3.8 | 3.8 |
| pose | 0.97 | 3.22 | 1.4 | 3.5 | 16 | 39 |
| expand | 1.04 | 2.37 | 1.8 | 3.9 | 20 | 44 |
| draw, faces only | 12.6 | 20.4 | 8.4 | 13.6 | 63 | 102 |
| sum for a full rig | 14.9 | 26.2 | 11.9 | 21.3 | 103 | 189 |

The draw is the largest part of the GPU time of a full rig on each tier: 85% for the pig and
78% for the frog on `M`, 61% for the pig on `W`. On `M` and `D`, the face cost sets the number of
full rigs. On `W`, the face cost and the bandwidth of the pose and the expand set it together.

### 2.4 Time per pass for N mobs

Each mob is visible and is a full rig. This is the worst case of the current design, which has
no tiers. The fill is in section 2.5.

| Tier, model | N | cull | pose | expand | draw, faces | sum |
|---|---|---|---|---|---|---|
| `M` pig | 10,000 | 0.003 | 0.005 (measured) | 0.008 (measured) | 0.126 | 0.14 ms |
| `M` pig | 100,000 | 0.019 (measured) | 0.079 (measured) | 0.077 (measured) | 1.26 | 1.43 ms |
| `M` pig | 300,000 | 0.074 (measured) | 0.292 (measured) | 0.312 (measured) | 3.80 (measured) | 4.48 ms |
| `M` frog | 10,000 | 0.003 | 0.047 (measured) | 0.016 (measured) | 0.204 | 0.27 ms |
| `M` frog | 100,000 | 0.019 | 0.299 (measured) | 0.221 (measured) | 2.04 | 2.58 ms |
| `M` frog | 300,000 | 0.074 | 0.966 (measured) | 0.712 (measured) | 6.12 | 7.87 ms |
| `D` pig | 10,000 / 100,000 / 300,000 | 0.003 / 0.034 / 0.10 | 0.014 / 0.14 / 0.43 | 0.018 / 0.18 / 0.54 | 0.084 / 0.84 / 2.52 | 0.12 / 1.19 / 3.59 ms |
| `D` frog | 10,000 / 100,000 / 300,000 | 0.003 / 0.034 / 0.10 | 0.035 / 0.35 / 1.05 | 0.039 / 0.39 / 1.17 | 0.136 / 1.36 / 4.08 | 0.21 / 2.13 / 6.40 ms |
| `W` pig | 10,000 / 100,000 / 300,000 | 0.038 / 0.38 / 1.15 | 0.16 / 1.60 / 4.80 | 0.20 / 2.02 / 6.05 | 0.63 / 6.30 / 18.9 | 1.03 / 10.3 / 30.9 ms |
| `W` frog | 10,000 / 100,000 / 300,000 | 0.038 / 0.38 / 1.15 | 0.39 / 3.90 / 11.7 | 0.44 / 4.35 / 13.1 | 1.02 / 10.2 / 30.6 | 1.89 / 18.8 / 56.6 ms |

The `M` draw of 300,000 pigs is the probe draw of 6.3 M faces: 3.80 ms (A.5). For a small count,
the probe shows a fixed cost: the frog pose at 10,000 mobs is 4.7 ns per mob, against 3.2 ns at
300,000, because 157 workgroups do not fill 40 cores.

### 2.5 Raster and gbuffer fill

The mean pixel count of one visible mob depends on the distance distribution (section 0.2):

| Distribution | `H` | pig, px per mob | frog, px per mob |
|---|---|---|---|
| uniform on a disk, `R` = 32 (scene S4) | 1440 | 5,300 | 1,330 |
| uniform on a disk, `R` = 32 | 1080 | 3,000 | 750 |
| uniform on a disk, `R` = 256 | 1440 | 105 | 26 |
| uniform on a disk, `R` = 256 | 1080 | 59 | 15 |
| uniform on a disk, `R` = 512 (scenes S2, S3) | 1440 | 37 | 9 |
| a field of 316 × 316 blocks, camera at a corner (scene S1) | 1440 | about 70 | about 18 |
| a field of 316 × 316 blocks, camera at a corner (scene S1) | 1080 | about 40 | about 10 |

Fill time is `N · px / fill rate`. Examples:

- `W`, 100,000 pigs in S1 at 1080p: `100,000 · 40 px / 0.9 Gpx/s = 4.4 ms`. This is more than the
  full entity share of `W`. The nearest pigs give most of the mean, so two seeds of the sample
  give 38 and 41 px.
- `W`, 10,000 frogs in S4 at 1080p: `10,000 · 750 px / 0.9 Gpx/s = 8.3 ms` with no depth
  rejection. The depth test rejects a part of the hidden points. With a random draw order and an
  average depth complexity `c`, a pixel shades `ln(c) + 0.58` times. Here `c = 7.5 M / 2.07 M =
  3.6`, so a pixel shades 1.86 times, and the fill is `2.07 M · 1.86 / 0.9 Gpx/s = 4.3 ms`.
- `M`, 100,000 pigs in S1 at 1440p: `100,000 · 70 / 50 Gpx/s = 0.14 ms`.

The pixels of a far mob are expensive. In S1 at 1440p, 92% of the pigs have a mean face area of
less than 4 px². A face of 0.04 block² (the mean pig face) covers 4 px² at
`d = 1028 · √(0.04 / 4) = 103` blocks. Beyond that distance, the face cost and not the fill rate
sets the draw time, and the rasterizer efficiency is below 25% (S6, p. 51).

### 2.6 CPU and upload

| Item | Value | Mark |
|---|---|---|
| pack a 32 B tick record as a `#[repr(C)]` struct into the mirror, at a random slot, and set the dirty bit of its page | 2.1 ns per mob at 10,000 and 100,000; 2.8 ns at 300,000 | measured, A.2 |
| the same record, packed byte by byte with `copy_from_slice` | 24–30 ns per mob | measured, A.2 |
| read the ECS components of a moving mob | 5–8 ns per mob | estimate: two or three cache misses; not measured |
| `queue.write_buffer`, CPU time | 0.12–0.31 ms for 1 MiB; 0.53–1.27 ms for 6 MiB; 1.2–2.1 ms for 13 MiB | measured, A.2 |
| `queue.write_buffer`, then submit and wait | 2.5–7.3 ms for 13 MiB | measured, A.2 |
| `writeBuffer` in a browser | an extra copy out of the wasm memory and a transfer to the GPU process | estimate; not measured |

Upload per tick, with the 64 B tick record of `docs/entity-render.md`, section 4.1:

| Moving mobs | 64 B record | 32 B record |
|---|---|---|
| 10,000 | 0.64 MB | 0.32 MB |
| 50,000 | 3.2 MB | 1.6 MB |
| 120,000 | 7.7 MB | 3.8 MB |
| 300,000 | 19.2 MB | 9.6 MB |

On `M`, 120,000 moving mobs give 7.7 MB (7.3 MiB) in one frame of each tick. The CPU time for
`write_buffer` is about 0.7–1.5 ms for that size, and the packing adds `120,000 · 10 ns =
1.2 ms`. That is more than the CPU share of 1.0 ms.

## 3. Where each tier reaches a limit

| Tier | First limit | Number | Next limit |
|---|---|---|---|
| `M` | face cost of the draw | 2.0 ms / 14.9 ns = 134,000 full-rig pigs, or 2.0 ms / 26.2 ns = 76,000 full-rig frogs | CPU upload in the tick frame: 7.7 MB at 64 B per record takes 0.7–1.5 ms, and the packing 1.2 ms more (section 2.6) |
| `D` | face cost of the draw (not measured) | 2.0 ms / 11.9 ns = 168,000 pigs, or 94,000 frogs | fill of a near crowd: S4 at 1440p is 13 M px at 10 Gpx/s = 1.3 ms |
| `W` | face cost and bandwidth together (not measured) | 4.0 ms / 103 ns = 39,000 full-rig pigs, or 4.0 ms / 189 ns = 21,000 frogs, with no fill | fill: 0.9 Gpx/s gives 3.6 M px in 4 ms, which is 1.7 screens at 1080p; the cull of 300,000 slots alone is 1.15 ms |
| `W` | upload | the target of 1.5 MB per tick (`docs/entity-render.md`, section 8) allows 23,000 moving mobs at 64 B, or 47,000 at 32 B | the main thread of wasm does the packing and the upload |
| `W` compatibility mode | binding | `maxStorageBuffersInVertexStage` is 0 by default (source S1, S2). The vertex stage reads no storage buffer | `maxTextureDimension2D` is 4096 and `maxComputeInvocationsPerWorkgroup` is 128 in compatibility mode (source S1) |

These limits of the web do not bind the entity passes:

| Limit | Guaranteed value | Distribution | Use in the design |
|---|---|---|---|
| `maxStorageBuffersPerShaderStage` | 8 (S1) | ≥ 8 on 100%, ≥ 10 on 98% of adapters (S3) | the Full vertex stage binds 5; the pose binds 6 when the static tables share one buffer |
| `maxStorageBuffersInVertexStage` | 8 in core, 0 in compatibility mode (S1) | ≥ 10 on 99% of core adapters (S3); about 45% of the devices that compatibility mode targets have none (S2) | the Compatibility path |
| `maxStorageBufferBindingSize` | 128 MiB (S1) | ≥ 256 MiB on 98%, ≥ 1 GiB on 92% (S3) | the bone arena at the full-rig cap, section 4.2 |
| `maxComputeWorkgroupStorageSize` | 16,384 B (S1) | ≥ 32 KiB on 99% (S3) | the workgroup scan of the cull and the expand needs less than 1 KiB |
| `maxTextureArrayLayers` | 256 (S1) | ≥ 2048 on 99% (S3) | the entity texture arrays |
| `maxColorAttachmentBytesPerSample` | 32 (S1) | | the gbuffer uses 20 B; 12 B remain |
| `timestamp-query` | optional | 98.6% (S3) | the gates of section 5 |
| `indirect-first-instance` | optional | 99.98% of core adapters (S3) | not needed |
| multi-draw-indirect | none | `chromium-experimental-multi-draw-indirect` on 0.14% (S3) | one `draw_indirect` per texture array |

The adapter `M` reports 31 storage buffers per stage, a binding size of 4 GiB, 2048 array
layers and 32 KiB of workgroup memory. It reports `TIMESTAMP_QUERY` and
`INDIRECT_FIRST_INSTANCE`, but not `TIMESTAMP_QUERY_INSIDE_PASSES` and not
`MULTI_DRAW_INDIRECT_COUNT` (measured, A.1).

## 4. Decisions that the numbers force

### 4.1 Tier selection in the cull: full rig or impostor

**Decision.** The entity cull gives each visible mob a tier from its projected height. A mob
with `h_px ≥ 12` is a full rig. A mob with `h_px < 12` is an impostor: one face, no pose and no
expand. The cull appends the mob to the visible list of its tier. This belongs in the core
design now, because it sets the outputs of the cull, the dispatch arguments of the pose and the
size of the bone arena. The impostor pipeline can come later. Until then, an impostor draws as a
full rig.

**Numbers.**

- The draw is 61–85% of the cost of a full rig (section 2.3).
- 12 px is the pig at 77 blocks at 1440p and at 58 blocks at 1080p. It is the frog at 47 blocks
  at 1440p and at 35 blocks at 1080p.
- Fraction of visible mobs that stay full rigs at 1440p: 4.6% of the pigs in S1, 2.2% of the pigs
  and 0.8% of the frogs in S2.
- S1, 100,000 pigs: `W` goes from 14.7 ms to 5.3 ms, `M` from 1.62 ms to 0.29 ms. The rest on
  `W` is fill: 4.4 ms for 4.0 M px.
- S2, 84,000 visible mobs: `W` goes from 13.0 ms to 2.9 ms, `D` from 1.54 ms to 0.38 ms.

A mob of less than 12 px has faces of less than 4 px² on average (section 2.5). A 2 px leg is
inside the position tolerance of CEREN (0.5 px) only in the impostor view at its own size. Thus
the impostor needs a new spec rule (section 6.2).

Not selected: a compute rasterizer for small faces (S7, p. 81, "3x faster than hardware" for
triangles with edges below 32 px). It needs 64-bit atomics, which the web does not have
(`SHADER_INT64_ATOMIC_MIN_MAX` is native only, A.1).

### 4.2 Full-rig cap and the size of the bone arena

**Decision.** Each tier has a full-rig cap. The bone arena, the face list and the Compatibility
bone texture have the size of the cap, not of the slot count. When the full rigs of a frame are
more than the cap, the cull raises the height threshold of 4.1 for the next frame. The CPU
computes the threshold from the full-rig count of the last frame, which it reads back a few
frames late.

| Tier | Full-rig cap | Bone arena at the cap, frog, 48 B per bone |
|---|---|---|
| `M` | 75,000 | `75,000 · 16 · 48 = 58 MB` |
| `D` | 75,000 | 58 MB |
| `W` | 20,000 | `20,000 · 16 · 48 = 15 MB`; as a bone texture, `20,000 · 16 · 3` texels fit in 1024 × 1024 `rgba32float` (16 MiB) |

**Numbers.** The caps are the frog figures of section 3, rounded down: 76,000 for `M` and
21,000 for `W`. With no cap, 300,000 frogs need `300,000 · 16 · 48 = 230 MB` of bone matrices.
That is more than the guaranteed binding size of 128 MiB (S1). A bone texture for the slot count
in compatibility mode is 4096 × 4096 `rgba32float`, which is 256 MiB.

### 4.3 Pose rate by distance: not on the GPU paths

**Decision.** On the Full and the Compatibility paths, the pose pass computes the pose of each
full rig in each frame. It keeps no pose of an earlier frame. On the Minimal path, the CPU can
compute the pose of a far mob at a lower rate (`docs/entity-render.md`, section 6.1).

**Numbers.** The pose is 6.5% of the GPU time of a full-rig pig on `M` (0.97 of 14.9 ns) and 16%
of a frog (3.22 of 26.2 ns). After 4.1, the pose of S1 is 0.004 ms on `M` and 0.04 ms on `W`. A
lower rate would need a persistent bone store for each slot: 300,000 · 16 · 48 B = 230 MB.

### 4.4 Shared poses for identical state: not now

**Decision.** The design does not share a pose between mobs. An impostor already shares its
image with all mobs in the same view and walk phase.

**Numbers.** A rest-pose shortcut for idle pigs saves the 288 B write of the pose and the bone
reads of the expand pass. In S1 with 50% idle pigs and no tiers, this is about 0.07 ms of
1.62 ms on `M`: half of 72% of the pose (0.097 ms) and half of 57% of the expand (0.104 ms).
After 4.1, it is less than 0.005 ms.

### 4.5 Vertex-expanded faces, also on the Compatibility path

**Decision.** The Full path keeps the expand pass and the vertex pulling of face records. The
Compatibility path also uses the face records: the expand pass writes them to a buffer with the
usages `STORAGE | VERTEX`, and the draw reads them as a vertex buffer with the step mode
`Instance`, six vertices per instance. The vertex stage reads the cuboids and the bones with
`textureLoad`. A draw for one texture array finds its part of the face list with the offset of
`set_vertex_buffer`, so it does not need `indirect-first-instance`.

**Numbers.**

- A pig after the expand pass has 126 vertices and 42 triangles. An instanced cuboid mesh has
  168 vertices and 84 triangles, and the hardware culls 42 of them after the setup (section 2.1).
- On `M`, the expand costs 1.04 ns per pig. The vertex rate is about 10 G per second (A.5, and
  30 M vertices in 2.9 ms for terrain, S9). Thus the expand path costs `1.04 + 126 · 0.1 =
  13.6 ns` against `168 · 0.1 = 16.8 ns` and the setup of 42 more triangles.
- On `W`, the setup is about one triangle per clock (estimate, not measured). The instanced mesh
  doubles the triangles: 10,000 pigs need 0.84 M triangles instead of 0.42 M, which is 0.38 ms
  more at 1.1 GHz.

A risk that this note does not measure: some GPUs pack the vertices of small instances badly
into waves. A draw of six vertices per instance can have a low wave occupancy on such a GPU.

### 4.6 Occlusion: one phase after the terrain, no mob in the pyramid

**Decision.** The entity cull stays where `docs/entity-render.md`, section 5, puts it: after
`WorldPass::OpaqueSecond`, against the pyramid of the current frame. Mobs do not write the
pyramid, and the entity passes have no second phase.

**Numbers.**

- The two-phase scheme of S5 (p. 51–52) and S7 (p. 19) exists because the first phase tests
  against the pyramid of the previous frame. The entity cull tests against the pyramid of the
  current frame, so it has no false negative that a second phase must correct.
- A second pyramid with mobs costs one more build (0.073 ms, S9) and a second cull of the
  candidates.
- Mobs are poor occluders in the pyramid. The cull reads the mip in which the rectangle of the
  mob is at most 2 × 2 texels. For a pig at 50 blocks at 1440p (18 px high), the texel is 16 px,
  which is 0.8 block. Pigs at a density of one per block² have gaps of about 0.4 block. Each
  texel thus contains ground behind the crowd, and the farthest depth of the texel hides
  nothing.
- The depth test of the draw removes most hidden pixels of a crowd. With a depth complexity of
  3.6, a pixel shades 1.86 times (section 2.5).

### 4.7 Camera-relative coordinates

**Decision.** Keep the integer block position of the tick record, and subtract the camera
section of `CameraOrigin` (`crates/mcrs_minecraft_render/src/frame.rs`) in `i32` before a
conversion to `f32`. The pose and the bone matrices stay in internal space of the entity. The
vertex shader adds the entity transform relative to the camera. This agrees with the terrain.

**Numbers.** The step of an `f32` at 10⁵ blocks is 0.0078 block, which is 0.125 model unit. The
position tolerance of EANIM-GPU-003 is `1e-3` model unit, 125 times less. At the world border
(3 · 10⁷ blocks) the step is 2 blocks. The fraction of 1/65536 block is 2.4 · 10⁻⁴ model unit,
which is inside the tolerance.

### 4.8 Previous bone palette for motion vectors: not now

**Decision.** The renderer has no TAA, and the gbuffer writes zero motion
(`crates/mcrs_minecraft_render/src/shaders/include/deferred.wgsl`). No document of the
repository plans TAA. The bone storage keeps the room for a second copy, as
`docs/entity-render.md`, section 5.2, states. The pose pass writes only the current pose.

**Numbers.** A previous palette doubles the bone writes of the pose: 288 B more for each pig.
At the full-rig cap of `W` (20,000 frogs), that is `20,000 · 768 B = 15 MB` per frame, 0.6 ms at
25 GB/s. With the tiers of 4.1 it is less than 1 MB in S1 and S2.

### 4.9 Overlay in the gbuffer, translucent layers in the forward pass

**Decision.** The red overlay and the white flash stay in the entity fragment shader that writes
the gbuffer (`docs/entity-render.md`, section 5.5). They need no pass and no target. A
translucent layer (CEREN-VIS-001, item 3) draws in `WorldPass::Forward` from the same face
records, with back faces culled and with blending.

**Numbers.**

- The gbuffer is 20 B per pixel. The WebGPU limit is 32 B per sample (S1), so at most 12 B of
  targets can come later. Compatibility mode allows 4 colour targets (S1); the gbuffer uses 3.
- A translucent face costs the face cost again and a blended fill. A spectator who looks at
  10,000 invisible pigs on `W` adds `10,000 · 21 · 3 ns = 0.63 ms`. The translucent layers
  therefore use the full-rig cap and the tiers of 4.1 like the opaque layers.

### 4.10 A 32 B tick record, and the previous values on the GPU

**Decision.** The CPU writes only the current values of the tick record: block position `i32 ×
3`, fraction `u16 × 3`, body yaw `u16`, head yaw `u16`, pitch `snorm16`, walk position `f32`,
walk speed `f32`. That is 32 B. The client tick of a record goes in a word of its page. The tick
records always go through the scatter pass of `docs/entity-render.md`, section 4.7. For each
dirty slot, the scatter pass first copies the current record to the previous record, then
writes the new record. The scatter pass also writes the cull record of 4.11.

**Numbers.**

- The previous values of the 64 B record are the current values of the previous upload. The
  upload sends them twice.
- `W` at 1.5 MB per tick: 23,000 moving mobs at 64 B, 47,000 at 32 B (section 3).
- `M`, S2 with 120,000 moving mobs: 7.7 MB at 64 B, 3.8 MB at 32 B. `write_buffer` needs about
  0.15 ms per MiB (A.2), thus 1.1 ms and 0.55 ms.
- The scatter pass on `W` reads 32 B and the old 32 B and writes 64 B for each moving mob. For
  47,000 mobs, that is 6 MB, 0.24 ms, in the frame of each tick.

### 4.11 A cull record of 16 B

**Decision.** The cull reads a separate array of 16 B for each slot: block position `i32 × 3`,
cull radius `f16`, and a flags word `u16` with the validity bit. The cull sphere has its centre
at the centre of the block and its radius grows by `√3 / 2` block, so the cull needs no
fraction, no previous position and no partial tick. The CPU writes the cull record when the
block position, the radius or the validity changes.

**Numbers.** The cull reads 96 B per slot today. For 300,000 slots on `W`, that is 29 MB per
frame, 1.15 ms of the 4.0 ms share, also when few mobs are visible (S3: 1.15 of 1.33 ms after
4.1). A 16 B record gives 4.8 MB and 0.19 ms. A mob that walks crosses a block boundary every 5 to
10 ticks, so the cull record adds about 2–3 B per moving mob and tick to the upload.

### 4.12 Packing of the tick record on the CPU

**Decision.** The tick systems write a record as one `#[repr(C)]` `Pod` struct into the mirror.
They do not build it from byte slices. On native, they split the work by page across the
compute task pool when more than 50,000 mobs move in a tick.

**Numbers.** 2.1–2.8 ns per mob for the struct store, against 24–30 ns per mob for byte packing
(A.2). With the estimated 5–8 ns to read the ECS components, 300,000 moving mobs need 2.1–3.2 ms
of one thread per tick, which is more than the CPU share of `M` and `D`. 50,000 mobs need
0.35–0.55 ms.

### 4.13 Atomics of the expand pass

**Decision.** The expand pass reserves the face records of a workgroup with one global
`atomicAdd`, after a scan inside the workgroup, like the cull.

**Numbers.** On `M`, the two forms cost the same: 0.30–0.31 ms with one atomic per workgroup and
0.31–0.35 ms with one atomic per lane, for 2.1 M lanes (A.4). Thus the aggregation costs
nothing on `M`. On a GPU that serializes atomics on one address, 2.1 M atomics at one per clock
and 1.5 GHz take 1.4 ms (estimate, not measured).

### 4.14 Measurement on Metal and in Chrome

**Decision.** The gates of section 5 time the compute passes with pass timestamps. They time
the entity draw on Metal as the difference of two runs, with and without the entity draw, under
`MCRS_GPU_HOT` (S9). In Chrome, a gate reads the timestamps only with the developer flag, or it
takes the mean of at least 64 frames.

**Numbers.** The adapter `M` has no `TIMESTAMP_QUERY_INSIDE_PASSES` (A.1). In the probe, the
timestamps at the bounds of a render pass read an end before the start in some runs, and the
median and the minimum of the same draw differ by 3 to 10 times (A.5). Chrome quantizes a
timestamp to 100 µs without the flag `enable-webgpu-developer-features` (S13). Third-party
reports state other steps for Chrome 148 and later; they are unverified.

### 4.15 The Metal System Trace is the ruler of the draw on `M`

**Decision.** On `M`, the draw time comes from a Metal System Trace, not from the difference of
two wall times. Each pass of `examples/stress_field.rs` has a label, so the trace gives the GPU
time of each pass:

```sh
cargo build --release -p mcrs_minecraft_entity_render --example stress_field
python3 -I tools/metal-trace/metal_trace.py record /tmp/s1.trace -- \
    target/release/examples/stress_field
python3 -I tools/metal-trace/metal_trace.py report /tmp/s1.trace --process stress_field --skip 60
```

`record` runs `xcrun xctrace record --template 'Metal System Trace'`. `report` reads the table
`metal-gpu-intervals` and prints the median vertex, fragment and compute time of each label, and
the `span` of each pass: the time from the start of its vertex work to the end of its fragment
work. The frames alternate between `gbuffer entities` and `gbuffer no entities`.
`MCRS_DRAW_ONLY=full_rigs` or `impostors` keeps one draw, and `MCRS_SPLIT_DRAW` puts the two
draws in two passes. A second pass loads and stores the gbuffer, so its time includes that
traffic.

**Numbers.**

- The wall-time difference does not measure the draw. For the same build, it read 1.35 ms with
  a heater of 1 workgroup, and 0.05 ms with the heater of 16384 workgroups. The trace read a span
  of 0.41 ms.
- The vertex interval of a pass that waits for an earlier pass starts at the wait. Read the
  vertex time of a pass only when no other work runs at the same time.
- A Metal System Trace records one GPU counter, "RT Unit Active". `xctrace record` has no option
  that selects a counter set: `--show-recording-options` lists none for `Metal GPU Counters`. To
  get the limiter counters, save a template with a counter set from the Instruments application
  and pass its path to `--template`.
- `start_graphics_debugger_capture` of wgpu 29 starts a capture for an attached debugger. With
  `MTL_CAPTURE_ENABLED=1` and no Xcode, it writes no `.gputrace`. The per-shader cost needs Xcode:
  run the program from Xcode and capture a GPU workload.
- `MTL_HUD_ENABLED=1` shows the GPU time of each frame of the client.

S1 on `M`, medians of 256 frames, with the heater; the draw is the span of `gbuffer entities`:

| Step | Draw span | Vertex | Fragment | Full rigs only | Impostors only |
|---|---|---|---|---|---|
| first impostors | 0.770 ms | 0.427 | 0.329 | 0.339 | 0.502 |
| a prepare pass writes one record for each impostor | 0.619 | 0.263 | 0.335 | | 0.323 |
| the full rigs draw one instance of a four-vertex strip for each face | 0.447 | 0.162 | 0.281 | 0.239 | |
| faces with no cutout texel draw with no discard | 0.410 | 0.164 | 0.246 | 0.185 | |
| impostors under 3 px draw a rectangle of their covered area, with no discard | 0.409 | 0.163 | 0.245 | | 0.271 |

The compute passes add 0.030 ms (cull), 0.030 ms (pose), 0.037 ms (expand) and 0.035 ms
(impostor prepare). The scatter reads 0.02 ms without the heater; with the heater, its timestamps
overlap the heater.

Choices that the trace rejects:

| Choice | Number |
|---|---|
| an impostor drawn as one instance of a four-vertex strip | vertex 0.087 ms against 0.057 ms for six vertices with no instance |
| impostors in two lists, near before far, for the depth test | fragment 0.218 ms against 0.209 ms |
| an opaque inner quad before the quad that discards | fragment 0.269 ms against 0.209 ms, and twice the vertex time |
| a depth-only pass with the discard, then a colour pass with no discard | the depth-only pass alone takes 0.161 ms, the cost of the whole discard pass |
| a coverage mesh of rectangles instead of the discard | the discard of the impostors from 3 to 12 px costs about 0.16 ms; a mesh of four rectangles for 70,000 impostors adds about 1.7 M vertices, about 0.17 ms at 10 G vertices per second |
| a box with no discard for impostors under 6 px | fragment 0.149 ms, but a FLIP error of 0.18 to 0.31 at 4 px |

The impostors of 3 px and less cost 0.06 ms of fragment time, those from 3 to 6 px 0.10 ms, and
those from 6 to 12 px 0.10 ms. With no discard at all, the fragment time of the impostors is
0.10 ms: the discard costs more than the shading.

## 5. Stress scenes and gates

### 5.1 Scenes

A debug spawner creates the client entities from a seed, so a scene needs no server
(`docs/entity-render.md`, section 8). Each scene has a fixed camera, a fixed seed and a fixed
number of ticks. A mob that walks follows a seeded random path at the walk speed of its type.

| Scene | Mobs | Distance distribution | Animation state | Occlusion | What it loads |
|---|---|---|---|---|---|
| S1 field | 100,000 pigs, 10% with a saddle | flat field of 316 × 316 blocks, one pig per block²; camera at a corner, at the eye height 1.62, toward the diagonal | 50% walk, 50% stand, head yaw from the path | none from terrain; mob on mob only | the draw, the fill and the tiers |
| S2 spread | 300,000 mobs: 70% pigs, 30% frogs | uniform on a disk of `R` = 512 around the camera, flat world; 28% in the frustum | 40% walk; 10% of the frogs in water | none | the cull of all slots, the upload, the tiers |
| S3 occluded | the mobs of S2 | the mobs of S2, in hilly terrain with caves; camera in a valley | as S2 | terrain hides about 97% | the pyramid test and the cull cost with few visible mobs |
| S4 crowd | 10,000 frogs | uniform on a disk of `R` = 32 | each frog changes its pose (`croaking`, `using_tongue`, `long_jumping`) every 40 ticks with a seeded phase; 50% in water | mob on mob | the fill, the input records, the `State` channels on the CPU |

The model results for these scenes, with the tiers of 4.1, are:

| Scene | Tier | Visible | Full rigs | Faces | Entity GPU time, model |
|---|---|---|---|---|---|
| S1 | `M` / `D` / `W` | 100,000 | 4,600 / 4,600 / 2,500 | 0.19 M / 0.19 M / 0.15 M | 0.29 / 0.83 / 5.3 ms |
| S2 | `M` / `D` / `W` | 84,000 | 1,600 / 1,600 / 900 | 0.12 M / 0.12 M / 0.10 M | 0.19 / 0.38 / 2.9 ms |
| S3 | `M` / `D` / `W` | 9,000 | 160 / 160 / 95 | 0.013 M / 0.013 M / 0.011 M | 0.09 / 0.13 / 1.3 ms; 0.37 ms on `W` with 4.11 |
| S4 | `M` / `D` / `W` | 10,000 | 10,000 | 0.34 M | 0.54 / 1.5 / 10.2 ms; 6.2 ms on `W` with the depth rejection of 2.5 |

S1 and S4 on `W` are over the share of 4.0 ms. The fill causes it: 4.0 M px in S1 and 7.5 M px
in S4. A gate for `W` therefore uses a scene size that the tier can hold: S1 with 50,000 pigs and
S4 with 4,000 frogs.

### 5.2 Metrics

For each scene and each run:

- the frame time, p50 and p99 over 4,096 frames, after 10 s of warm-up;
- the GPU time of each entity pass: entity cull, pose, expand and draw, as section 4.14 states;
  the median over 256 frames;
- the CPU time of each entity system, from the Tracy zones (`--features telemetry-tracy`) or the
  Bevy diagnostics: slot allocation, tick write, state write, input write, the batch VM, the
  render-world upload;
- the counts: slots, visible mobs, full rigs, impostors, emitted faces, bytes uploaded per tick.

The probe runs with `MCRS_HOT=1` and `MCRS_GPU_HOT=16384` on `M`, as S9 states.

### 5.3 Gates

A gate fails when a median is more than the limit. The limits are the targets of section 1 and
the model numbers of 5.1 with a margin of 50%. Each gate becomes a CI gate when the scene and the
entity passes exist.

| Gate | `M` | `D` | `W` |
|---|---|---|---|
| S1, all entity passes on the GPU, p50 | ≤ 0.45 ms | ≤ 1.2 ms | ≤ 4.0 ms with 50,000 pigs |
| S2, entity cull with the cull record of 4.11 | ≤ 0.03 ms | ≤ 0.04 ms | ≤ 0.3 ms |
| S2, entity cull with the 64 B tick record | ≤ 0.11 ms | ≤ 0.15 ms | ≤ 1.7 ms |
| S2, all entity passes on the GPU, p50 | ≤ 0.3 ms | ≤ 0.6 ms | ≤ 4.0 ms |
| S3, all entity passes on the GPU, p50 | ≤ 0.15 ms | ≤ 0.2 ms | ≤ 0.6 ms |
| S4, all entity passes on the GPU, p50 | ≤ 0.8 ms | ≤ 2.0 ms | ≤ 4.0 ms with 4,000 frogs |
| entity CPU per frame, frames with no tick, p50 | ≤ 0.2 ms | ≤ 0.2 ms | ≤ 0.5 ms |
| entity CPU per frame, frames with a tick, p50 | ≤ 1.0 ms | ≤ 1.0 ms | ≤ 2.0 ms |
| upload per tick, S2 | ≤ 4.0 MB | ≤ 4.0 MB | ≤ 1.5 MB |
| frame time p99 / p50 | ≤ 2.0 | ≤ 2.0 | ≤ 2.0 |

Only the gates of `M` can run on the reference machine. The gates of `D` and `W` need such an
adapter in CI, or a forced path on native (`docs/entity-render.md`, section 6). A forced path
tests the code of a path, not the speed of a tier.

## 6. Edits that the numbers require

### 6.1 `docs/entity-render.md`

These edits are in this change. Each one follows only from a number of this note.

1. Section 6.1, row "Compatibility": "a bone texture of 4096 × 4096 texels holds 1.3 M bones"
   is wrong. At 3 texels per bone, 4096 × 4096 texels hold 5.6 M bones and use 256 MiB. The row
   now gives the size from the full-rig cap.
2. Section 8: a reference to this note, and the rule for the time measurement of the draw on Metal (4.14).

These edits need a decision of the owner of the design, so this change does not make them:

3. Section 4.1: the tick record of 32 B with the previous record on the GPU (4.10).
4. Section 4.1 or a new section 4.7: the cull record of 16 B (4.11).
5. Section 4.6: the tick records always go through the scatter pass, which rotates the previous
   record (4.10).
6. Section 5.1: the cull selects the tier of 4.1 and appends to one visible list per tier. Add
   the height threshold to the frame uniforms, and its feedback from the full-rig count (4.2).
7. Section 5.2: the pose dispatch covers only the full rigs. The bone arena has the size of the
   full-rig cap (4.2).
8. Section 5.3: one global `atomicAdd` per workgroup, after a scan inside the workgroup (4.13).
9. Section 6, row "Compatibility": the face records as an instance-rate vertex buffer, not an
   instanced cuboid mesh (4.5). Integer varyings use `@interpolate(flat, either)`, because
   compatibility mode has no other flat mode (S2). Entity workgroups have at most 128
   invocations (S1).
10. Section 6.1, column "Visible mobs, target": replace it with the full-rig caps of 4.2: 75,000
    on `M` and `D`, 20,000 on `W`. Impostors have no cap.
11. Section 8: replace the benchmark scene with the scenes and gates of section 5.
12. Section 9: move the tier selection of the cull (4.1) into step 2. Add the impostor pipeline as
    a step after step 4.
13. Section 10: remove "Tiers by distance" and "Impostors for the horizon" from the deferred
    items. Keep "a lower update rate for far mobs" there only for the Minimal path (4.3).

### 6.2 Specifications

The specifications do not change in this note. These rules need a decision of the
specification author:

1. `client-entity-render.md`: a new rule for the fidelity of a small entity. Below a projected
   height of 12 px at the test resolution, the client can draw the entity with less detail: an
   image from a fixed set of views and animation phases. The parity tests of section 0.2 (0.5 px,
   2 of 255) then apply only to entities at or above that height. Kind: semantic. Source: 4.1.
2. `entity-animation.md`, EANIM-GPU-003: item 2 states that the limits apply "whichever
   processor evaluates a slot". Add an item: the limits apply to an entity that the client draws
   with its full rig. An entity under the rule of item 1 has no pose to compare. Source: 4.1.
3. `entity-animation.md`, EANIM-GPU-003, item 3: "for example because the slots of the type
   exceed a GPU budget". Add the full-rig cap as such a budget: over the cap, the client draws
   the entity with less detail under item 1, and does not compute its pose on the CPU. Source:
   4.2.
4. `client-entity-render.md`, CEREN-VIS-002, item 5: the cull of 4.11 uses a sphere about the
   centre of the block, with the radius grown by `√3 / 2` block. This sphere contains the culling
   volume, so the rule allows it. No edit is necessary. Confirm it in the provenance line.
5. `client-entity-render.md`, CEREN-VIS-001, item 3: a translucent entity uses the tiers of 4.1.
   The rule of item 1 must also cover translucent entities. Source: 4.9.
6. `docs/molang.md`, section 10.2: the tick record of 4.10 holds the current values only. The
   table row for `lerp`, `alerp` and `walk` says that "the record holds the current and the
   previous value". Change it to "the GPU keeps the previous record". This is a design document,
   not a specification.

## Appendix A. Probe output on `M`

The probe is `crates/mcrs_minecraft_render_probe/examples/entity_budget.rs`:

```
cargo run --release -p mcrs_minecraft_render_probe --example entity_budget
```

It requests the adapter with `PowerPreference::HighPerformance` and all its limits. It times a
GPU pass with timestamps at the bounds of the pass. A calibration run selects the repetition
count that makes one submission about 25 ms long; the figures are per repetition, the median
and the minimum over 9 submissions. The data is synthetic, with the record sizes of
`docs/entity-render.md`. The probe ran on 2026-11-07, release build, wgpu 29.0.4, macOS, while
other builds ran on the same machine (load average 40 to 87). Thus the figures are lower bounds
of the speed of `M`. A second run without the calibration gave the same bandwidth and the same
arithmetic rate within 5%.

### A.1 Adapter

```
name: "Apple M4 Max", device_type: IntegratedGpu, backend: Metal
subgroup_min_size: 4, subgroup_max_size: 64
features (WebGPU): DEPTH_CLIP_CONTROL | DEPTH32FLOAT_STENCIL8 | TEXTURE_COMPRESSION_BC |
  TEXTURE_COMPRESSION_BC_SLICED_3D | TEXTURE_COMPRESSION_ETC2 | TEXTURE_COMPRESSION_ASTC |
  TEXTURE_COMPRESSION_ASTC_SLICED_3D | TIMESTAMP_QUERY | INDIRECT_FIRST_INSTANCE | SHADER_F16 |
  RG11B10UFLOAT_RENDERABLE | BGRA8UNORM_STORAGE | FLOAT32_FILTERABLE | FLOAT32_BLENDABLE |
  DUAL_SOURCE_BLENDING | IMMEDIATES | PRIMITIVE_INDEX
features (wgpu): SHADER_FLOAT32_ATOMIC | TEXTURE_FORMAT_16BIT_NORM | TEXTURE_COMPRESSION_ASTC_HDR |
  TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES | TIMESTAMP_QUERY_INSIDE_ENCODERS |
  MAPPABLE_PRIMARY_BUFFERS | TEXTURE_BINDING_ARRAY | STORAGE_RESOURCE_BINDING_ARRAY |
  SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING |
  STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING | PARTIALLY_BOUND_BINDING_ARRAY |
  ADDRESS_MODE_CLAMP_TO_ZERO | ADDRESS_MODE_CLAMP_TO_BORDER | POLYGON_MODE_LINE |
  VERTEX_WRITABLE_STORAGE | CLEAR_TEXTURE | MULTIVIEW | TEXTURE_ATOMIC | EXTERNAL_TEXTURE |
  EXPERIMENTAL_RAY_QUERY | SHADER_INT64 | SUBGROUP | SUBGROUP_BARRIER |
  SHADER_INT64_ATOMIC_MIN_MAX | TEXTURE_INT64_ATOMIC | EXPERIMENTAL_MESH_SHADER |
  PASSTHROUGH_SHADERS | SHADER_BARYCENTRICS | EXPERIMENTAL_COOPERATIVE_MATRIX |
  MEMORY_DECORATION_COHERENT
not present: TIMESTAMP_QUERY_INSIDE_PASSES, MULTI_DRAW_INDIRECT_COUNT
downlevel flags: COMPUTE_SHADERS | FRAGMENT_WRITABLE_STORAGE | INDIRECT_EXECUTION | BASE_VERTEX |
  READ_ONLY_DEPTH_STENCIL | NON_POWER_OF_TWO_MIPMAPPED_TEXTURES | CUBE_ARRAY_TEXTURES |
  COMPARISON_SAMPLERS | INDEPENDENT_BLEND | VERTEX_STORAGE | ANISOTROPIC_FILTERING |
  FRAGMENT_STORAGE | MULTISAMPLED_SHADING | DEPTH_TEXTURE_AND_BUFFER_COPIES |
  WEBGPU_TEXTURE_FORMAT_SUPPORT | BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED | UNRESTRICTED_INDEX_BUFFER |
  FULL_DRAW_INDEX_UINT32 | DEPTH_BIAS_CLAMP | VIEW_FORMATS | UNRESTRICTED_EXTERNAL_TEXTURE_COPIES |
  SURFACE_VIEW_FORMATS | NONBLOCKING_QUERY_RESOLVE | SHADER_F16_IN_F32
shader model: Sm5
```

Limits: the adapter, then the wgpu values of S8 (`defaults` is WebGPU core, `downlevel` is GLES
3.1 and D3D11, `webgl2` is GLES 3.0 and WebGL2). wgpu 29 has no limit for the storage buffers in
the vertex stage. It reports their absence with the downlevel flag `VERTEX_STORAGE`.

```
limit                                                adapter      defaults   downlevel      webgl2
max_storage_buffers_per_shader_stage                      31             8           4           0
max_storage_buffer_binding_size                   4294967292     134217728   134217728           0
max_buffer_size                                  86586540032     268435456   268435456   268435456
max_uniform_buffer_binding_size                   4294967295         65536       16384       16384
max_storage_textures_per_shader_stage                    128             4           4           0
max_sampled_textures_per_shader_stage                    128            16          16          16
max_texture_array_layers                                2048           256         256         256
max_texture_dimension_2d                               16384          8192        2048        2048
max_texture_dimension_3d                                2048          2048         256         256
max_bind_groups                                            8             4           4           4
max_bindings_per_bind_group                            65535          1000        1000        1000
max_dynamic_storage_buffers_per_pipeline_layout            4             4           4           0
max_compute_workgroup_storage_size                     32768         16384       16352           0
max_compute_invocations_per_workgroup                   1024           256         256           0
max_compute_workgroup_size_x                            1024           256         256           0
max_compute_workgroups_per_dimension                   65535         65535       65535           0
max_color_attachments                                      8             8           4           4
max_color_attachment_bytes_per_sample                    128            32          32          32
max_inter_stage_shader_variables                          31            16          15          15
max_vertex_buffers                                        16             8           8           8
max_vertex_attributes                                     31            16          16          16
max_immediate_size                                      4096             0           0           0
min_storage_buffer_offset_alignment                      256           256         256         256
min_uniform_buffer_offset_alignment                      256           256         256         256
```

The compatibility-mode defaults of WebGPU are in S1: `maxStorageBuffersInVertexStage` 0,
`maxStorageTexturesInVertexStage` 0, `maxStorageBuffersInFragmentStage` 4,
`maxComputeInvocationsPerWorkgroup` 128, `maxTextureDimension2D` 4096, `maxColorAttachments` 4,
`maxUniformBufferBindingSize` 16384, `maxInterStageShaderVariables` 15. The other limits of
section 3 have the core value. A native adapter cannot report these limits through wgpu 29.

### A.2 CPU

```
# CPU upload: queue.write_buffer + submit + wait, wall clock
  1 MiB  write_buffer   0.119–0.313 ms  with submit and wait   1.39–1.70 ms
  6 MiB  write_buffer   0.529–1.270 ms  with submit and wait   1.79–6.13 ms
 13 MiB  write_buffer   1.207–2.078 ms  with submit and wait   2.47–7.29 ms
 26 MiB  write_buffer   2.293–3.821 ms  with submit and wait   3.57–5.49 ms

# CPU tick records: pack and scatter into the slot mirror, one thread
mobs   10000 64 B record, byte packing     0.255 ms median   25.47 ns/mob
mobs   10000 32 B record, byte packing     0.268 ms median   26.76 ns/mob
mobs   10000 32 B record, struct store     0.021 ms median    2.12 ns/mob
mobs  100000 64 B record, byte packing     2.578 ms median   25.78 ns/mob
mobs  100000 32 B record, byte packing     2.676 ms median   26.76 ns/mob
mobs  100000 32 B record, struct store     0.214 ms median    2.14 ns/mob
mobs  300000 64 B record, byte packing     8.968 ms median   29.89 ns/mob
mobs  300000 32 B record, byte packing     7.416 ms median   24.72 ns/mob
mobs  300000 32 B record, struct store     0.851 ms median    2.84 ns/mob
```

The upload rows give the range over four runs.

### A.3 Bandwidth and arithmetic

```
copy (read + write)       1.318 ms median    1.263 min    407.3 GB/s median   425.1 GB/s best
read only                 0.616 ms median    0.581 min    435.6 GB/s median   462.2 GB/s best
fma f32                   8.869 ms median    8.705 min     3873.9 GFLOP/s median    3947.0 best
scalar mul-add f32        4.423 ms median    4.393 min     3884.1 GFLOP/s median    3910.6 best
sin + cos f32             2.884 ms median    2.854 min      744.5 G trig/s median     752.5 best
```

The buffers are 256 MiB each. The copy reaches 75–78% of the peak of 546 GB/s (S10). The
arithmetic kernels reach 3.9 TFLOP/s with 16 or 8 independent chains for each lane. The
third-party figure of about 16 TFLOP/s is unverified, and this note does not use it. The other
load on the machine can explain a part of the difference.

### A.4 Entity compute passes

The probe no longer has the synthetic kernels of A.4 and A.5. The `stress_field` example of
`mcrs_minecraft_entity_render` times the production passes. A.4 and A.5 record the run of
2026-11-07.

```
cull   slots   10000 kept  300‰   0.0034 ms median  0.0032 min    0.34 ns/slot
cull   slots  100000 kept  300‰   0.0189 ms median  0.0186 min    0.19 ns/slot
cull   slots  300000 kept  300‰   0.0836 ms median  0.0802 min    0.28 ns/slot
cull   slots  300000 kept 1000‰   0.0741 ms median  0.0706 min    0.25 ns/slot
pose   pig           mobs   10000   0.0050 ms median  0.0047 min    0.50 ns/mob
pose   pig           mobs  100000   0.0794 ms median  0.0701 min    0.79 ns/mob
pose   pig           mobs  300000   0.2918 ms median  0.2859 min    0.97 ns/mob   411.3 GB/s
pose   pig + saddle  mobs   10000   0.0066 ms median  0.0064 min    0.66 ns/mob
pose   pig + saddle  mobs  100000   0.1627 ms median  0.1541 min    1.63 ns/mob
pose   pig + saddle  mobs  300000   0.6189 ms median  0.5884 min    2.06 ns/mob   333.5 GB/s
pose   frog          mobs   10000   0.0471 ms median  0.0467 min    4.71 ns/mob
pose   frog          mobs  100000   0.2993 ms median  0.2782 min    2.99 ns/mob
pose   frog          mobs  300000   0.9655 ms median  0.9392 min    3.22 ns/mob   273.4 GB/s
expand pig   workgroup atomic mobs   10000 lanes    70000   0.0082 ms median  0.0080 min    0.82 ns/mob
expand pig   workgroup atomic mobs  100000 lanes   700000   0.0769 ms median  0.0733 min    0.77 ns/mob
expand pig   workgroup atomic mobs  300000 lanes  2100000   0.3118 ms median  0.3046 min    1.04 ns/mob
expand pig   lane atomic      mobs  300000 lanes  2100000   0.3503 ms median  0.3098 min    1.17 ns/mob
expand frog  workgroup atomic mobs   10000 lanes   160000   0.0156 ms median  0.0149 min    1.56 ns/mob
expand frog  workgroup atomic mobs  100000 lanes  1600000   0.2207 ms median  0.2156 min    2.21 ns/mob
expand frog  workgroup atomic mobs  300000 lanes  4800000   0.7121 ms median  0.6848 min    2.37 ns/mob
expand frog  lane atomic      mobs  300000 lanes  4800000   0.7135 ms median  0.6749 min    2.38 ns/mob
```

The GB/s column is the bytes of section 2.2 divided by the median. At 10,000 and 100,000 mobs,
the records stay in the cache, so the column is not a memory figure there. The pose kernel of
the frog composes each bone with one kept parent; the real pose reads the parent from the bone
arena, which the cache holds.

### A.5 Gbuffer draw

Vertex pulling of face records into the three gbuffer targets and a 32-bit depth target at
2560 × 1440, with a `discard` test in the fragment shader. The faces lie on a grid of squares
with the side in pixels. A side of 0.25 px gives triangles that cover almost no sample. The time
is the wall time per repetition, because the timestamps of a render pass on `M` do not bracket
its work (section 4.14).

```
side  0.25 px faces   630000 verts   3780000 covered  0.00 screens   wall 1.063 ms  1.69 ns/face
side  0.25 px faces  6300000 verts  37800000 covered  0.00 screens   wall 3.795 ms  0.60 ns/face
side     2 px faces   921600 verts   5529600 covered  1.00 screens   wall 0.979 ms  1.06 ns/face   3.76 Gpx/s
side     4 px faces   230400 verts   1382400 covered  1.00 screens   wall 0.292 ms  1.27 ns/face  12.62 Gpx/s
side     4 px faces   921600 verts   5529600 covered  4.00 screens   wall 0.999 ms  1.08 ns/face  14.76 Gpx/s
side     8 px faces    57600 verts    345600 covered  1.00 screens   wall 0.143 ms  2.49 ns/face  25.73 Gpx/s
side    16 px faces    14400 verts     86400 covered  1.00 screens   wall 0.071 ms  4.94 ns/face  51.81 Gpx/s
side    16 px faces    57600 verts    345600 covered  4.00 screens   wall 0.270 ms  4.68 ns/face  54.70 Gpx/s
side    16 px faces    57600 verts    345600 covered  4.00 screens   no discard   wall 0.269 ms  54.90 Gpx/s
side    64 px faces     3600 verts     21600 covered  4.00 screens   wall 0.191 ms 53.19 ns/face  77.01 Gpx/s
```

The fragment shader has a `discard` that no texel takes. The hidden surface removal of the GPU
stops for a shader with `discard` (S10, WWDC20 session 10632), so these figures include each
covered pixel of each layer. The row with no `discard` reads the same. At 6.3 M faces, the draw
reaches 10 G vertices per second, the rate that S9 measured for terrain.
