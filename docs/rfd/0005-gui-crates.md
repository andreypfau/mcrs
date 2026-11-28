# RFD 5: Split the client GUI into crates by screen

- Authors: andreypfau
- State: prediscussion
- Labels: architecture, client, gui

This page proposes to replace the crate `mcrs_minecraft_client_gui` with eighteen small crates.
It also proposes a retained GUI scene and a screen model that a dialog screen can use. Read it
if you touch a screen of the client, if you plan the dialog screen, or if you wonder why the GUI
is not one crate.

## The problem

The crate `mcrs_minecraft_client_gui` has one flat `src/` and its only consumer is
`mcrs_minecraft_client`. Section "What is known" gives the measures. The maintainer has three
goals for it, in this order.

1. A clean and simple architecture that obeys `CLAUDE.md`.
2. Fewer findings of `deslop-please` and fewer expectations of the ECS lint, with no new
   exception.
3. Modular code that a user can take in parts, and that an agent can read in parts.

The crate misses each goal today.

- **One crate for every screen.** `src/` holds 156 entries. `deslop-please` reports
  `wide-directory` at its default threshold of 15 entries. The finding is a symptom: no
  dependency and no boundary of meaning separates the parts.
- **Field leaks.** `deslop-please` reports each field that code outside its type reads. The
  central modules and `ClickState` cause 11 of them.
- **An immediate-mode scene.** `begin_gui_frame` cleared the scene on each frame. Each draw
  system then pushed its quads again, and the batch was built, cloned and uploaded again. Most
  lint expectations of the crate followed from this. Step 2 ends it.
- **A draw order in the schedule.** `GuiPhase` had five sets, and the order of two draws in one
  set was the order of the systems. A title screen draws a menu and a logo. A world screen draws
  a menu and a message. The chat draws under a container screen. Each case needed a new `after`.
- **A fixed screen enum.** `Screen` in `mcrs_minecraft_client_world` has 19 variants, and 28
  files of the GUI match on it. Three central modules (`next_screen`, `follow_menu_events`,
  `project_menu`) know every screen. A dialog that the server shows in the configuration phase
  has no place in that enum. `docs/spec/dialog.md` needs a previous screen, an Escape that a
  screen can refuse and a screen that the network replaces (sections 11 and 13).
- **An atlas that knows the screens.** `gui_atlas_load` reads the creative tabs of
  `menu_geometry`, and `menu_background` lists the menu type of every container. A new screen
  with its own background changes the atlas.
- **A heavy dependency list.** A change to the title screen rebuilds chat, creative and the F3
  screen. A user of one screen pays for all.

## What is known

Each fact has the way it was found. The values are the state of the working tree during step 2.

| Fact | Source |
|---|---|
| 156 entries in `src/`: 155 Rust files and one shader | `ls crates/mcrs_minecraft_client_gui/src \| wc -l` |
| 25,369 lines of Rust | `cat crates/mcrs_minecraft_client_gui/src/*.rs \| wc -l` |
| 0 module cycles, 32 field leaks, 2 `wide-directory` (`crates/` and `src/`) | `deslop-please mcrs_minecraft_client_gui` |
| 60 normal dependencies | `crates/mcrs_minecraft_client_gui/Cargo.toml` |
| 55 `pub mod`, 26 import paths in `mcrs_minecraft_client` | `src/lib.rs`, `grep -rho 'client_gui::[a-z_]*' crates/mcrs_minecraft_client/src` |
| 16 expectations of the ECS lint, 7 of `clippy::too_many_lines` | `grep -n 'mcrs::\|too_many_lines' src/*.rs` |
| `Screen` is matched in 28 GUI files and 2 files of `mcrs_minecraft_client` | `grep -l 'Screen::'` |
| `ChatInput(pub ChatScreen)` is a resource of `client_world` | `mcrs_minecraft_client_world/src/chat.rs` |
| `GuiPointer` is `Option<IVec2>`, `None` outside the window | `src/gui_pointer.rs` |

`docs/ecs-lint.md` section 10 lists the event conditions. A closure never counts. A same-crate
function that returns a condition counts. A system with an event condition is not checked for
`alloc_in_tick`.

The 16 expectations of the ECS lint, their cause and the step that ends each:

| Lint | Where | Cause | Step |
|---|---|---|---|
| `alloc_in_tick` (7) | `debug_screen` (2), `debug_entry_fps` (2), `debug_entry_position`, `debug_entry_network`, `debug_entry_system_specs` | The refresh condition is a closure, so the lint does not see an event condition. | 2 |
| `alloc_in_tick` (1) | `chat_input` | The chat key system reads the keys on each frame and allocates when the chat opens or closes. | 9 |
| `alloc_in_tick` (3) | `follow_menu_events`, `follow_create_world`, `world_rows` | A system reads the screen on each frame and allocates on a rare branch. | 11 |
| `alloc_in_tick` (2) | `chunk_map`, `inventory_clicks` | The F6 view while it shows; a copy of the selected search text. Step 9 moves the second to `creative_inventory_clicks`. | stays |
| `ungated_system` (1) | `inventory_clicks` | `ClickState` copies the screen and the open menu on each frame. | 9 |
| `blocking_in_system` (1) | `debug_entry_fps` | Frame times are wall-clock instants. | stays |
| `marker_toggle` (1) | `mark_creative_screen` | A tick system inserts and removes `CreativeScreen` on the player. | 9 |

The field leaks fall into seven groups. Section "Field leaks" names the owner and the fix of
each.

## Determination

The maintainer decided these points. The numbers are the numbers of this page.

1. **The GUI splits into crates.** The maintainer rejected the option to keep one crate and
   allow the findings.
2. **The font is its own resource and its own crate**, `mcrs_minecraft_font`, in layer 3. It is
   no field of the atlas. It has no dependency on `client_jar`. The caller gives the font bytes,
   so a map viewer can take the crate alone. `mcrs_minecraft_client` reads the jar. The font
   and the atlas read pack files through `PackFiles` of a new Bevy-free crate
   `mcrs_minecraft_resource_pack` (layer 3), not through `ResourceSet` of
   `mcrs_minecraft_model`. That crate also holds the PNG decoder and the missing texture
   (`missing_pixels`, `MISSING_SIZE`). It needs `std`, because `png` 0.18 has no `no_std` build:
   its API is on `std::io`.
3. **The scene and the atlas know no screen.** No module of the scene or of the atlas names a
   screen, and no screen orders itself against another screen.
4. **A screen is an entity.** Its kind is a component that the crate of the screen owns. The
   stack keeps a previous screen, an Escape that a screen can refuse and replacement by the
   network.
5. **The screen stack is GUI infrastructure in layer 6.** Its crate is `mcrs_minecraft_gui_screen`.
   It has no network and no client world, so it is not layer 7. It uses Bevy components and
   observers, so it is not layer 3. A crate of layer 6 may depend on another crate of layer 6.
6. **`gui_chat` owns the chat key.** `gui_screen` has the constant `CHAT_KEY`. The chat key
   opens the chat only when the stack is empty (CHAT-OPEN-001). No screen allows the chat above
   it, so no marker for it exists. The container crates do not depend on `gui_chat`.
7. **`gui_render` is separate.** A headless client and a bot take no wgpu.
8. **`sign_text` moves into `mcrs_minecraft_client`.** It lays out sign text and draws no GUI.
9. **The migration lands as one pull request.** Each step is one commit in it, and each commit
   builds and passes the tests. A step may touch any crate group. The maintainer is a code owner,
   so the limits on the size and the crate groups of a pull request do not apply.
10. **The scene is retained.** Each screen or HUD entity owns a `GuiLayer`. A draw system writes
    its layer only on a change. The order is the key of the layer, not the order of the systems.
    `GuiPhase` shrinks to `Draw` and `Batch`, and keeps its name.
11. **The atlas is a Bevy `Image` asset** with `RenderAssetUsages::RENDER_WORLD`. The main world
    rasterises a missed glyph into `GlyphUploads`. `FontCells` and `GuiAtlasImage` end.
    `Font::unifont_cell` is the one path to a unifont cell.
12. **A dependency or a natural boundary of meaning forces a split.** No crate exists for a
    topic alone. 15 entries is the default threshold of `deslop-please`, not a rule of the
    project. It is a soft guide. A crate above it splits only where a natural boundary exists.
    If no boundary exists, the `wide-directory` finding of that crate is accepted. No crate is
    provisional, and no review after a step decides the map. The skill `rust-style` says to split
    a crate only when a dependency forces the split. By the decision of the maintainer, this
    determination overrides that rule for this RFD.
13. **Redesign a part before its move. Move a part as soon as its redesign is done.** A pure move
    step edits only `use` paths and `Cargo.toml`. Step 6 is a redesign step, not a pure move: it
    creates `resource_pack` and changes the font loader. A move waits for the crates that the
    part depends on. Section "Migration" gives the order.
14. **The chat draws in the HUD band.** It shows under a container screen and under the creative
    screen.
15. **The F3 screen is its own crate**, `mcrs_minecraft_gui_debug`, in layer 7. `gui_hud` loses
    the F3 modules and the dependencies `render_probe` and `volume`. This RFD fixes only the
    crate boundary and moves today's modules as they are. The inner design of the F3 screen will
    be rebuilt later from its own specification in `docs/spec/`.
16. **The title opens the world list through a stored function pointer.** `gui_session` defines
    the resource `SingleplayerEntry(fn(&mut Commands))`. The plugin of `gui_worlds` inserts it
    with a function that spawns the world list. The title shows the singleplayer button only
    when the resource exists. So `gui_session` does not depend on `gui_worlds`, and a client that
    only joins servers does not take `save`. A function pointer needs no registration and no
    `dyn`, so it is simpler than a `SystemId`.
17. **The render systems run only on a change.** `prepare_gui_pass` runs on a change of the
    target format, of a terrain atlas or of a GUI image. `write_gui_buffers` runs on a new batch.
    `upload_glyphs` runs while uploads wait. `write_gui_uniform` runs on a change of the batch, of
    the glint or of the viewport. A named condition with `mcrs::event_condition` tests each change.
18. **Each crate that can be `#![no_std]` with `alloc` is.** The column `no_std` of the crate
    table gives the result. `container_gesture` and `gui_screen` are `no_std`. `font` is
    not, because its dependencies `text`, `lang` and `nbt` need `std`.
19. **The chat fades on the GUI tick, not on real time.** `docs/spec/client-chat.md` CHAT-LOG-003
    gives the rule. The resource `GuiTick` of `mcrs_minecraft_client_world` holds the counter.
    `GuiTickPlugin` owns it and advances it in `FixedUpdate`. Each chat line and the action bar
    store the GUI tick at which they arrived. `Chat::tick` and `advance_tick` end. `GuiTick` also
    drives the heart shake. The gate of the chat layer is in section "The retained scene".
20. **The background belongs to the kind of the screen, and the world is a fact of the client.**
    The paragraph "The background" of section "The screen model" defines the enum component
    `Backdrop` and lists its variants.
21. **The atlas stitches what the screens request.** The atlas takes a list of stitch requests.
    A request names the source textures and the target region. The crate of each screen
    registers its requests in its plugin. Section "The atlas and the render world" gives the
    design.
22. **The inventory figure is its own crate**, `mcrs_minecraft_gui_figure`, in layer 7. It
    takes `entity_render` and `pose`, so `gui_container` without the figure takes no entity
    renderer. The figure names no screen. It reads the component `ShowsFigure` of the top screen.
23. **`GuiPointer` follows WID-PTR-005 and CPR-FIG-003.** The pointer is the last position that
    the platform reported. Section "GuiSize and GuiPointer" gives the rule.

## Proposal

### Layers and names

The names follow `CLAUDE.md`. The prefix `mcrs_minecraft_gui_` groups the GUI crates. The font,
the atlas and the scene are useful without a client, so the word `client` is not in their names.
Each crate keeps private modules and re-exports its public names at the root. One public path
exists for each item.

| Crate | Layer | `no_std` | Job | What forces it |
|---|---|---|---|---|
| `mcrs_minecraft_resource_pack` | 3 | no: `png` | `PackFiles`, the PNG decoder, the missing texture | Dependency: the font and the atlas read pack files without `mcrs_minecraft_model`, which takes Bevy and `client_jar` (determination 2). |
| `mcrs_minecraft_font` | 3 | no: `text`, `lang`, `nbt` | Font data, glyph cache, text layout, wrap, translation | Dependency: no Bevy. A sign renderer or a map viewer takes it alone. |
| `mcrs_minecraft_container_gesture` | 3 | yes | The state machine of the click, drag and quick-move gestures of a container, and the drag preview value | Dependency: no Bevy. A bot takes it without a GUI. |
| `mcrs_minecraft_gui_atlas` | 6 | no: Bevy asset, image | The GUI image assets, the atlas layout and load, the sprites, the stitch requests | Dependency: `resource_pack` and Bevy image, and no window. Boundary: the images of a pack, apart from the frame. |
| `mcrs_minecraft_gui_scene` | 6 | no: Bevy window | Layers, GUI size, pointer, phases, fade, the generic draw gates, the F3 chord | Dependency: Bevy window and `client_widget`. Boundary: the frame that each draw writes into. |
| `mcrs_minecraft_gui_primitives` | 6 | no: Bevy | Text, tooltip, item icon, decorations, sprite draw, tile, box, the in-game gradient | Dependency: the font layout and the item crates (`item_stack`, `item_table`, `registries`, `model`). The scene takes none of them. |
| `mcrs_minecraft_gui_render` | 6 | no: wgpu | The batch, the pipeline, the extract, the background and the panorama view | Dependency: wgpu (determination 7). |
| `mcrs_minecraft_gui_screen` | 6 | yes, checked in step 9 | The screen stack, the shared parts of a screen, `Backdrop`, `WorldShown`, `ShowsFigure` | Dependency: Bevy ECS, app and input only (determination 5). A dialog takes it. |
| `mcrs_minecraft_gui_menu` | 6 | no: Bevy | The host of a widget menu: layout state, draw, input, cursor | Boundary: the widget host that every screen crate and a dialog share. It reads no client world. |
| `mcrs_minecraft_gui_chat` | 7 | no: Bevy | Chat lines, chat screen, chat key, suggestions | Dependency: `chat_client` with `bevy`, `command`. |
| `mcrs_minecraft_gui_figure` | 7 | no: Bevy | The inventory figure: placement, projection, pose | Dependency: `pose`, `entity_visual` and `entity_render` (determination 22). |
| `mcrs_minecraft_gui_container` | 7 | no: Bevy | Container screens, hotbar, item tooltip, container backgrounds, the Bevy side of the gestures | Dependency: `container_gesture`, the inventory engine crates, `item_tooltip`, `player_holders`. |
| `mcrs_minecraft_gui_creative` | 7 | no: Bevy | The creative screen | Dependency: `creative_content`, `creative_report`. |
| `mcrs_minecraft_gui_hud` | 7 | no: Bevy | Health, game mode switcher | Boundary: the parts that draw over a world with no screen, and the switcher of the chord F3 and F4. Its dependencies `damage` and `entity_attribute` are not unique to it. |
| `mcrs_minecraft_gui_debug` | 7 | no: Bevy UI | F3 screen, chunk map, light levels | Dependency: `render_probe`, `volume`, Bevy UI and text (determination 15). |
| `mcrs_minecraft_gui_options` | 7 | no: Bevy | The options screen and its sub screens | Dependency: `client_options`, and no `network`. Boundary: the settings of the client. A tool that shows only settings takes it. |
| `mcrs_minecraft_gui_session` | 7 | no: Bevy | Death, disconnect, level loading, message, the session flow, title, pause | Dependency: `network`. The pause button and the death screen send `LeaveRequest`. |
| `mcrs_minecraft_gui_worlds` | 7 | no: Bevy | World list, world create, world delete | Dependency: `save`. |

`gui_menu`, `gui_hud` and `gui_options` rest on a boundary: the widget host, the HUD of a world
and the settings of the client. `gui_options` stays apart from `gui_session`, because
`gui_session` takes `network`.

Layer 6 depends on no network, no client world and no server, so `gui_render` and `gui_menu` may
depend on `gui_screen`. The screen crates read the client world and the network, so they sit in
layer 7, as does the later crate `mcrs_minecraft_gui_dialog` (section "Dialog readiness").

### Dependency graph

An arrow means "depends on". The graph shows the main direct edges between GUI crates. The
dependency lists of section "What each crate holds" give every edge.

```
resource_pack <- font <- atlas <- scene <- primitives <- menu <- chat
                                    ^          ^          +-- container <- creative
                                    |          |          +-- options <- session <- worlds
                                    |          |          +-- hud
                                    |          |          +-- dialog (later)
                                    |          +-- render
                                    |          +-- figure
                                    +-- debug

container_gesture (layer 3, no Bevy) <- container, creative
gui_screen (layer 6, Bevy ECS only) <- render, menu, chat, figure, container, creative, hud,
                                       options, session, worlds, dialog (later)
```

Six rules hold for the graph.

- No edge goes up. `atlas`, `scene` and `primitives` name no screen.
- An opener depends on the crate of the screen that it opens, because it spawns the kind
  component. `worlds` opens a message screen, so it depends on `session`. The pause and the title
  open the options, so `session` depends on `options`. The title is the one exception
  (determination 16): it opens the world list, but `session` does not depend on `worlds`. No
  cycle exists.
- The chat key has no edge. `gui_chat` reads the stack (determination 6). `container` has no
  edge to `chat` or to `creative`.
- `figure` has no edge to a screen crate, and no screen crate depends on `figure`. A screen kind
  writes `ShowsFigure` of `gui_screen` (determination 22).
- `render` is the only crate that needs the render world. It reads `ScreenStack` to find the
  top screen. A headless client leaves `render` out and keeps the rest. `render` depends on
  `mcrs_minecraft_render` for `PanoramaView`, and no screen crate depends on `render`.
- `debug` draws with Bevy UI, not with the GUI scene. Its one GUI edge is `scene`, for
  `DebugOverlay` and the set `DebugOverlayToggle` of `debug_chord`.

`menu` depends on `gui_screen` to read `WorldShown`, and `gui_screen` depends on no other GUI
crate. So the background adds no upward edge, and no screen crate depends on
`mcrs_minecraft_render`. `mcrs_minecraft_client_world` does not depend on `gui_screen`. Only its
chat systems change, because `ChatInput` ends. `scene` depends on `atlas`, because
`gui_frame_changed` tests a change of `GuiAtlasLayout`.

### The retained scene

Steps 1 and 2 built the scene. The open items of step 2 are in section "Step 2: the retained
scene".

**The layer.** `GuiLayer` is a component. It holds the quads and the item icons of one part of
the GUI, its key and the time of its next redraw. The entity of a layer is a screen entity, a
child of a screen entity, or an entity that a HUD plugin spawns at startup.

| Field | Meaning |
|---|---|
| `band` | `Hud`, `Screen` or `Overlay`. The order of the bands is fixed. |
| `part` | A small number. It orders two layers of one band and one screen. |
| `quads`, `icons` | The output of the draw. The owner clears the vectors and keeps their capacity. Code outside `GuiLayer` writes and reads them through methods (step 2). |
| next redraw | The time at which the layer is due again, set with `GuiLayer::redraw_at`. A draw clears it. |

The batch sorts layers by the key (band, depth, part). The depth is the index of the screen
entity in `ScreenStack`. A HUD layer and an overlay layer have the depth 0. The batch takes the
HUD band, the screen layers of the top screen and the overlay band. A screen with two parts
spawns a child entity for the second part. The despawn of the screen despawns the child.

| Layer | Key |
|---|---|
| Hotbar, health, chat lines, F3 screen | (`Hud`, 0, 0 to 3) |
| Background of the top screen | (`Screen`, depth, 0) on a child |
| Title screen: menu, then logo | (`Screen`, depth, 1) and (`Screen`, depth, 2) on a child |
| Create world screen with its message | (`Screen`, depth, 1) and (`Screen`, depth, 2) on a child |
| Tooltip, drag preview | (`Overlay`, 0, 0 to 1) |

**The draw.** One draw system writes one kind of layer. It runs only on a change of its inputs.
Each gate is an event condition.

| Trigger | Condition |
|---|---|
| The kind component or the data of the kind | `any_match_filter::<Changed<Kind>>` |
| The window, the GUI scale, the pointer | `resource_changed::<GuiSize>`, `resource_changed::<GuiPointer>` |
| The language, the options | `resource_changed::<ClientLanguage>`, `resource_changed::<ClientOptions>` |
| The caret blink, a caption scroll | `layer_due::<Kind>` |
| The chat lines and the action bar | `chat_fading`, and the change of the inputs of the chat layer |
| The top screen | `any_match_filter::<Added<TopScreen>>` |

No global clock exists. Each layer records the time of its next redraw, and `layer_due::<Kind>`
is true when that time comes. A global clock cannot express the caret blink, because it counts
from the focus time of each box (EDIT-CUR-002 in `docs/spec/client-edit-box.md`). The widget crate
reports the next time through `EditBox::next_blink_ms`, `Widgets::next_redraw_ms` and
`CaptionLayout::scrolls`. `layer_due` and `gui_glint` read the time through `menu_clock` of
`menu_input` today. Step 5 moves that function into `layer_due` as `gui_clock`, so the scene does
not depend on the menu host.

The chat layer redraws on a new line and when the chat opens or closes. `chat_fading` is true on
a GUI tick while a line or the action bar changes its alpha (determination 19). The health HUD
redraws on a tick only while hearts shake.

The shared gates are functions that return a condition. Each gate goes to the lowest crate that
holds all the types that it reads.

| Gate | Reads | Crate |
|---|---|---|
| `gui_frame_changed` | `GuiSize`, `GuiAtlasLayout`, `Font` | `gui_scene`, module `draw_gate` |
| `layer_stale::<Kind>` | `gui_frame_changed`, `layer_due` | `gui_scene`, module `draw_gate` |
| `changed_or_removed::<Component>` | Any component | `gui_scene`, module `draw_gate` |
| `item_icons_changed` | `ItemTable`, `CurrentRegistries`, `ItemModels`, `IconMeshes` | `gui_primitives`, module `item_icons` |
| `slot_view_changed` | `ShownDragPreview`, `SlotIcon`, the holders and the open menu | `gui_container`, module `slot_view_changed` |

**The tooltip.** `Tooltip` is a component on the layer entity of each screen kind. The input
system of the kind writes the request into it. `draw_tooltip` of `tooltip_layer` runs in `Batch`
before `build_gui_batch`, because it reads the tooltips that the draws write in the same frame.
It runs on `any_match_filter::<Changed<Tooltip>>`.

**The title fade.** `GuiFade` is a resource. The title system writes it, and `build_gui_batch`
applies it to the quads. A draw never changes a retained layer after it wrote it.

**The batch.** `build_gui_batch` runs on `any_match_filter::<Changed<GuiLayer>>`, on
`any_component_removed::<GuiLayer>`, on `resource_changed::<GuiFade>`, on `gui_frame_changed` or
on `resource_changed::<ScreenStack>`. It joins the layers in key order into `GuiBatch`. It
resolves the glyph slots in `GlyphCache`. A missed glyph is rasterised through
`Font::unifont_cell` into the list `GlyphUploads`.

**The crossing.** An extract system with `ResMut<MainWorld>` moves the batch into the render
world when the batch changed, and moves `GlyphUploads` when the list is not empty. Nothing is
cloned. The render world keeps the last batch until a new one arrives, and writes the vertex
buffer only then. The glint offset changes on each frame, so it leaves the batch. It is a small
`Copy` value, `GuiGlint`, that crosses with `ExtractResource`. The main world writes it with
`set_if_neq` only while the batch has a glint draw (`BatchGlint`).

**The phases.** `GuiPhase` has two sets in a chain. No screen orders itself against another
screen, so a dialog over a paused screen needs no new set. `GuiPointerPlugin` writes `GuiSize`
and `GuiPointer` with `set_if_neq` in `PreUpdate`, before the menu input reads them.

| Set | What runs in it |
|---|---|
| `Draw` | The draw systems. Each one writes only its own layers, so they run in parallel. |
| `Batch` | `draw_tooltip`, then `build_gui_batch`. |

**The cost model.** The work of a frame is proportional to the layers that changed. A frame with
no change runs no draw system and no batch build, and writes no buffer.

**Classification of each value** (skill `bevy-ecs`).

| Value | Class | Owner |
|---|---|---|
| Window size, `GuiConfig`, `ClientOptions`, `ClientLanguage` | Authoritative | Bevy, the options, the language load |
| The last cursor position that the platform reported | Temporal fact | Bevy window messages |
| `GuiSize`, `GuiPointer` | Materialized derived state, for many readers and for change detection | One system each in `GuiPointerPlugin` |
| `GuiLayer` quads and icons | Materialized projection of the kind data and the inputs above | The draw system of its kind |
| The next redraw time of a layer | Temporal fact | The draw system that writes the layer |
| `Tooltip` | Materialized projection of the hovered widget or stack | The input system of the screen kind |
| `GuiFade` | Derived from the state of the title screen | The title system |
| `GuiBatch` | Materialized projection of all layers in key order | `build_gui_batch`, then the render world after the move |
| `GlyphCache` | Authoritative: the slot of each glyph | `build_gui_batch` |
| `GlyphUploads` | Temporal fact: the cells that the GPU does not have yet | `build_gui_batch`, then the extract |
| `GuiGlint` | Derived from the time | One system in `Batch` |
| `GuiTick` | Temporal fact: the count of GUI ticks | `GuiTickPlugin` |
| `GuiImages`, `GuiImageHandles` | Authoritative: the atlas and glint assets | The atlas load; the handles stay in the main world |
| `StitchRequests` | Authoritative: the textures that the screen crates need in the atlas | The plugin of each screen crate, at build |

### The atlas and the render world

- The atlas pixels and the glint pixels are two Bevy `Image` assets with
  `RenderAssetUsages::RENDER_WORLD`. Bevy uploads each image and drops the main copy. A pack
  reload replaces the asset. The GUI pipeline binds the GPU image of each handle.
- `GuiImages` holds the two `AssetId` values. It is `Copy` and crosses with `ExtractResource`.
  `GuiImageHandles` keeps the strong handles in the main world.
- `GuiAtlasLayout` keeps the regions, the sprites, the textures and the origins.
  `gui_atlas_image.rs` and `GuiAtlasImage` end.
- The glint tuple `(u32, u32, Vec<u8>)` ends. The loader output `GuiAtlasParts` names the glint
  as an `Image`. `load_gui_atlas` returns the enum `GuiAtlasError`.
- `FontCells` ends. The main world owns `Font` and `GlyphCache`, and rasterises each missed cell
  with `Font::unifont_cell` into `GlyphUploads`. The render world holds no font data. It writes
  each cell of `GlyphUploads` into the glyph region of the atlas texture.
- A new atlas image has no glyphs. When the handle changes, `GlyphCache` clears, and the next
  batch uploads the cells again.
- `Font::unifont_cell` is the one path to a cell. `glyph_page.rs` in `mcrs_minecraft_client`
  uses it too.

**The stitch requests** (determination 21). Step 5 changes this part. Today `gui_atlas_load`
imports `CREATIVE_TABS` and `CreativeGeometry` from `menu_geometry` for the tab and scroller
sprites. `menu_background` reads `MenuType` of every container from `menu_key`, and the creative
backgrounds from `menu_geometry`. The new design:

- `StitchRequest` names the source textures and the target region. A request for a background
  also names the extent that the atlas crops.
- `StitchRequests` is a resource of the atlas. The plugin of each screen crate adds its requests
  when the app builds. The atlas load reads the resource.
- `gui_container` registers the container backgrounds. `menu_background` moves to
  `gui_container` and keeps only the containers.
- `gui_creative` registers the tab sprites, the scroller sprites and the creative backgrounds.
- The atlas keeps its list of generic GUI sprites, for example the widgets and the glint.
- `gui_atlas` loses its edges to `menu_geometry` and `menu_key`.

**The model crate.** The atlas uses `ResourceSet`, `decode_png`, `missing_pixels` and
`MISSING_SIZE` of `mcrs_minecraft_model` today. Step 6 moves the PNG decoder and the missing
texture down to `mcrs_minecraft_resource_pack`, and the atlas reads `&PackFiles`. After step 6
the atlas needs no `mcrs_minecraft_model`.

### GuiSize and GuiPointer

`GuiSize` (width, height and scale) and `GuiPointer` are resources. One system of
`GuiPointerPlugin` writes each with `set_if_neq` in `PreUpdate`, from the window and `GuiConfig`. They replaced
`GuiScene` and `MenuExtent`. The `Pointer` parameter of `menu_input` and the function
`creative_pointer` read them.

`GuiPointer` follows WID-PTR-005 in `docs/spec/client-widget.md` and CPR-FIG-003 in
`docs/spec/client-player-render.md`. Step 2 removed its `None` outside the window.

- `GuiPointer` is a `DVec2` in GUI units, not an `Option`. It is the last cursor position that
  the platform reported, divided by the scale. `point()` gives its floor.
- The system takes the position from the cursor-moved messages of the window. It never clears
  the position, also when the cursor leaves the window.
- While a button is held outside the window, the platform reports each move. The position can
  then be negative or larger than the GUI size. No widget is hovered at such a position.
- The figure turns to that position (CPR-FIG-003). The fallback `IVec2::NEG_ONE` of
  `inventory_figure` ends.

### The screen model

**The kind component.** Each open screen is one entity. Its kind is a component that the crate
of the screen owns: `PauseScreen`, `TitleScreen`, `OptionsScreen`, and so on. The data of a screen
is a field of its kind component, not a resource. `GameModeSwitcher(GameMode)` becomes the state
of the switcher. `Message(ScreenMessage)` becomes the data of `MessageScreen`. The set of screens
is open. No central enum and no id registry exist.

Three kinds reuse a type that exists.

- The chat kind is `ChatScreen` of `mcrs_minecraft_chat_client`. A new `bevy` feature of that
  crate adds the `Component` derive. `ChatInput` of `client_world` ends. The suggestion systems
  of `client_world` read `Single<&mut ChatScreen>`. A suggestion reply with no open chat is
  dropped.
- The creative kind is `CreativeScreen` of `mcrs_minecraft_inventory_client`. It moves from the
  player to the open-menu entity. The observer `empty_creative_cursor` of that crate follows.
- `ContainerScreen` is new. It sits on the open-menu entity, which `Menus` of the player names.
  A chest menu entity is spawned on open and despawned on close, so its screen follows it. The
  inventory menu entity lives as long as the player. For it, the screen opens with the insert of
  the kind and closes with its removal. The key system and the observers of the open and the
  close packets do this on an event.

**The shared parts** belong to `gui_screen`.

| Item | What it is |
|---|---|
| `ScreenEntry` | Every kind component requires it. It holds `replace`: `None`, `Top`, `All` or `At(Entity)`. Its add and remove observers keep the stack. |
| `ScreenStack` | A resource, `ScreenStack(Vec<Entity>)`. The last entity is the top. |
| `TopScreen` | A marker on the entity at the top. The observers of `ScreenEntry` move it. |
| `WorldPause` | An enum component, `Pause` or `Run`. `ScreenEntry` requires it with `Pause`. |
| `EscapeRule` | An enum component, `Close` or `Refuse`. `ScreenEntry` requires it with `Close`. A kind requires `Refuse` when Escape must not close the screen. A kind may change the value while the screen is open. |
| `CHAT_KEY` | The default key of the binding `key.chat` (`docs/spec/client-chat.md`). No key binding exists yet; a later binding replaces this one item. `gui_chat` opens the chat with it, and `gui_creative` selects the search tab with it. |
| `Backdrop` | An enum component. `ScreenEntry` requires it with `Default`. |
| `WorldShown` | A resource, `WorldShown(bool)`. The client app writes it. |
| `ShowsFigure` | A component, `ShowsFigure(Option<FigurePlacement>)`: the rect of the inventory figure in GUI units and the GUI units for each block. |
| `close_top_screen` | The one Escape system. |

**Open, close and replace are observers.** To open a screen, a system spawns an entity with the
kind component. The add observer of `ScreenEntry` reads `replace`. For `Top` it despawns the old
top, and for `All` it despawns every entity of the stack. Then it pushes the new entity and moves
`TopScreen` to it. For `At(target)`, it puts the new entity at the index of `target` and
despawns `target`. `TopScreen` moves only when `target` was the top.

The game has one current screen, and every open replaces it. So each screen that the network or
the session opens states its mode. A container, a disconnect, level loading, reconfiguration and
death spawn with `All`: they replace any open screen, a dialog too (DLG-FLOW-005). A dialog over
a dialog or over the waiting screen spawns with `Top` (DLG-FLOW-002). A dialog that arrives
while the warning screen is on top spawns with `At` of the dialog below the warning screen
(section 18 of `docs/spec/dialog.md`). A screen that keeps its parent, such as options over the
pause screen, spawns with `None`.

To close a screen, a system despawns the entity or removes its kind. The remove observer of
`ScreenEntry` takes the entity out of the stack, wherever it stands. If the entity was the top,
it moves `TopScreen` to the new last entity. `clear_dialog` despawns the dialog entity wherever
it stands. When the warning screen is above it, the warning screen then returns to the screen
below the dialog (DLG-FLOW-003).

**Why a resource and not a relationship.** A relationship `Covers` and `CoveredBy` must link the
screen above to the screen below on a removal from the middle, and "replace all" needs a walk.
The batch also needs the depth of a screen, which a resource gives with the order.

**Why `TopScreen` is a marker.** Every draw and input system selects the top screen with a
`Single<.., With<TopScreen>>`. Only the observers move it, on an open or a close, and
`marker_toggle` checks tick systems only.

**The menu is a component.** `Menu` becomes a component of the screen entity. Today it is a
resource that `project_menu` rebuilds for the top screen. A screen below the top keeps its own
`Menu` with its focus and its scroll. So the table `kept_focus` ends.

**A return to a screen.** WID-SCR-003 gives two kinds of screen. A screen that keeps its
widgets keeps the focus and the values on a return and on a resize. Each other screen builds its
widgets again and clears the focus (MENU-PROP-001, the pause screen). The stack keeps every
`Menu`, so the second kind needs an explicit rebuild: its layout system also runs on the insert
of `TopScreen` on its entity.

**One layout system per kind.** The layout of a kind is a pure function from the kind data, the
GUI size, the options and the language to the widgets. One system per kind calls it. The system
runs on `any_match_filter::<Added<Kind>>` or on `resource_changed` of `GuiSize`, `ClientOptions`,
`ClientLanguage` and the resources of that kind (`NewWorldForm`, `WorldTypes`, `WorldList`; for
the title, `SingleplayerEntry` through `resource_exists_and_changed`). On `Added` it builds the
`Menu`. On a resource change, a kind that keeps its widgets writes the new rects and captions
into the existing widgets and keeps the focus. A kind that builds its widgets again builds the
`Menu` again.

**The figure.** Today `placed_figure` of `inventory_figure` reads `Screen`, the open menu and
`CreativeScreenState`. Step 9 replaces that read with `ShowsFigure` on the screen entity.

- `ContainerScreen` of the inventory menu requires `ShowsFigure` with the survival placement.
  Its layout system writes the rect on a change of `GuiSize`.
- `CreativeScreen` requires `ShowsFigure(None)`. Its systems write the creative placement on the
  survival inventory tab and `None` on each other tab (CCI-LAY-007).
- The pose system of `gui_figure` reads `ShowsFigure` of the entity with `TopScreen`.
- `FigurePlacement::for_creative_tab` moves to `gui_creative`. The survival placement moves to
  `gui_container`. `FigurePlacement` moves to `gui_screen` as the value of `ShowsFigure`.

**Escape.** `close_top_screen` runs on `input_just_pressed(KeyCode::Escape)`. It takes
`Single<(Entity, &EscapeRule), With<TopScreen>>` and despawns the entity when the rule is
`Close`. With `Refuse`, the key goes to the focused widget (WID-SCR-002), or the kind reads it
itself. The title, the connection, disconnect, message, level loading, reconfiguration and death
screens refuse, and so do the confirm screens. A container screen refuses and sends the close
request through its own path. A dialog refuses and runs its cancel action (DLG-KEY-002). The
waiting screen spawns with `Refuse` and writes `Close` when its button becomes active
(DLG-WAIT-001). It keeps the `GuiTick` of its open in its kind data. The pause opener of
`gui_session` runs on the same input when the stack is empty and a player exists. It reads the
stack before the despawn applies, so one key press never closes a screen and opens the pause.

**The background.** The spec term is "background" (WID-SCR-004 and WID-SCR-008 in
`docs/spec/client-widget.md`). `Backdrop` is the code name: one enum component that the kind of
the screen requires. The opener chooses nothing, and a sub screen needs no copy. Whether a world
exists is not a field of a screen. The resource `WorldShown` holds it, and the client app is its
one writer, from its world state, with `set_if_neq`. A dialog of the configuration phase sees
`false`.

`project_backdrop` is the one system that reads `Backdrop`. It belongs to `gui_render`, names no
screen kind and runs on a change of `TopScreen`, of the `Backdrop` of the top screen or of
`WorldShown`. It writes `PanoramaView` and `BackdropBlur` for the render world. It also writes
the texture quad or the gradient quad into the background layer of the top screen. That layer is
a child of the screen entity with the key (`Screen`, depth, 0), and the system spawns it when the
screen becomes the top. A draw system of a kind uses the parts 1 and up. The gradient quad of
`InGameGradient` takes `DIM_TOP` and `DIM_BOTTOM` of `in_game_gradient` in `gui_primitives`.

`BackdropBlur` is the radius of the blur, a `Copy` resource of `mcrs_minecraft_render`. The
radius is the option "Menu Background Blur" (0 to 10, default 5, 0 is off) when the variant
blurs, and 0 otherwise.

| `Backdrop` | Used by | `PanoramaView.draw` | `BackdropBlur` | Quad of the background layer |
|---|---|---|---|---|
| `Default` (the value that `ScreenEntry` requires), no world | Pause, options, a server dialog | `Blurred` | The option | The menu background texture over the window |
| `Default`, in a world | The same screens | `Hidden` | The option | The in-world texture over the window |
| `Panorama` | Title | `Plain` | 0 | None |
| `PanoramaOverWorld` | Message, level loading | `Blurred` | The option | The texture of the world state of `WorldShown` |
| `InGameGradient` | Screens that WID-SCR-008 lists | `Hidden` | 0 | The gradient `C0101010` to `D0101010` |
| `Gradient { top, bottom }` | Death | `Hidden` | 0 | A vertical gradient of the two ARGB colours |
| `None` | Chat, `PauseOverlay`, game mode switcher | `Hidden` | 0 | None. The game mode switcher draws its own texture in its own layer. |

With no world, the panorama pass blurs the panorama with the radius of `BackdropBlur`. In a
world, the blur node of `mcrs_minecraft_render` blurs the image of the world and the HUD with
the same six passes (WID-SCR-005). The background quads draw over the result, so they stay
sharp. The spin of the panorama is a separate system in `gui_render`. It runs each frame while
`PanoramaView.draw` is not `Hidden` and writes `spin_degrees` with `set_if_neq`.

The blur needs render work (step 12). Today `mcrs_minecraft_render` blurs only the panorama, in
a pass that runs before the transparent pass of the world. The HUD draws later, in the GUI pass.
So the blur node takes the radius from `BackdropBlur` and runs a second time on the target after
the HUD band. `gui_render` splits the draw of the batch into the HUD band and the rest, and the
blur node runs between them. The option "Menu Background Blur"
(`options.accessibility.menu_background_blurriness`) is a new field of `client_options`, with a
slider in the accessibility screen.

`Menu` draws its header and footer separators with the textures of the world state. It reads
`WorldShown` for them and redraws on `resource_changed::<WorldShown>`. It draws no background.

**The behaviours that the split adds.** The specification corrects four screens. The plan
carries each one in a step, with a check.

| Behaviour | Spec | Owner | Step | Check |
|---|---|---|---|---|
| The survival inventory tab of the creative screen shows the figure | CCI-LAY-007 in `docs/spec/client-creative-inventory.md` | `inventory_figure`, then `gui_figure` and `gui_creative` | 2 | A test that selects the survival inventory tab and reads the figure view: the rect (73, 6) to (105, 49) from the window corner and 20 GUI units for each block. Another tab shows no figure. |
| Each `Default` screen in a world blurs the world and the HUD | WID-SCR-004, WID-SCR-005, WID-SCR-008 | The blur node and `BackdropBlur` in `mcrs_minecraft_render`; the split of the draw and `project_backdrop` in `gui_render` | 12 | A pixel test of the blur with the radius 5 against the postconditions of WID-SCR-005, and a test that the radius 0 runs no pass. A screenshot of the pause screen in a world. |
| The death screen and the confirmation of its title button draw a gradient | WID-SCR-008, PLD-SCR-004 | `gui_session` | 10 | A test that opens the death screen and reads the background layer: one gradient quad of the colours of the spec, no texture quad, `BackdropBlur` 0. |
| The pause with no menu | WID-SCR-008, MENU-PAUSE-007 to 010 | `gui_session` | 10 | A test that sends the chord in a local world with no screen: the stack holds the kind, its menu holds only the title "Game Paused", the background layer is empty, `BackdropBlur` is 0 and the local server runs no tick. |

The pause with no menu is a second kind, `PauseOverlay`, in `gui_session`. It requires
`Backdrop = None`, and the pause opener spawns it (MENU-PAUSE-007 to 010).

**The initial screen.** Until step 11, the client app inserts the initial `Screen` resource
itself. From the step that moves a screen to an entity, it spawns that kind when its
configuration names that screen.

**Classification of each value** (skill `bevy-ecs`).

| Value | Class | Owner |
|---|---|---|
| The kind component and its data | Authoritative | The crate of the screen |
| `Menu` | Authoritative widget state (focus, scroll, values) with derived rects | The layout system and the menu input of its kind |
| `replace`, `Backdrop`, `WorldPause` | Authoritative, fixed at spawn | The kind that requires them, or the spawner |
| `EscapeRule` | Authoritative. It flips at most once while a screen is open, so it is a value and not a marker. | The kind that requires it |
| `ShowsFigure` | Derived from the kind data and `GuiSize` | The layout system of its kind |
| The order of `ScreenStack` | Authoritative. No other value gives the order. | The observers of `ScreenEntry` |
| `TopScreen` | Materialized from `ScreenStack`, for query selectivity | The observers of `ScreenEntry` |
| The previous screen, the depth | Derived from `ScreenStack` | Nobody. Each reader computes it. |
| `MenuEvent` | Temporal fact | The menu input |
| `WorldShown` | Derived from the world state of the client | The client app, the one writer |
| `PanoramaView.draw`, `BackdropBlur`, the background layer | Materialized from `Backdrop`, `WorldShown` and the option | `project_backdrop` |
| `PanoramaView.spin_degrees` | Temporal fact (the angle) | The spin system of `gui_render` |

### No central dispatcher

Three modules know every screen today. `next_screen` holds the table "back from X goes to Y".
`follow_menu_events` matches (screen, widget) for every screen through `TitleActions`, a
parameter of nine fields. `project_menu` builds the `Menu` of every screen. All three end in
steps 10 and 11.

- `MenuEvent` gains the screen entity. The crate of each kind reads `MenuEvent` on
  `on_message::<MenuEvent>`, keeps the events of its own kind, and acts. It spawns a screen,
  despawns its own screen, or writes a message such as `PlayWorld`.
- "Back" is the despawn of the top. The previous screen is below it in the stack, so the table of
  `next_screen` is not necessary. Options opened from the pause screen return to the pause
  screen, and options opened from the title return to the title.
- "Back to the title" despawns every screen. The title crate shows the title when the stack is
  empty and no player exists. It runs on `resource_changed::<ScreenStack>` and on
  `any_component_removed::<Player>`.
- `ClickState::last_screen` and `ClickState::last_menu` end. The add and remove observers of
  `ContainerScreen` reset the gesture state.
- `TitleScreenState` becomes data of `TitleScreen`. The title fades only on its first show in a
  run. So `follow_session` no longer ends the fade.

### The container gesture kernel

The gestures of a container are a state machine: press, drag, release, quick move and the double
click. Today `Hover::Creative(CreativeHit)` and the argument `CreativeGrid` of `Viewer::input`
make the gesture code depend on the creative code. `gesture_frame` also writes the Bevy resource
`DragPreview`, and it names `engine_inventory_click`, `engine_item` and `inventory_client`.
`viewer` reads `MenuStacks`, which is a container item.

The proposal inverts these edges. The kernel keeps only the pure state machine.

- The kernel takes a resolved hit, `GestureHit`. Its field `clicked` is `Option<Clicked>`, and
  `None` marks a hit that sends no click. The hit also states the drag permission and the target.
  The kernel does not know creative. `Hover` stays in the client; the kernel has `gesture_hit`.
- `gui_creative` maps `CreativeHit` to that value. `creative_clicks` holds the map.
- The kernel takes slot indices, not `Entity`. It reads the menu through the small trait
  `MenuView` with the one empty host `NoMenu`, as `BodyView` and `NoBodies` in
  `mcrs_minecraft_physics` do. `MenuView` gives the presence of the carried stack and of an entry,
  the drag rules of an entry, and `knows_drag_rules`.
- The double click remembers a stack. The stack type is the associated type `MenuView::Stack`,
  so `MenuGesture<S>` is generic. `stack()` and `shift_double_click_moves()` take `&mut self`,
  because a host such as `ItemColumns` fills a mask cache when it reads a stack.
- `MenuGesture` holds one menu. A host makes a new `MenuGesture` when the menu changes, so
  `LastPress` names no menu. A drag holds the entries 0 to 127.
- The kernel pushes each click into a `Vec<GestureClick>` of the caller. `GestureClick::Hit` is a
  click on the hit, `GestureClick::Menu` a click of a drag or a double click. `GestureButton` is a
  mouse button with its binding, without Bevy.
- `DragPreview` becomes a value with no Bevy in the kernel. The kernel fills it beside the clicks.
- `MenuGesture` holds the press, the release and the drag logic itself. That ends the leaks of
  `release`, `remembered` and `kind`.
- The Bevy resource and the glue to the engine stay in `gui_container`. `item_key_codes` and
  `wheel_notches` map Bevy input types, so they stay there too.

Step 3 splits three modules of today. Each half has one home.

| Module | Kernel half, in `container_gesture` | Bevy half, in `gui_container` |
|---|---|---|
| `gesture_frame` | `gesture_frame`: the state machine, which returns the click and the `DragPreview` | `gesture_input` (→): the map of a mouse button, the host of `MenuView` over `MenuStacks`, `MenuTargetView` and `Stacks`, the wrap of a click into `MenuInput` |
| `viewer` | `viewer`: the click input from the resolved hit and `MenuView` | `gesture_input`: the viewer and menu entities, the read of `MenuStacks` |
| `drag_preview` | `drag_preview`: the value `DragPreview` | `shown_drag_preview` (→): the resource `ShownDragPreview`, which holds the value that the kernel returned |

`menu_view` (→) is new in the kernel. It holds `MenuView` and `NoMenu`.

The kernel is `mcrs_minecraft_container_gesture`, in layer 3 beside `container_click`. It is
`#![no_std]` with `alloc`. Its dependencies `container_click` and `inventory_click` are `no_std`
today, with `inventory_player` and `inventory_transaction`. It must not take `item_stack`, which
needs `std`. It reads stack presence and the generic stack, as the list above says.

### What each crate holds

The lists use the file names of today. A name with an arrow marks a new module. "Merge" means
that two modules become one. The count of files includes `lib.rs` and the test modules. Every
file of today's `src/` has one home below, or ends in a named step.

**`mcrs_minecraft_resource_pack`** (new). It holds `PackFiles` of `model_file`, a PNG decoder
(the `png` crate, not the Bevy image), and the missing texture (`missing_pixels`, `MISSING_SIZE`)
of `mcrs_minecraft_model`. It has no Bevy and is not part of `mcrs_minecraft_resource`.
`model_file`, the model, the atlas, the font and the language take these items from it. It is not
`no_std`, because `png` 0.18 reads through `std::io` (section "Options" records `zune-png`).

**`mcrs_minecraft_font`** (11 files). Modules: `font`, `font_definition`, `font_glyph`,
`font_unihex`, `glyph_cache`, `language`, `text`, `span_events`, `event_index`,
`font_dump_tests`.

- Public names: `Font`, `FontOptions`, `Span`, `Glyph`, `GlyphImage`, `ImageSource`,
  `BitmapPage`, `GlyphCache`, `ClientLanguage`, `Wrap`, `text_spans`, `wrap`, `argb`,
  `shadow_color`, `SpanEvents`.
- Not `#![no_std]`: `text`, `lang` and `nbt` have no `no_std` build, and `text` also needs
  `wire`, `core` and `profile`.
- `Text` comes from `protocol_game::text`, a re-export of `mcrs_minecraft_text`. The font takes
  it from `text` and drops the edge to `protocol_game`. `ResourceSet` and `decode_png` of
  `mcrs_minecraft_model` need Bevy and `client_jar`, and `full_id` and `split_id` come from
  `mcrs_minecraft_model_file`. Only the file lookup is used, and that is `PackFiles`. Step 6
  moves `PackFiles` and the PNG decoder to `mcrs_minecraft_resource_pack`, and the font takes
  `&PackFiles`. `full_id` and `split_id` end; `ResourceLocation::read` of
  `mcrs_minecraft_resource` replaces them.
- `lib.rs` re-exports `draw_centered_text` of `text_draw` for `vanilla.rs` of
  `mcrs_minecraft_client`. The re-export ends in step 7, and the client takes the function from
  `gui_primitives`. `font_definition` uses `bevy::log::warn`, which becomes an error value.
  `font_unihex` names `client_jar::Directory`. The loader takes the bytes from its caller.
- Dependencies: `mcrs_minecraft_resource_pack`, `mcrs_minecraft_lang`, `mcrs_minecraft_text`,
  `mcrs_minecraft_text_flatten`, `mcrs_minecraft_item` and `mcrs_minecraft_registries` (for the
  click and hover event types of `font`), `bevy_math`. A `bevy` feature adds the `Resource`
  derive to `Font`, `GlyphCache` and `ClientLanguage`.

**`mcrs_minecraft_gui_atlas`** (10 files, new). Modules: `gui_atlas_layout`, `gui_atlas_load`,
`gui_atlas_error`, `gui_images`, `gui_image_handles`, `sprite`, `gui_sprite_id`,
`gui_texture_id`, `stitch_request` (→).

- Public names: `GuiAtlasLayout`, `GuiAtlasParts`, `GuiAtlasError`, `load_gui_atlas`,
  `GuiImages`, `GuiImageHandles`, `GuiSprite`, `Scaling`, `GuiSpriteId`, `GuiTextureId`,
  `StitchRequest`, `StitchRequests`.
- `sprite` keeps the sprite data and the read of `.mcmeta`. `GuiSprite::draw` emits `GuiQuad` of
  the scene, and the scene depends on the atlas. So step 5 moves it, and the test of
  `gui_atlas_load` that draws the nine-slice button, to `sprite_draw` in the primitives.
- Dependencies: `font`, `resource_pack`, Bevy asset and image.

**`mcrs_minecraft_gui_scene`** (14 files). Modules: `gui_size`, `gui_pointer`, `gui_config`,
`gui_quad`, `gui_layer`, `gui_band`, `scene_icon`, `gui_phase`, `layer_due`, `gui_fade`,
`draw_gate`, `debug_chord` (→), `gui_scene_plugin` (→, from `gui_plugin`).

- Public names: `GuiSize`, `GuiConfig`, `GuiPointer`, `GuiQuad`, `GuiLayer`, `GuiBand`,
  `SceneIcon`, `GuiPhase`, `GuiFade`, `layer_due`, `gui_clock`, `gui_frame_changed`,
  `layer_stale`, `changed_or_removed`, `DebugModifier`, `DebugOverlay`, `DebugOverlayToggle`,
  `GuiScenePlugin`.
- `debug_chord` holds the state of the F3 chord. `DebugModifier` records that F3 served as a
  modifier. `DebugOverlay` records that the F3 screen shows. `DebugOverlayToggle` is the system
  set of the F3 overlay toggle. The game mode switcher writes `DebugModifier` and runs before the
  set. `gui_debug` puts `toggle_overlay` in the set. So `gui_hud` does not depend on `gui_debug`.
- `GuiScenePlugin` holds only the generic scene: size, pointer, layers, fade and phases. The
  tooltip, the batch and the glint belong to `GuiRenderPlugin`. The scene has no edge to
  `client_world`, because `GuiPlugin` loses its field `screen` in step 5.
- Dependencies: `atlas`, `font`, `mcrs_minecraft_item_icon`, `client_widget` (for `GuiPoint`),
  Bevy window.

**`mcrs_minecraft_gui_primitives`** (13 files). Modules: `text_draw`, `tooltip`,
`tooltip_layer`, `item_decorations`, `item_icons`, `slot_icon` (the component only),
`texture_tile`, `outlined_box`, `irect`, `sprite_draw` (→), `in_game_gradient` (→), and
`gui_test_assets` (→) under a `test-support` feature.

- Public names: `draw_text`, `draw_str`, `draw_centered_text`, `Tooltip`, `TooltipLayer`,
  `LineGap`, `Decorated`, `CountLabel`, `decorations`, `ItemIcons`, `SlotIcon`,
  `item_icons_changed`, `draw_sprite`, `DIM_TOP`, `DIM_BOTTOM`.
- `slot_icon` orders its projection after `ClientNetworkSystems`. The projection system and
  `SlotIconPlugin` move to `slot_icon_plugin` of `gui_container` in step 7, so primitives take
  no network edge.
- `menu_test_app` is not a primitive. It builds apps with `MenuPlugin` and
  `MenuScreensPlugin`. Its asset fixtures (`ATLAS`, `TABLE`, `MODELS`, `insert_gui_assets`,
  `GUI`) move to `gui_test_assets`. Its app builders stay in `client_gui`. Each screen crate takes
  the builder that its tests use when it moves.
- `menu_item_tooltip` reads the player, the holders and the open menu, so it moves to
  `gui_container`.
- Dependencies: `scene`, `atlas`, `font`, `client_widget`, `item_icon`, `item_stack`,
  `item_table`, `mcrs_minecraft_registries`, `mcrs_minecraft_model`.

**`mcrs_minecraft_gui_render`** (12 files and the shader at its move, 13 files and the shader
after step 10). Modules: `atlas_quad`, `gui_batch`, `gui_batch_build`, `gui_items`,
`batch_glint`, `gui_glint`, `gui_layer_slot`, `glyph_uploads`, `gui_batch_extract`,
`gui_render_plugin` (→, the rest of `gui_plugin`), `panorama_spin` (→, `spin` of
`panorama_view`), and the shader `gui_items.wesl`.

- Public names: `GuiRenderPlugin`, `GuiBatch`, `GuiDraw`, `GuiDrawKind`, `GLINT_ALPHA`.
- Step 10 adds `project_backdrop` (→), the one background system (determination 20).
- `gui_items` embeds the shader. Its path `embedded://mcrs_minecraft_client_gui/gui_items.wesl`
  names the crate, so step 8 changes it to `embedded://mcrs_minecraft_gui_render/gui_items.wesl`.
  It draws the inventory figure into the GUI pass through `FigureDraw` of `entity_render`.
- Dependencies: `primitives`, `scene`, `atlas`, `font`, `mcrs_minecraft_render`,
  `entity_render`, `item_icon`, `model`, the Bevy render crates, wgpu. Dev-dependencies:
  `item_stack`, `mcrs_minecraft_resource`. Step 9 adds `gui_screen`, because the batch reads
  `ScreenStack`. Step 12 adds `client_options` for the blur radius.

**`mcrs_minecraft_gui_menu`** (17 files). Modules: `menu`, `menu_draw`, `menu_input`,
`menu_plugin`, `menu_cursor`, `menu_event`, `menu_input_systems`, `scroll_area_draw`,
`tab_bar_draw`, `edit_box_draw`, `leaf_draw`, `cursor_grab`, `edit_input`, `search_input`,
`option_caption`, and the test module `menu_test_app` (→).

- Public names: `Menu`, `MenuEvent`, `MenuPlugin`, `MenuInputSystems`,
  `draw_menu`, `screen_key`, `pastes`, `clipboard_text`, `copy_text`, `grab`, `SearchInput`,
  `option_caption`.
- `option_caption` formats the caption of an option widget from the language. It belongs to the
  widget host, because the create world screen uses it too.
- `edit_input` maps keys to `Key` and `TabDirection` of `chat_client`, a crate without Bevy.
- Today the caret test of `menu_draw` builds `title_app` of the old `menu_test_app` and sets
  `Screen::CreateWorld`. Step 16 gives `gui_menu` its own `menu_test_app` with the builder
  `menu_app`. It adds the scene, `MenuPlugin` and `gui_test_assets`, and spawns a test kind with
  an edit box. The caret test uses it.
- Dependencies: `primitives`, `scene`, `atlas`, `font`, `gui_screen`, `client_widget`,
  `chat_client`. Dev-dependency: `primitives` with `test-support`.

**`mcrs_minecraft_gui_screen`** (11 files, new). Modules: `screen_entry`, `screen_stack`,
`top_screen`, `escape_rule`, `world_pause`, `backdrop`, `world_shown`, `shows_figure`,
`close_top_screen`, `gui_screen_plugin`.

- Items: the table "The shared parts" of section "The screen model", and `FigurePlacement`: a
  rect and a scale. Its creative placement goes to `gui_creative`, and its feet and eye points
  stay in `gui_figure`.
- Dependencies: `bevy_ecs`, `bevy_app`, `bevy_input`, `bevy_math`. It is `#![no_std]`:
  `bevy_ecs` and `bevy_app` already serve `no_std` crates such as `engine_inventory_click`. No
  crate of the workspace takes `bevy_input` yet. Step 9 checks that it builds without `std`.

**`mcrs_minecraft_container_gesture`** (12 files, new). Modules: `gesture_frame` (the kernel
half), `hover` (becomes the resolved hit), `viewer` (the kernel half), `modifiers`, `last_press`,
`menu_gesture`, `open_drag`, `press_kind`, `release_rule`, `drag_preview` (the value),
`menu_view` (→). `#![no_std]` with `alloc`. Dependencies: `container_click`,
`inventory_click`. Today `menu_gesture` names `ItemStack`; step 3 replaces it with
`StackPresence`. Section "The container gesture kernel" gives the split.

**`mcrs_minecraft_gui_chat`** (5 files). Modules: `chat`, `chat_input`, `chat_suggestions`,
`chat_view`.

- The chat key system runs on `input_just_pressed` of `CHAT_KEY` or the slash key (CHAT-OPEN-001).
- Public names: `ChatDrawPlugin`, `ChatScreenPlugin`, `ChatLines`.
- Dependencies: `menu`, `primitives`, `scene`, `atlas`, `font`, `gui_screen`, `client_widget`,
  `client_world`, `chat_client` with `bevy`, `mcrs_minecraft_command`, `protocol_game`,
  `mcrs_minecraft_item`. Dev-dependency: `mcrs_minecraft_text`.

**`mcrs_minecraft_gui_figure`** (4 files, new). Modules: `inventory_figure`, `figure_points`
(from `figure_placement`), `figure_projection`.

- `figure_points` keeps `FigurePoints`, the feet and eye points of a placement for a pose.
- `inventory_window_corner` of `inventory_figure` moves with the survival placement to
  `gui_container` in step 9. A test of `menu_screen` reads it.
- Public names: `InventoryFigurePlugin`, `InventoryFigure`, `FigureView`.
- Dependencies: `primitives` (for `irect`), `scene`, `gui_screen`, `client_world`,
  `entity_render`, `entity_visual`, `entity`, `pose`, `item_icon`, `network`, `protocol_game`. It
  has no edge to `menu_geometry`, `inventory_client` or a screen crate.

**`mcrs_minecraft_gui_container`** (16 files). Modules: `menu_screen`, `menu_stacks`,
`menu_label_text`, `hotbar`, `shown_drag_preview` (→), `gesture_input` (→), `inventory_clicks`
(the container half), `container_screen` (→, merges `close_menu_screen` and
`follow_open_menu`), `container_draw`, `menu_item_tooltip`, `slot_icon_plugin` (→, the
projection of `slot_icon`), `item_key_codes`, `wheel_notches`, `menu_background`,
`slot_view_changed` (→, from `draw_gate`).

- The plugin registers the stitch requests of the container backgrounds.
- Today `inventory_clicks` imports eight creative modules (`creative_clicks`, `creative_grid`,
  `creative_pointer`, `creative_screen_edit`, `creative_screen_input`, `creative_screen_plugin`,
  `creative_screen_shown`, `mark_creative_screen`) and `chat_input::CHAT`. Step 9 splits it. The
  creative half goes to `creative_inventory_clicks` of `gui_creative` and orders itself against
  the public set `InventoryClickSystems`. The check of the chat key ends, because the chat key
  opens nothing over a screen. So `gui_container` has no edge to `gui_creative` or `gui_chat`.
- Public names: `ContainerScreen`, `InventoryClicksPlugin`, `InventoryClickSystems`,
  `HotbarPlugin`, `SlotIconPlugin`, `MenuStacks`, `ShownDragPreview`, `item_key_codes`,
  `wheel_notches`.
- Dependencies: `container_gesture`, `menu`, `primitives`, `scene`, `atlas`, `font`,
  `gui_screen`, `client_widget`, `client_world`, `network`, `protocol_game`, `container_click`,
  `inventory_click`, `inventory_menu`, `inventory_client`, `engine_inventory_click`,
  `engine_inventory_menu`, `engine_item`, `engine_registries`, `game_mode`, `entity`,
  `entity_attribute`, `item`, `item_icon`, `item_stack`, `item_table`, `item_tooltip`,
  `menu_geometry`, `menu_key`, `player_holders`, `registries`, `mcrs_minecraft_resource`,
  `mcrs_minecraft_text`. Dev-dependencies: `game_registries`, `inventory_file`, `model`,
  `mcrs_test_alloc`, `primitives` with `test-support`. It has no edge to `entity_render` or `pose`.

**`mcrs_minecraft_gui_creative`** (16 files). Modules: `creative_clicks`, `creative_grid`,
`creative_inventory_clicks` (→, the creative half of `inventory_clicks`),
`creative_item_tooltip`, `creative_screen_draw`, `creative_screen_input` (merges
`creative_screen_edit` and `creative_pointer`), `creative_screen_plugin`,
`creative_screen_state`, `creative_search_edit`, `creative_search_index`,
`creative_tab_content`, `creative_tab_view`, and the three test modules
`creative_item_tooltip_tests`, `creative_screen_tests` and `creative_screen_view_tests`.

- `mark_creative_screen`, `creative_screen_shown` and `creative_screen_open` end with the screen
  entity in step 9. `loaded_creative_report` ends in step 17: `mcrs_minecraft_creative_report`
  reads the embedded report, and its `bevy` feature derives the resource.
- `creative_inventory_clicks` selects the search tab on `CHAT_KEY`. The plugin registers the
  stitch requests of the tab sprites, the scroller sprites and the creative backgrounds.
  `FigurePlacement::for_creative_tab` joins `creative_screen_state`.
- Public names: `CreativeScreenPlugin`, `CreativeContentSystems`.
- Dependencies: `container`, `container_gesture`, `menu`, `primitives`, `scene`, `atlas`,
  `font`, `gui_screen`, `client_widget`, `client_world`, `client_options`, `chat_client`,
  `network`, `creative_content`, `creative_report`, `container_click`, `inventory_click`,
  `inventory_client`, `inventory_menu`, `engine_inventory_click`, `engine_inventory_menu`,
  `engine_item`, `engine_registries`, `game_mode`, `entity_attribute`, `item_icon`, `item_key`,
  `item_stack`, `item_table`, `item_tooltip`, `menu_geometry`, `player_holders`, `registries`,
  `mcrs_minecraft_resource`, `mcrs_minecraft_text`. Dev-dependencies: `game_registries`,
  `inventory_file`, `model`, `mcrs_test_alloc`, `primitives` with `test-support`.

**`mcrs_minecraft_gui_hud`** (3 files). Modules: `health_hud`, `game_mode_switcher`.

- `HudTicks` and `count_hud_ticks` end. The heart shake reads `GuiTick`.
- `game_mode_switcher` uses `grab` of `cursor_grab`, so the HUD depends on `menu`. It reads
  `DebugModifier`, `DebugOverlay` and `DebugOverlayToggle` of the scene, not of `gui_debug`.
- Public names: `HealthHudPlugin`, `GameModeSwitcherPlugin`.
- Dependencies: `menu`, `primitives`, `scene`, `font`, `gui_screen`, `client_world`,
  `game_mode`, `damage`, `entity_attribute`, `network`, `protocol`, `protocol_game`,
  `item_icon`, `item_key`, `item_stack`, `item_table`. Dev-dependencies: `model`,
  `mcrs_minecraft_resource`, `primitives` with `test-support`.

**`mcrs_minecraft_gui_debug`** (9 files). Modules: `debug_screen`, `debug_displayer`,
`debug_entry_fps`, `debug_entry_network`, `debug_entry_position`, `debug_entry_system_specs`,
`chunk_map`, `light_levels`.

- The crate moves today's modules as they are (determination 15). Step 2 makes the refresh
  condition a named function with `mcrs::event_condition`. It draws with Bevy UI and Bevy text.
- Public names: `DebugScreenPlugin`, `DebugScreenDisplayer`, `DebugScreenSet`, `ChunkMapPlugin`,
  `LightLevelsPlugin`.
- Dependencies: `scene` (for `DebugOverlay` and `DebugOverlayToggle`), `client_world`,
  `engine_world`, `network`, `render_probe`, `volume`, `mcrs_voxel`, `mcrs_minecraft_core`,
  `mcrs_minecraft_resource`, Bevy UI and text. Dev-dependency: `mcrs_minecraft_dimension`. It
  has no edge to `gui_screen`.

**`mcrs_minecraft_gui_options`** (9 files). Modules: `options_screen`, `option_sub_screen`,
`accessibility_settings_screen`, `controls_screen`, `mouse_settings_screen`,
`video_settings_screen`, `menu_options_systems`, `options_plugin` (→, from
`menu_screens_plugin`).

- The options screens keep `Backdrop::Default`. "Back" despawns the top, so the options return to
  the pause screen or to the title without an edge to `gui_session`.
- Dependencies: `menu`, `scene`, `font`, `gui_screen`, `client_widget`, `client_options`. It has
  no edge to `network`.

**`mcrs_minecraft_gui_session`** (13 files). Modules: `death_screen`, `death_screen_plugin`,
`disconnected_screen`, `level_loading_screen`, `message_screen`, `session_screens`,
`title_screen`, `title_draw`, `pause_screen`, `pause_overlay` (→), `singleplayer_entry` (→),
`session_plugin` (→, from `menu_screens_plugin`).

- `MessageScreen` holds `ScreenMessage` as its data. Step 11 moves `ScreenMessage` from
  `screen.rs` of `client_world` into `message_screen`.
- Dependencies: `options`, `menu`, `primitives`, `scene`, `atlas`, `font`, `gui_screen`,
  `client_widget`, `client_world`, `network`, `protocol_game`, `damage`,
  `mcrs_minecraft_core`.

**`mcrs_minecraft_gui_worlds`** (10 files). Modules: `select_world_screen`,
`create_world_screen`, `delete_world_screen`, `world_entry`, `world_rows`,
`world_list_error_screen`, `follow_create_world`, `follow_list_answer`, `worlds_plugin` (→, from
`menu_screens_plugin`). The plugin inserts `SingleplayerEntry`.

- Dependencies: `session`, `menu`, `scene`, `atlas`, `font`, `gui_screen`,
  `client_widget`, `client_world`, `game_mode`, `save`, `mcrs_minecraft_resource`.
  Dev-dependency: `mcrs_minecraft_core`.

**Not in a GUI crate.** `sign_text` (763 lines) reads `SignFamily` and `SignMount` and draws no
GUI. It moves into `mcrs_minecraft_client` (determination 8), beside `sign_text.rs`,
`name_tag.rs` and `glyph_page.rs`.

**Files that end.** These files of today's `src/` have no home. Each ends in the step named.

| File | Ends in | How |
|---|---|---|
| `mark_creative_screen`, `creative_screen_shown`, `creative_screen_open` | 9 | `CreativeScreen` sits on the open-menu entity. |
| `close_menu_screen`, `follow_open_menu` | 9 | They merge into `container_screen`. |
| `creative_screen_edit`, `creative_pointer` | 9 | They merge into `creative_screen_input`. |
| `world_presence` | 10 | `WorldShown` replaces `WorldPresence`. |
| `panorama_view` | 10 | Its table ends; its spin left in step 8 as `panorama_spin`. |
| `gui_plugin` | 8 | It splits into `gui_scene_plugin` (step 7) and `gui_render_plugin` (step 8). |
| `next_screen`, `follow_menu_events`, `project_menu` | 11 | The screen entities replace the central modules. |
| `menu_screens_plugin` | 11 | It splits into `session_plugin` and `options_plugin` (step 10) and `worlds_plugin` (step 11). |
| `loaded_creative_report` | 15 | `creative_report` reads the embedded report. |
| `sign_text` | 13 | It moves to `mcrs_minecraft_client`. |
| `menu_test_app`, `lib` | 22 | The crate ends. |

### Field leaks

The leaks fall into these groups. Each group has one owner and one fix. The total is 32.

| Group | Leaks | Owner | Fix | Step |
|---|---|---|---|---|
| Chat view | `ChatScreen::commands`, `ChatScreen::edit`, `CommandInput::list`, `CommandInput::entries`, `CommandInput::highlights`, `SuggestionMenu::entries` (6) | `mcrs_minecraft_chat_client` | A view API in that crate: the input runs, the highlights and the rows of the suggestion list | 9 |
| Creative search | `CreativeSession::items`, `::language`, `::registries`, `CreativeScreenState::search`, `CreativeTabContent::search`, `CreativeContent::search_list` (6) | `mcrs_minecraft_creative_content` | One type for a search: the query and its results. The screen state holds it and asks it. | 15 |
| Creative tab | `CreativeTab::kind` (1) | `mcrs_minecraft_menu_geometry` | An accessor for the tab kind | 15 |
| Hubs | `TitleActions` (8), `ClickState::last_screen`, `ClickState::last_menu`, `Widgets::screen` (11) | `gui_session`, `gui_worlds`, `gui_container` | The hubs dissolve. `Widgets::set_screen` has no caller after `project_menu` ends. | 9 (`ClickState`), 11 |
| Gestures | `MenuGesture::release`, `MenuGesture::remembered`, `OpenDrag::kind` (3) | `container_gesture` | Methods on the kernel types | 3 |
| Layer | `GuiLayer::quads`, `GuiLayer::icons` (2) | `gui_scene` | Methods on `GuiLayer` that push and read the quads and the icons. The fields become private. | 2 |
| Small | `MenuLabelText::title`, `MenuLabelText::inventory`, `TitleScreenState::logo` (3) | `gui_container`, `gui_session` | A label method; data of `TitleScreen` | 3, 10 |

Step 2 ended the leaks `Pointer::buttons` and `Table::stored`. No step moves a leak to a new
crate. Each leak ends before the move of its owner.

### Dialog readiness

`docs/spec/dialog.md` section 18 lists the widgets. Sections 11 and 13 give the flow. This page
does not design the dialog crate `mcrs_minecraft_gui_dialog`. The split makes every part that the
dialog crate needs ready, so that crate adds only its own kind, layout and actions.

**Limits on the dialog crate.**

- It has no edge to `client_world`, to a player or to a container crate. A dialog can show in
  the configuration phase, where no world exists (DLG-FLOW-005).
- Its dependencies are `font`, `atlas`, `scene`, `primitives`, `menu`, `gui_screen`,
  `client_widget`, `item_stack`, `item_tooltip`, `network` and `chat_client`. The item crates
  draw the item element and its tooltip. `network` sends `custom` actions and the disconnect of
  the warning screen. `chat_client` writes `suggest_command` into the chat (DLG-CLK-001).
- It requires no `Backdrop`, so `Default` applies (WID-SCR-008). In the configuration phase
  `WorldShown` is `false`, and the dialog draws the panorama with blur and the menu texture.
- It requires `EscapeRule::Refuse` and runs its cancel action on Escape (DLG-KEY-002).

**What the split adds for dialogs.** Each gap of the screen model and of the widgets has a step.

| Need | Spec | Part | Step |
|---|---|---|---|
| Replace a screen below the top; remove a screen anywhere | DLG-FLOW-002, DLG-FLOW-003, section 18 | `replace: At(Entity)` and the remove observer of `ScreenEntry` | 9 |
| Each screen that the network opens replaces any open screen | DLG-FLOW-005 | `All` for container, disconnect, level loading, reconfiguration, death | 9, 10 |
| The chat key opens nothing over a screen | CHAT-OPEN-001 | `gui_chat` tests the empty stack; `AllowsChat` does not exist | 9 |
| An Escape refusal that changes while the screen is open | DLG-WAIT-001 | `EscapeRule` | 9 |
| A screen that builds its widgets again on a return loses its focus | WID-SCR-003, MENU-PROP-001 | The layout system of the kind runs on the insert of `TopScreen` | 10 |
| A caption is styled text | DLG-DATA-006, DLG-IN-002 | `Widget` loses `caption`; `Menu` keeps styled text for each widget | 14 |
| The pause of a local world | MENU-PAUSE-009, DLG-KEY-003 | `WorldPause` and the reader in the client app | 10 |
| A focusable message | DLG-BODY-001, DLG-SCR-010, DLG-CLK-007 | `WidgetKind::Message` | 14 |
| A focusable item box | DLG-BODY-002, DLG-SCR-011 | `WidgetKind::ItemBox` | 14 |
| A scroll container in a scroll panel | DLG-CTL-008, DLG-DEV-003 | `ScrollPanel` and the wheel route | 14 |
| Hover and click events of text; tooltips of a hover event | DLG-SCR-010, DLG-SCR-011 | `Menu` with `span_at` and `SpanEvents` of `font` | 14 |
| A multi-line edit box | DLG-CTL-005 to DLG-CTL-008 | `WidgetKind::MultiLineEditBox` | 15 |

**Step 14 in `client_widget`.**

- `Widget::caption` ends. The widget crate never reads the caption: it takes the text width from
  the host. `Menu` keeps the caption of each widget as styled text, a list of `Span` of `font`,
  by `WidgetId`.
- `WidgetKind::Message` is a text kind that takes the focus. It has a padding of 4 and draws its
  outline only when it has the focus. A press and an activation key give an event with the point
  in the text that DLG-CLK-007 states. The widget crate gives the default pointer shape for it.
- `WidgetKind::ItemBox` takes the focus, has a set size and has no caption.
- `ScrollPanel` accepts a scroll container in its area. The wheel goes to the innermost widget
  under the pointer that can scroll in that direction, then to the panel (DLG-DEV-003).

**Step 14 in the menu host.**

- The host draws the styled caption of each widget. Over a `Message`, it finds the span under
  the pointer with `span_at`. It shows the hand over a span with a click event, and it writes
  the tooltip of a hover event (`show_text`, `show_item`, `show_entity`) into `Tooltip`.
- A click on a span with a click event becomes a `MenuEvent` with the click of the span.
- The host draws the icon and the decorations of an `ItemBox` with `primitives`. The kind gives
  the tooltip lines of the stack, so the menu host takes no edge to `item_tooltip`.

**Step 15.** `WidgetKind::MultiLineEditBox` in `client_widget` and its draw in the menu host. The
step follows WID-MLE-001 to WID-MLE-023 of `docs/spec/client-widget.md` and the deviations
WID-DEV-003 to WID-DEV-011.

**The pause of a local world.** MENU-PAUSE-009 and DLG-KEY-003 state it: every screen pauses
a local world that is not open to LAN, unless its kind opts out. Chat, container, creative, sign,
death, level loading, title, the game mode switcher and the waiting screen require `Run`. A dialog
writes its `pause` field at spawn. The client app is the one reader: it pauses the tick of its
local server while the top screen has `Pause`. Step 10 adds the reader with the pause screens.

## Migration

The migration is one pull request. Each step is one commit, and each commit builds and passes
the tests. The step that moves the last caller of a path deletes the path. The old crate loses
modules as they leave. A step that changes a client crate runs the skill `run-client`.

Determination 13 gives the order. A redesign comes before the move of its part. A move comes as
soon as its redesign is done and the crates that it needs have moved. A move never needs a crate
that has not moved yet.

Line counts are estimates of the added lines, from `wc -l` of the modules that change or move.

| Step | Crate group | Content | Lines |
|---|---|---|---|
| 1 | `client_gui` | Done: split `scene.rs`, add `GuiPhase`, make `Font` a resource, break the cycles (5 to 0) | about 3,200 added, 2,700 deleted |
| 2 | `client_gui`, `client` | The retained scene. Done: `GuiSize`, `GuiPointer`, retained layers, the atlas asset, the glyph upload list. Open: the F3 condition, the layer leaks, `GuiPointer` of WID-PTR-005, the creative tab figure. | about 2,600 |
| 3 | `client_gui` | The gesture kernel without Bevy, without creative and without `item_stack`; the split of `gesture_frame`, `viewer` and `drag_preview` | about 700 |
| 4 | `container_gesture` | Move the kernel as a `no_std` crate | about 500 |
| 5 | `client_gui`, `client` | The stitch requests; `menu_background` out of the atlas; `sprite_draw`; `in_game_gradient`; `gui_clock`; `debug_chord`; `GuiPlugin` loses `screen` | about 500 |
| 6 | `resource_pack`, `model_file`, `model`, `font`, `client` | A redesign step: create `resource_pack` with the missing texture; the font loader takes bytes and returns errors; move the font; `client` reads the jar | about 2,000 |
| 7 | `gui_atlas`, `gui_scene`, `gui_primitives` | Move the atlas, the scene and the primitives; split `menu_test_app` and `slot_icon` | about 2,700 |
| 8 | `gui_render` | Move the batch, the pipeline, the extract and the panorama spin | about 1,500 |
| 9 | `gui_screen`, `client_world`, `chat_client`, `inventory_client`, `client_gui`, `gui_render`, `client` | The stack (`no_std`), chat, container, creative and the game mode switcher as entities; `ShowsFigure`; the split of `inventory_clicks` | about 2,100 |
| 10 | `gui_screen`, `client_gui`, `gui_render`, `client` | `Menu` as a component, the session screens, title, pause and options as entities; `project_backdrop` in `gui_render` | about 2,200 |
| 11 | `client_gui`, `client_world`, `client` | The world screens as entities; `SingleplayerEntry`; `ScreenMessage` as data of `MessageScreen`; delete `Screen`, `next_screen`, `follow_menu_events`, `project_menu` | about 1,700 |
| 12 | `mcrs_minecraft_render`, `client_options`, `client_gui`, `gui_render` | The option "Menu Background Blur" and its slider; `BackdropBlur`; the blur node after the HUD band; the split of the batch draw; the radius in `project_backdrop` | about 900 |
| 13 | `client` | Move `sign_text` | about 780 |
| 14 | `client_widget`, `client_gui` | Dialog readiness: styled captions in `Menu`, `Message`, `ItemBox`, the nested scroll, the span events in the host | about 1,500 |
| 15 | `client_widget`, `client_gui` | The multi-line edit box | about 900 |
| 16 | `gui_menu` | Move the menu host; add its own `menu_test_app` | about 2,300 |
| 17 | `creative_content`, `creative_report`, `menu_geometry`, `client_gui` | The search type, the tab accessor, the embedded report | about 400 |
| 18 | `gui_chat` | Move chat | about 1,400 |
| 19 | `gui_figure` | Move the inventory figure | about 450 |
| 20 | `gui_container` | Move the container screens | about 2,900 |
| 21 | `gui_creative` | Move the creative screen | about 3,200 |
| 22 | `gui_debug`, `gui_hud` | The F3 screen and the HUD | about 1,400, 900 |
| 23 | `gui_options`, `gui_session`, `gui_worlds` | In this order | about 800, 1,400, 1,500 |
| 24 | `client`, workspace | Delete `client_gui`; update `docs/crates.md` | small |

Step 13 is independent. It can come at any point before step 24.

What each move needs, and the step that provides it:

| Move | Needs | From step |
|---|---|---|
| 4 `container_gesture` | `container_click`, `inventory_click`, the split of the kernel | exists, 3 |
| 6 `font` | `resource_pack` | 6 |
| 7 `gui_atlas` | `font`, `resource_pack`, the stitch requests, the atlas test in `sprite_draw` | 5, 6 |
| 7 `gui_scene` | `gui_atlas`, `font`, `gui_clock`, `debug_chord`, `GuiPlugin` without `screen` | 5, 6, 7 |
| 7 `gui_primitives` | `gui_scene`, `gui_atlas`, `font`, `sprite_draw`, `in_game_gradient` | 5, 6, 7 |
| 8 `gui_render` | `gui_primitives`, `gui_scene`, `gui_atlas`, `font` | 7 |
| 14 dialog readiness | `Menu` as a component, the screen entities | 10, 11 |
| 15 multi-line edit box | step 14, WID-MLE-001 to WID-MLE-023 | 14 |
| 16 `gui_menu` | `Menu` as a component, `gui_screen`, the primitives | 7, 9, 10 |
| 18 `gui_chat` | `ChatScreen` as an entity, `gui_menu` | 9, 16 |
| 19 `gui_figure` | `ShowsFigure`, `gui_primitives` | 7, 9 |
| 20 `gui_container` | `container_gesture`, `gui_menu`, the stitch requests, the split of `inventory_clicks` | 4, 5, 9, 16 |
| 21 `gui_creative` | `gui_container`, the search type | 17, 20 |
| 22 `gui_debug` | `gui_scene` with `debug_chord` | 7 |
| 22 `gui_hud` | `gui_menu`, `gui_primitives`, `debug_chord` | 7, 16 |
| 23 `gui_options` | `gui_menu`, the screen entities | 10, 16 |
| 23 `gui_session` | `gui_options`, `gui_menu`, the screen entities, `ScreenMessage` as data | 11, 16, and the move of `gui_options` before it |
| 23 `gui_worlds` | `gui_session` | the moves before it in step 23 |

### Step 2: the retained scene

The step is done: the resources `GuiSize` and `GuiPointer`, the layers, the draw gates, the
two sets of `GuiPhase`, the batch crossing, the atlas as an asset (`FontCells` and
`gui_atlas_image` gone), `HudTicks` gone, and the figure of CCI-LAY-007. The F3 refresh is a
named `mcrs::event_condition`, which ends the 8 `alloc_in_tick` expectations of the debug
entries. `GuiLayer` keeps its quads and icons private and resolves the icon index itself.
`GuiPointer` follows WID-PTR-005: the last point of the cursor-moved messages, never cleared.
`deslop-please` reports 30 field leaks and 0 module cycles.

The step changes one behaviour: the figure of CCI-LAY-007. A test of that figure, the existing
client tests and a screenshot of the title, a container, the creative screen and the F3 screen
check the step.

### Step 3: the gesture kernel

The step is done. It splits `gesture_frame`, `viewer` and `drag_preview`, as the table in
section "The container gesture kernel" states. The Bevy halves `gesture_input` and
`shown_drag_preview` move with `gui_container` in step 20. `MenuLabelText` draws its own labels,
which ends its two leaks. `ShownDragPreview` changes only with the preview, so an idle frame draws
no container. The client tests `creative_screen_clicks` and `screen_clicks` and the kernel tests
of the double click and the drag check the step. `deslop-please` reports 25 field leaks.

### Step 5: the atlas stitch requests

The step makes the atlas name no screen (determination 21). It adds `StitchRequest` and
`StitchRequests`, and the container plugin and the creative plugin register their requests.
`gui_atlas_load` loses `CREATIVE_TABS`, `CreativeGeometry` and `menu_background`. Five small cuts
prepare the move of step 7.

- `GuiSprite::draw` moves to `sprite_draw` as `draw_sprite`, with the test of the nine-slice
  button.
- `menu_clock` moves from `menu_input` into `layer_due` as `gui_clock`.
- `DebugModifier` and `DebugOverlay` move from `debug_screen` into `debug_chord`, with the new set
  `DebugOverlayToggle`. The game mode switcher and its tests no longer read `debug_screen`.
- `DIM_TOP` and `DIM_BOTTOM` move from `menu_screen` into `in_game_gradient`. So `menu_draw` and
  `creative_screen_draw` no longer read `menu_screen`.
- `GuiPlugin` loses its field `screen`. The client app inserts the initial `Screen` itself until
  step 11.

A test loads the atlas with the requests of both plugins and checks the regions of each
background and each tab sprite. The existing atlas tests check the rest.

### Steps 9 to 12: the screen entities

These steps must also dissolve the three central modules (839 lines), because they match on
`Screen`. Steps 9 to 11 carry about 6,000 added lines, and step 12 adds the blur.

- **9** adds `mcrs_minecraft_gui_screen` and moves the screens of the game: chat, container,
  creative and the game mode switcher. `ChatScreen` gets its `bevy` feature, and `ChatInput`
  ends. `CreativeScreen` moves to the open-menu entity, and `mark_creative_screen` ends. The
  chat view API ends six leaks. `ClickState` loses two fields. `ShowsFigure` replaces the read of
  `placed_figure`. The batch of `gui_render` reads `ScreenStack`. `inventory_clicks` splits into
  its container half and `creative_inventory_clicks`. `ScreenEntry` has the four modes of
  `replace`, `EscapeRule` replaces the marker of a refusal, and a container spawns with `All`.
- **10** makes `Menu` a component, adds the layout system of each kind, and moves the session
  screens, the title, the pause screen and the options screens. `WorldPresence` and
  `MenuBackground` end. `Backdrop` and `WorldShown` join `gui_screen`, and the client app writes
  `WorldShown`. `project_backdrop` lands in `gui_render`, and the table of `panorama_view` ends.
  The death screen and its confirmation require `Backdrop::Gradient`. `PauseOverlay` arrives
  with the chord F3 and Escape. `WorldPause` and its reader arrive. The disconnect, level loading
  and death screens spawn with `All`. The pause screen builds its `Menu` again on a return. The
  central modules lose the arms of these screens.
- **11** moves the world screens, deletes the central modules, and deletes `Screen` and its three
  parent enums from `client_world`. `ScreenMessage` leaves `screen.rs` of `client_world` and
  becomes the data of `MessageScreen` in `message_screen`. The title opens the world list through
  `SingleplayerEntry`.

Step 12 changes the render crate, the options crate and the GUI crates in one commit. The blur
needs a node, a radius and a split draw together. A split in two would leave a node with no
writer.

Between steps 9 and 11, `Screen` loses variants as screens move. A system that asks "is a screen
open" reads both the enum and `ScreenStack` until step 11. That is one migration with one end.
Step 11 deletes the old path. Each step reproduces today's screens, their parents and their Escape
rules. It adds no feature, except the behaviours of the table in section "The screen model".

### The moves

Steps 4, 7, 8, 13, 16 and 18 to 23 are pure move steps. They move modules with no edit besides
`use` paths and `Cargo.toml` (determination 13). Each move shortens the dependency list of the
old crate and gives the new crate only what it uses. Some of them also move items between files
along a crate boundary, and some edit the client app.

- Step 7 splits `gui_plugin`, `slot_icon`, `draw_gate` and `menu_test_app` along the crate
  boundaries.
- Step 8 adds `GuiRenderPlugin` to the client app only when the app has a window. It changes
  the embedded path of the shader to the name of the new crate.
- Step 16 adds the test module `menu_test_app` of `gui_menu`.

Four steps are redesign steps, not pure moves.

- Step 6 creates `resource_pack` with the PNG decoder and the missing texture. It changes the font
  loader: it takes bytes, and a warning becomes an error value. Then it moves the font.
- Steps 14 and 15 make the widgets and the menu host ready for dialogs, before the menu host
  moves (section "Dialog readiness").
- Step 17 adds the search type, the tab accessor and the embedded report.

### What deslop-please and the ECS lint report

The measured state of the working tree in step 2, today: 32 field leaks, 156 entries in the old
`src/`, 13 `alloc_in_tick` expectations, 1 `ungated_system`, 1 `blocking_in_system`, 1
`marker_toggle` and 0 `missed_set_if_neq`. A step must not add a finding or an expectation. The
pull request body carries the measurement for each step. The 156 entries stay one `wide-directory`
finding until the crate ends.

What stays: `chunk_map` formats while the F6 view shows. The creative search copies the selected
text on a key press. `debug_entry_fps` reads the wall clock to measure frame times. The seven
expectations of `clippy::too_many_lines` fall to five in step 11, because `follow_menu_events`
and `project_menu` end.

Each new crate starts with no field leak. A new crate above 15 entries keeps its
`wide-directory` finding when no natural boundary splits it (determination 12). `crates/` stays
wide; `.deslop-please.allow` allows it.

## Options

The maintainer considered these options.

| Option | Result |
|---|---|
| Keep one crate and allow the findings | Rejected (determination 1). |
| Modules inside one crate | Rejected. The layout rule keeps `src/` flat, and a crate that needs `foo/mod.rs` is too big. |
| A split at each 15 entries | Rejected (determination 12). The threshold is a default of the tool. A split with no dependency and no boundary only moves the finding. |
| A crate `gui_menus` for the title, the pause and the options | Rejected. The pause button sends `LeaveRequest`, so that crate takes `network`, as `gui_session` does. The title and the pause go to `gui_session`, and the options to `gui_options`. |
| `gui_options` inside `gui_session` | Rejected. `gui_session` takes `network`, and a tool that shows only the settings does not. |
| The screen stack in a client part above layer 4 | Rejected (determination 5). The stack holds no network and no world, so it is layer 6, and `gui_render` and `gui_menu` may take it. |
| `inventory_clicks` whole in `gui_container` | Rejected. It imports eight creative modules and the chat key, so `gui_container` would depend on `gui_creative` and `gui_chat`. |
| The kernel writes the resource `DragPreview` | Rejected. The kernel has no Bevy. It returns the value, and `gui_container` writes the resource. |
| `gui_primitives` inside `gui_scene` | Rejected. The primitives take the font layout and the item crates, and the scene does not. |
| The figure inside `gui_container` | Rejected (determination 22). The container screens would take `entity_render`. |
| Keep the immediate-mode scene; the draw order as a chain of sets | Rejected (determination 10). |
| Move the atlas by `ResMut<MainWorld>` | Rejected (determination 11). |
| The atlas stitches the screen backgrounds | Rejected (determinations 3 and 21). A new screen with its own background would change the atlas, and the atlas would depend on `menu_geometry` and `menu_key`. |
| `zune-png` for a `no_std` `resource_pack` | Deferred. `zune-png` with its default features off is `no_std` with `alloc`. Its support of palette images and 16-bit images is not verified. Revisit when a `no_std` consumer of `resource_pack` exists. |
| Provisional crates and a review after a move | Rejected (determination 12). |
| Moves before the redesign, or all moves at the end | Rejected (determination 13). |
| The F3 screen stays in `gui_hud` | Rejected (determination 15). |
| The title depends on `gui_worlds` | Rejected (determination 16). |
| A central table from screen to background | Rejected (determination 20). |
| Several marker components for the background | Rejected (determination 20). The variants exclude each other. |
| `WorldPresence` or `MenuBackground` as a field of a screen | Rejected (determination 20). |
| Bevy `States` for the screen | Rejected. `States` holds one value at a time and has no stack. The set of values is closed. |
| A stack of ids, or the `Screen` enum with a stack | Rejected. An id table is a second registry beside the ECS. The enum names every screen in one crate, so a dialog cannot join it. |
| Messages `EscapePressed` and `CloseTopScreen` | Rejected. One system with a `Single` query does the same with no hop. |
| A relationship for the stack | Rejected. Section "The screen model" gives the reasons. |
| A tick system `mark_top_screen` | Rejected. The lint reports it as `marker_toggle`. The observers of `ScreenEntry` move the marker instead. |

## What would reopen this

- A crate of the split takes a dependency that only a part of it needs.
- A measurement shows that the retained scene costs more main-world time than the immediate
  scene in a frame with a change, for example a pointer move over a large menu.
- A pack reload loses glyphs or shows a frame with a wrong atlas.
- A screen needs a fact that the stack of entities cannot give.
- The dialog crate needs the player or the world in the configuration phase.

## Objections

These are the strongest objections that the text does not already answer.

- Eighteen crates for one concern. `rust-style` says to split a crate only when a dependency
  forces it. Determination 12 overrides that rule for this RFD. The column "What forces it" gives
  a dependency or a boundary of meaning for each crate.
- `gui_menu`, `gui_container` and `gui_creative` have 17, 16 and 16 files at their move. 15 is a
  guide, not a rule, and such a crate splits only at a natural boundary.
- The retained scene adds a gate on each draw and a trigger for each input. A missed trigger
  shows a stale layer. The trigger list is short and the same for every draw, and a test that
  changes one trigger and reads the layer catches a stale one.
- The redesign comes first and touches every draw system. Each redesign step has the client tests
  and a screenshot run, and a move follows soon, so a mistake shows in a small diff.
- `gui_render` reads `ScreenStack`, so the renderer knows the stack. The answer is that it knows
  the generic stack and no screen kind. Both crates are in layer 6.
- With no id registry, no code lists all screens. A query over `ScreenEntry` lists the open
  screens at run time, and each kind component is a type that a search finds.

## Open questions

None.

## References

- [RFD 1: Requests for Discussion](0001-requests-for-discussion.md).
- [RFD 2: Mission, principles and values](0002-mission-principles-values.md).
- `docs/spec/dialog.md`: sections 11, 13 and 18, DLG-FLOW-001 to DLG-FLOW-005, DLG-KEY-002.
- `docs/spec/client-chat.md`: CHAT-LOG-003.
- `docs/spec/client-creative-inventory.md`: CCI-LAY-007.
- `docs/spec/client-edit-box.md`: EDIT-CUR-002.
- `docs/spec/client-player-render.md`: CPR-FIG-003.
- `docs/spec/client-widget.md`: WID-PTR-005, WID-SCR-004, WID-SCR-005 and WID-SCR-008.
- `docs/ecs-lint.md`: section 5 (rules) and section 10 (event conditions).
- `CLAUDE.md`: Layers, ECS, Crossing a boundary, Ownership, locks, dispatch and allocation.
- `.agents/skills/rust-style/SKILL.md`: Crates and modules.
- `.agents/skills/bevy-ecs/SKILL.md`: the classification of values.
