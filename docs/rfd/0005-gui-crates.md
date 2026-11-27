# RFD 5: Split the client GUI into crates by screen

- Authors: andreypfau
- State: prediscussion
- Labels: architecture, client, gui

This page proposes to replace the crate `mcrs_minecraft_client_gui` with fifteen small crates.
It also proposes a retained GUI scene and a screen model that a dialog screen can use. Read it
if you touch a screen of the client, if you plan the dialog screen, or if you wonder why the GUI
is not one crate.

This is the second draft. Step 1 of the first draft is done in the working tree. A review of
2026-11-27 measured the result and found that the first draft kept the cause of most lint
expectations. The maintainer then decided four points (determinations 10 to 13). Later answers
added determinations 15 to 20. The text marks each fact that the code changed with the words
"The code says". Section "Open questions" lists what is still open.

## The problem

The crate `mcrs_minecraft_client_gui` has 142 Rust files in a flat `src/` and 24,409 lines. Its
only consumer is `mcrs_minecraft_client`. The maintainer has three goals for it, in this order.

1. A clean and simple architecture that obeys `CLAUDE.md`.
2. Fewer findings of `deslop-please` and fewer expectations of the ECS lint, with no new
   exception.
3. Modular code that a user can take in parts, and that an agent can read in parts.

The crate misses each goal today.

- **Wide directory.** `src/` holds 142 entries. The limit is 15. `deslop-please` reports
  `wide-directory`.
- **Field leaks.** `deslop-please` reports 32. Each leak is a field that code outside its type
  reads. The central modules and `ClickState` cause 11 of them.
- **An immediate-mode scene.** `begin_gui_frame` clears the scene on each frame. Each draw system
  then pushes its quads again, and the batch is built, cloned and uploaded again. The lint
  expectations of the crate follow from this. Section "What is known" counts them.
- **A draw order in the schedule.** `GuiPhase` has five sets, and the order of two draws in one
  set is the order of the systems. A title screen draws a menu and a logo. A world screen draws a
  menu and a message. The chat draws under a container screen. Each case needs a new `after`.
- **A fixed screen enum.** `Screen` in `mcrs_minecraft_client_world` has 19 variants, and 27
  files of the GUI match on it. Three central modules (`next_screen`, `follow_menu_events`,
  `project_menu`) know every screen. A dialog that the server shows in the configuration phase
  has no place in that enum. `docs/spec/dialog.md` needs a previous screen, an Escape that a
  screen can refuse and a screen that the network replaces (sections 11 and 13).
- **A heavy dependency list.** The crate has 69 normal dependencies. A change to the title
  screen rebuilds chat, creative and the F3 screen. A user of one screen pays for all.

## What is known

Each fact has the way it was found.

| Fact | Source |
|---|---|
| 142 Rust files and one shader, 24,409 lines | `wc -l crates/mcrs_minecraft_client_gui/src/*.rs` |
| 0 module cycles, 32 field leaks, 2 `wide-directory` (`crates/` and `src/`) | `deslop-please mcrs_minecraft_client_gui` |
| 58 `pub mod`, about 35 import paths in `mcrs_minecraft_client` | `src/lib.rs`, `grep client_gui:: crates/mcrs_minecraft_client/src` |
| 29 expectations of the ECS lint, 7 of `clippy::too_many_lines` | `grep -n 'mcrs::' src/*.rs` |
| `Screen` is matched in 27 GUI files and 5 files of `mcrs_minecraft_client` | `grep -l 'Screen::'` |
| `ChatInput(pub ChatScreen)` is a resource of `client_world` | `mcrs_minecraft_client_world/src/chat.rs` |
| `Chat::tick` changes on each fixed tick | `advance_tick` in the same file |

The 29 expectations of the ECS lint, and their cause:

| Lint | Count | Where | Cause |
|---|---|---|---|
| `alloc_in_tick` | 20 | `chat`, `container_draw`, `creative_item_tooltip`, `menu_item_tooltip`, `menu_draw`, `title_draw` | A draw system runs on each frame, because the scene is cleared on each frame. |
| | | `debug_screen` (2), `debug_entry_fps` (2), `debug_entry_position`, `debug_entry_network`, `debug_entry_system_specs` | The refresh condition is a closure, so the lint does not see an event condition. |
| | | `follow_menu_events`, `follow_create_world`, `world_rows`, `chat_input`, `inventory_clicks` | A system reads the screen or the keys on each frame and allocates on a rare branch. |
| | | `chunk_map`, `gui_items` | The F6 view while it shows; the creation of GPU resources. |
| `ungated_system` | 6 | `gui_batch_build`, `gui_items` (3) | The batch is rebuilt and uploaded on each frame. |
| | | `health_hud` | `count_hud_ticks` is a second tick counter. |
| | | `inventory_clicks` | `ClickState` copies the screen and the open menu on each frame. |
| `blocking_in_system` | 1 | `debug_entry_fps` | Frame times are wall-clock instants. |
| `marker_toggle` | 1 | `mark_creative_screen` | A tick system inserts and removes `CreativeScreen` on the player. |
| `missed_set_if_neq` | 1 | `gui_frame` | `begin_gui_frame` writes the scene on each frame. |

`docs/ecs-lint.md` (section 10) lists the run conditions that count as an event:
`on_message`, `resource_changed`, `input_just_pressed`, `any_match_filter` with `Changed` or
`Added`, `any_component_removed` and a function with `mcrs::event_condition`. A closure never
counts. A system with such a condition is not checked for `alloc_in_tick`.

The field leaks fall into six groups. Section "Field leaks" names the owner and the fix of each.

## Determination

The maintainer decided these points. The numbers are the numbers of this page.

1. **The GUI splits into crates.** The maintainer rejected the option to keep one crate and
   allow the findings.
2. **The font is its own resource and its own crate**, `mcrs_minecraft_font`, in layer 3. It is
   no field of the atlas. It has no dependency on `client_jar`. The caller gives the font bytes,
   so a map viewer can take the crate alone. `mcrs_minecraft_client` reads the jar. The font
   reads pack files through `PackFiles` of a new Bevy-free crate `mcrs_minecraft_resource_pack`
   (layer 3), not through `ResourceSet` of `mcrs_minecraft_model`.
3. **The scene knows no screen.** No module of the scene names a screen, and no screen orders
   itself against another screen.
4. **A screen is an entity.** Its kind is a component that the crate of the screen owns. The
   stack keeps a previous screen, an Escape that a screen can refuse and replacement by the
   network.
5. **`client_screen` sits above the engine**, in the client part of the layers. It uses Bevy
   components and observers, so it is not layer 3.
6. **`gui_chat` owns the chat key.** `client_screen` has the marker `AllowsChat`. The chat key
   opens the chat when no screen is open or when the top screen has `AllowsChat`. The container
   crates do not depend on `gui_chat`.
7. **`gui_render` is separate.** A headless client and a bot take no wgpu.
8. **`sign_text` moves into `mcrs_minecraft_client`.** It lays out sign text and draws no GUI.
9. **The migration lands as one pull request.** Each step is one commit in it, and each commit
   builds and passes the tests. A step may touch any crate group. The maintainer is a code owner,
   so the limits on the size and the crate groups of a pull request do not apply.
10. **The scene is retained.** Each screen or HUD entity owns a `GuiLayer`. A draw system writes
    its layer only on a change. The order is the key of the layer, not the order of the systems.
    `GuiPhase` shrinks to `Begin`, `Draw` and `Batch`, and keeps its name.
11. **The atlas is a Bevy `Image` asset** with `RenderAssetUsages::RENDER_WORLD`. The main world
    rasterises a missed glyph into an upload list. `FontCells` ends. `Font::unifont_cell` is the
    one path to a unifont cell.
12. **Dependencies and the limit of 15 entries force the crates.** No crate exists for a topic
    alone. `container_gesture` has no Bevy, and the creative dependency is inverted. No crate is
    provisional, and no review after a step decides the map.
13. **Redesign first, move last.** Step 1b and step 1c redesign inside `client_gui`. Steps 2a to
    2c make the screens entities and dissolve the central modules. The moves come after.
14. **The chat draws in the HUD band.** It shows under a container screen and under the creative
    screen.
15. **The F3 screen is its own crate**, `mcrs_minecraft_gui_debug`, in layer 7. `gui_hud` loses
    the F3 modules and the dependencies `render_probe` and `volume`. This RFD fixes only the
    crate boundary and moves today's modules as they are. The inner design of the F3 screen, its
    entries and how they register, will be rebuilt later from its own specification in
    `docs/spec/`.
16. **The title opens the world list through a stored function pointer.** `gui_menus` defines
    the resource `SingleplayerEntry(fn(&mut Commands))`. The plugin of `gui_worlds` inserts it
    with a function that spawns the world list. The title shows the singleplayer button only
    when the resource exists. So `gui_menus` does not depend on `gui_worlds`, and a client that
    only joins servers does not take `save`. A function pointer needs no registration and no
    `dyn`, so it is simpler than a `SystemId`. The edge is `worlds` to `menus`.
17. **`prepare_gui_pass` runs only on a change** of the window view or of the atlas asset. A
    named condition with `mcrs::event_condition` tests the change. The system carries no
    ungated expectation.
18. **Each crate that can be `#![no_std]` with `alloc` is.** The column `no_std` of the crate
    table gives the result. `container_gesture` and `client_screen` are `no_std`. `font` is
    not, because its dependencies `text`, `lang` and `nbt` need `std`.
19. **The chat fades on the GUI tick, not on real time.** `docs/spec/client-chat.md` CHAT-LOG-003
    gives the rule. The GUI tick advances once for each client tick and stops while the game is
    paused. Each chat line stores the GUI tick at which it arrived; the age is derived. One
    resource `GuiTick` of the client holds the counter, and one system advances it in the fixed
    schedule. `Time<Fixed>` drives that schedule on native and in the browser. No code reads
    `std::time::Instant`, which the browser does not have. `Chat::tick` and `advance_tick` end.
    The chat layer redraws on a new line, on an opened or closed chat, and on a change of
    `GuiTick` only while a visible line is in its fade window (age 180 to 200). A named
    condition with `mcrs::event_condition` is true in these cases only. The action-bar overlay
    stores the GUI tick at which it was set and follows the same rule.
20. **The backdrop belongs to the kind of the screen, and the world is a fact of the client.**
    `client_screen` defines one enum component, `Backdrop`. Its variants follow WID-SCR-004 and
    WID-SCR-008 in `docs/spec/client-widget.md`.

    | Variant | What it draws |
    |---|---|
    | `Default` | With no world: the panorama, the blur and the menu background texture. In a world: the blur of the world and the HUD, then the in-world texture. |
    | `Panorama` | The panorama only. The title uses it. |
    | `PanoramaOverWorld` | The panorama, the blur and the texture, also in a world. The message screen and the level loading screen use it. |
    | `InGameGradient` | The gradient `C0101010` to `D0101010`. No blur and no texture. |
    | `Gradient { top, bottom }` | A vertical gradient of two ARGB colours. |
    | `None` | Nothing. The chat screen uses it. The game mode switcher also uses it, and draws its
      own texture in its own layer, so no texture name crosses into `client_screen`. |

    `ScreenEntry` requires `Backdrop`, and the value is `Default`. A kind overrides it with
    `#[require(Backdrop = ...)]`. Whether a world exists is not a field of a screen. The resource
    `WorldShown(bool)` of `client_screen` holds it. The client app is its one writer, from its
    world state, with `set_if_neq`. A dialog of the configuration phase sees `false`. One system
    in `gui_render` reads `Backdrop` of the top screen and `WorldShown`. It names no screen kind.
    `WorldPresence`, its `Player` query and the field `MenuBackground` of `Menu` end. No central
    table of screens exists. So `menus` needs no edge to `session` or to `worlds`, and a new
    screen picks its backdrop with its own `require`.
Determinations 10 to 13 replace three decisions of the first draft: the three menu crates, the
provisional crates `gui_creative_state` and `gui_gesture`, and the review after step 8.

## Proposal

### Layers and names

The names follow `CLAUDE.md`. The prefix `mcrs_minecraft_gui_` groups the GUI crates. The font and the scene are useful without a client, so the
word `client` is not in their names. Each crate keeps private modules and re-exports its public
names at the root. One public path exists for each item.

| Crate | Layer | `no_std` | Job | What forces it |
|---|---|---|---|---|
| `mcrs_minecraft_font` | 3 | no: `text`, `lang`, `nbt` | Font data, glyph cache, text layout, wrap, translation | No Bevy. A sign renderer or a map viewer takes it alone. |
| `mcrs_minecraft_container_gesture` | 3 | yes | The click, drag and quick-move gestures of a container | No Bevy. A bot takes it without a GUI. |
| `mcrs_minecraft_gui_scene` | 6 | no: Bevy asset, image, window | Layers, GUI size, pointer, clock, phases, atlas layout and load | The base of each draw |
| `mcrs_minecraft_gui_primitives` | 6 | no: Bevy | Text, tooltip, item icon, decorations, tile, box | The limit of 15 entries: scene and primitives hold 23 modules |
| `mcrs_minecraft_gui_render` | 6 | no: wgpu | The batch, the pipeline, the extract, the backdrop and the panorama view | wgpu (determination 7) |
| `mcrs_minecraft_gui_menu` | 6 | no: Bevy | The host of a widget menu: layout state, draw, input, cursor | `client_widget`, `client_options` |
| `mcrs_minecraft_client_screen` | Client, above 4 | yes | The screen stack, the shared parts of a screen, `Backdrop`, `WorldShown` | `bevy_ecs` only (determination 5) |
| `mcrs_minecraft_gui_chat` | 7 | no: Bevy | Chat lines, chat screen, chat key, suggestions | `chat_client`, `command` |
| `mcrs_minecraft_gui_container` | 7 | no: Bevy | Container screens, hotbar, inventory figure, item tooltip | `entity_render`, the inventory crates |
| `mcrs_minecraft_gui_creative` | 7 | no: Bevy | The creative screen | `creative_content`, `creative_report` |
| `mcrs_minecraft_gui_hud` | 7 | no: Bevy | Health, game mode switcher | `damage` |
| `mcrs_minecraft_gui_debug` | 7 | no: Bevy | F3 screen, chunk map, light levels | `render_probe`, `volume` (determination 15) |
| `mcrs_minecraft_gui_session` | 7 | no: Bevy | Death, disconnect, level loading, message, the session flow | `network` |
| `mcrs_minecraft_gui_worlds` | 7 | no: Bevy | World list, world create, world delete | `save` |
| `mcrs_minecraft_gui_menus` | 7 | no: Bevy | Title, pause, options and its sub screens | The count of the modules |

The code says: the review estimated about eleven crates. Three more are forced.

- Scene and primitives together hold 23 modules. One crate would add a `wide-directory` finding.
- Container and creative together hold about 27 modules. Creative also needs the content crates.
- Options, title, pause and the session screens together hold 18 modules and two plugins. One
  crate for the session screens and one for title, pause and options keep each crate under the
  limit.

The F3 screen is the fifteenth crate. The limit does not force it. Determination 15 sets it,
because the F3 screen takes `render_probe` and `volume`.

Layer 6 crates depend on neither the network, the client world nor the server. The screen crates
read the client world and the network, so they sit in layer 7. A later crate
`mcrs_minecraft_gui_dialog` is layer 7. Section "Dialog readiness" states its limits.

### Dependency graph

An arrow means "depends on". The graph shows only the direct edges between GUI crates.

```
font <- scene <- primitives <- menu <- chat
                     ^           ^
                     |           +-- container <- creative
                     |           +-- session <- worlds
                     |           +-- menus <- worlds
                     |           +-- dialog (later)
                     +-- hud
                     +-- debug
                     +-- render

container_gesture (layer 3, no Bevy) <- container, creative
client_screen (bevy_ecs only) <- render, menu, chat, container, creative, hud,
                                 session, worlds, menus, dialog (later)
```

Four rules hold for the graph.

- No edge goes up. `scene` and `primitives` name no screen.
- An opener depends on the crate of the screen that it opens, because it spawns the kind
  component. `worlds` opens a message screen, so it depends on `session`. The title is the one
  exception (determination 16): it opens the world list through `SingleplayerEntry`, so `worlds`
  depends on `menus`, and `menus` depends on neither `worlds` nor `session`. No cycle exists.
  No crate below opens the title. The title crate shows the title when no screen is open and no
  world exists.
- The chat key has no edge. `gui_chat` reads `AllowsChat` (determination 6).
- `render` is the only crate that needs the render world. It reads `ScreenStack` to find the
  top screen. A headless client leaves `render` out and keeps the rest.
- `render` depends on `mcrs_minecraft_render` for `PanoramaView`. It is the lowest crate that
  has this edge and sits below every screen crate, because no screen crate depends on `render`.
  `menu` depends on `client_screen` to read `WorldShown`. `client_screen` depends on no GUI
  crate. So the backdrop adds no upward edge, and `menus` loses its edge to `mcrs_minecraft_render`.

The code says: `mcrs_minecraft_client_world` does not need `client_screen`. No module of it reads
the screen after `Screen` ends. Only its chat systems change, because `ChatInput` ends.

### The retained scene

**The layer.** `GuiLayer` is a component. It holds the quads and the item icons of one part of
the GUI, and its key. The entity of a layer is a screen entity, a child of a screen entity, or an
entity that a HUD plugin spawns at startup.

| Field | Meaning |
|---|---|
| `band` | `Hud`, `Screen` or `Overlay`. The order of the bands is fixed. |
| `part` | A small number. It orders two layers of one band and one screen. |
| `quads`, `icons` | The output of the draw. The owner clears the vectors and keeps their capacity. |

The batch sorts layers by the key (band, depth, part). The depth is the index of the screen
entity in `ScreenStack`. A HUD layer and an overlay layer have the depth 0. The batch takes the
HUD band, the screen layers of the top screen and the overlay band. A screen with two parts
spawns a child entity for the second part. The despawn of the screen despawns the child.

Examples of keys:

| Layer | Key |
|---|---|
| Hotbar, health, chat lines, F3 screen | (`Hud`, 0, 0 to 3) |
| Backdrop of the top screen | (`Screen`, depth, 0) on a child |
| Title screen: menu, then logo | (`Screen`, depth, 1) and (`Screen`, depth, 2) on a child |
| Create world screen with its message | (`Screen`, depth, 1) and (`Screen`, depth, 2) on a child |
| Tooltip, drag preview, title fade | (`Overlay`, 0, 0 to 2) |

**The draw.** One draw system writes one kind of layer. It runs only on a change of its inputs.
Each gate is an event condition of `docs/ecs-lint.md`.

| Trigger | Condition |
|---|---|
| The kind component or the data of the kind | `any_match_filter::<Changed<Kind>>` |
| The window, the GUI scale, the pointer | `resource_changed::<GuiSize>`, `resource_changed::<GuiPointer>` |
| The language, the options | `resource_changed::<ClientLanguage>`, `resource_changed::<ClientOptions>` |
| The caret blink, a fade | `resource_changed::<GuiClock>` |
| The chat lines | A named condition `chat_layer_due` (determination 19) |
| The top screen | `any_match_filter::<Added<TopScreen>>` |

`GuiClock` holds the phases that a draw animates: the blink phase of a caret and the fade of the
title. One system writes it in `Begin` with `set_if_neq`. A caret blinks two times a second, so a
screen with an edit box redraws its layer at that rate and not on each frame. The chat layer does
not use `GuiClock`. Open question 1 states its gate.

**The tooltip.** Each screen kind writes its tooltip request into a `GuiTooltip` component on its
own entity. One overlay system draws the tooltip of the top screen. It runs on
`any_match_filter::<Changed<GuiTooltip>>`.

**The batch.** `build_gui_batch` runs on `any_match_filter::<Changed<GuiLayer>>`, on
`any_component_removed::<GuiLayer>` or on `resource_changed::<ScreenStack>`. It joins the layers
in key order into `GuiBatch`. It resolves the glyph slots in `GlyphCache`. A missed glyph is
rasterised through `Font::unifont_cell` into the list `GlyphUploads`.

**The crossing.** An extract system with `ResMut<MainWorld>` moves the batch into the render
world when the batch changed. It moves `GlyphUploads` when the list is not empty. Nothing is
cloned. The render world keeps the last batch until a new one arrives. The vertex buffer is
written only when a new batch arrived. The glint offset changes on each frame, so it leaves the
batch. It is a small `Copy` value, `GuiGlint`, that crosses with `ExtractResource`. The main world
writes it with `set_if_neq` only while the batch has a glint draw.

**The phases.** `GuiPhase` has three sets in a chain.

| Set | What runs in it |
|---|---|
| `Begin` | Write `GuiSize`, `GuiPointer` and `GuiClock` with `set_if_neq`. |
| `Draw` | The draw systems. Each one writes only its own layers, so they run in parallel. |
| `Batch` | `build_gui_batch`. |

No screen orders itself against another screen. A dialog over a paused screen needs no new set.

**The cost model.** The work of a frame is proportional to the layers that changed. A frame with
no change runs no draw system and no batch build, and it writes no buffer. A pointer move over a
menu redraws the menu layer and rebuilds the batch. In game, the layer of the chat lines redraws on a new line, on an
opened or closed chat, and while a line fades (determination 19).

**Classification of each value** (skill `bevy-ecs`).

| Value | Class | Owner |
|---|---|---|
| Window size, `GuiConfig`, `ClientOptions`, `ClientLanguage` | Authoritative | Bevy, the options, the language load |
| `GuiSize`, `GuiPointer`, `GuiClock` | Materialized derived state, for many readers and for change detection | One system in `Begin` |
| `GuiLayer` | Materialized projection of the kind data and the inputs above | The draw system of its kind |
| `GuiTooltip` | Materialized projection of the hovered widget or stack | The input system of the screen kind |
| `GuiBatch` | Materialized projection of all layers in key order | `build_gui_batch`, then the render world after the move |
| `GlyphCache` | Authoritative: the slot of each glyph | `build_gui_batch` |
| `GlyphUploads` | Temporal fact: the cells that the GPU does not have yet | `build_gui_batch`, then the extract |
| `GuiGlint` | Derived from the time | One system in `Batch` |

### The atlas and the render world

`GuiAtlasImage` holds the pixels and is an `ExtractResource`, so each change copies a few
megabytes. `FontCells` copies the unifont sources into the render world.

The proposal:

- The atlas pixels and the glint pixels become two Bevy `Image` assets with
  `RenderAssetUsages::RENDER_WORLD`. Bevy uploads each image and drops the main copy. A pack
  reload replaces the asset. The GUI pipeline binds the GPU image of each handle. No custom
  extract and no assumption "the main world reads no pixel" is necessary.
- `GuiAtlasLayout` keeps the regions, the sprites, the textures and the origins, and gains the
  two handles. `gui_atlas_image.rs` ends.
- The glint tuple `(u32, u32, Vec<u8>)` ends. The loader output `GuiAtlasParts` names the glint
  as an `Image`.
- `FontCells` ends. The main world owns `Font` and `GlyphCache`. The render world holds no font
  data. It writes each cell of `GlyphUploads` into the glyph region of the atlas texture.
- A new atlas image has no glyphs. When the handle changes, `GlyphCache` clears, and the next
  batch uploads the cells again.
- `Font::unifont_cell` is the one path to a cell. `glyph_page.rs` in `mcrs_minecraft_client`
  uses it too.

### GuiSize and GuiPointer

`GuiSize` (width, height and scale) and `GuiPointer` (the pointer in GUI units, or none) become
resources. One system in `GuiPhase::Begin` writes each with `set_if_neq`, from the window and
`GuiConfig`. Today `GuiScene` holds both, and `MenuExtent` copies the size. `MenuExtent` ends. The
`Pointer` parameter of `menu_input` and the function `creative_pointer` read the resources.

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
  close packets do this, so the insert and the removal happen on an event.

**The shared parts** belong to `client_screen`.

| Item | What it is |
|---|---|
| `ScreenEntry` | Every kind component requires it. It holds `replace`: `None`, `Top` or `All`. Its add and remove observers keep the stack. |
| `ScreenStack` | A resource, `ScreenStack(Vec<Entity>)`. The last entity is the top. |
| `TopScreen` | A marker on the entity at the top. The observers of `ScreenEntry` move it. |
| `EscapeRefused` | A marker. A kind requires it when Escape must not close the screen. |
| `AllowsChat` | A marker. A kind requires it when the chat key may open the chat above it. |
| `Backdrop` | An enum component. `ScreenEntry` requires it with `Default`. A kind overrides it (determination 20). |
| `WorldShown` | A resource, `WorldShown(bool)`. The client app writes it. |
| `close_top_screen` | The one Escape system. |

**Open, close and replace are observers.** To open a screen, a system spawns an entity with the
kind component. The add observer of `ScreenEntry` reads `replace`. For `Top` it despawns the old
top, and for `All` it despawns every entity of the stack. Then it pushes the new entity and moves
`TopScreen` to it. To close a screen, a system despawns the entity or removes its kind. The remove
observer of `ScreenEntry` takes the entity out of the stack, wherever it stands. If the entity was
the top, the observer moves `TopScreen` to the new last entity. A despawn from any code leaves no
stale entity in the stack. A disconnect spawns its screen with `All`. A dialog that replaces a
dialog spawns with `Top`.

**Why a resource and not a relationship.** The stack needs a defined order, a top, a replace of
all entries and a removal from the middle. With a relationship `Covers` and `CoveredBy`, the top
is `Without<CoveredBy>` and needs no walk. But a removal from the middle must link the screen
above to the screen below, and "replace all" needs a walk. The batch also needs the depth of a
screen. A resource gives the order and the depth in one place, and the observers are its one
writer. `ScreenStack` is the simpler choice.

**Why `TopScreen` is a marker.** Every draw and input system selects the top screen with a
`Single<.., With<TopScreen>>`. Only the observers move it, on an open or a close, and
`marker_toggle` checks tick systems only.

**The menu is a component.** `Menu` becomes a component of the screen entity. Today it is a
resource that `project_menu` rebuilds for the top screen. A screen below the top keeps its own
`Menu` with its focus and its scroll. So the table `kept_focus` ends.

**One layout system per kind.** The layout of a kind is a pure function from the kind data, the
GUI size, the options and the language to the widgets. One system per kind calls it. The system
runs on `any_match_filter::<Added<Kind>>` or on `resource_changed` of `GuiSize`, `ClientOptions`,
`ClientLanguage` and the resources of that kind (`NewWorldForm`, `WorldTypes`, `WorldList`; for
the title, `SingleplayerEntry` through `resource_exists_and_changed`). On
`Added` it builds the `Menu`. On a resource change it writes the new rects and captions into the
existing widgets and keeps the focus.

**Escape.** `close_top_screen` runs on `input_just_pressed(KeyCode::Escape)`. It takes
`Single<Entity, (With<TopScreen>, Without<EscapeRefused>)>` and despawns that entity. A screen
with `EscapeRefused` reads Escape itself. The title, the death screen and level loading refuse.
A container screen refuses and sends the close request through its own path. The pause opener of
`gui_menus` runs on the same input when the stack is empty and a player exists. It reads the
stack before the despawn applies, so one key press never closes a screen and opens the pause.

**The background.** The kind of the screen selects its backdrop (determination 20). The opener
chooses nothing, and a sub screen needs no copy. One system, `project_backdrop`, belongs to
`gui_render`. It runs on a change of `TopScreen`, of the `Backdrop` of the top screen or of
`WorldShown`. It does two things.

- It writes `PanoramaView` and `BackdropBlur` for the render world. `draw` is `Hidden`, `Plain` or
  `Blurred`. `BackdropBlur` is the radius of the blur, a `Copy` resource of `mcrs_minecraft_render`.
  The radius is the option "Menu Background Blur" (0 to 10, default 5, 0 is off) when the backdrop
  blurs, and 0 otherwise.
- It writes the texture quad or the gradient quad into the backdrop layer of the top screen. The
  layer is a child of the screen entity with the key (`Screen`, depth, 0). The system spawns it
  when the screen becomes the top. A draw system of a kind uses the parts 1 and up.

| `Backdrop` | `PanoramaView.draw` | `BackdropBlur` | Quad of the backdrop layer |
|---|---|---|---|
| `Default`, no world | `Blurred` | The option | The menu background texture over the window |
| `Default`, in a world | `Hidden` | The option | The in-world texture over the window |
| `Panorama` | `Plain` | 0 | None |
| `PanoramaOverWorld` | `Blurred` | The option | The texture of the world state of `WorldShown` |
| `InGameGradient` | `Hidden` | 0 | The gradient `C0101010` to `D0101010` |
| `Gradient { top, bottom }` | `Hidden` | 0 | The gradient of the variant |
| `None` | `Hidden` | 0 | None |

With no world, the panorama pass blurs the panorama with the radius of `BackdropBlur`. In a world,
the blur node of `mcrs_minecraft_render` runs after the HUD band and before the screen layers. It
blurs the image of the world and the HUD with the same six passes (WID-SCR-005). The backdrop
quads draw over the result, so they stay sharp.

The spin of the panorama is a separate system in `gui_render`. It runs each frame while
`PanoramaView.draw` is not `Hidden`, because the angle changes on each frame. It writes
`spin_degrees` with `set_if_neq`. The backdrop system does not write the angle.

`Menu` draws its header and footer separators with the textures of the world state. It reads
`WorldShown` for them and redraws on `resource_changed::<WorldShown>`. It draws no backdrop.

**The three behaviours that the split fixes.** The specification corrects three screens. The
plan carries each one in a step, with a check.

| Behaviour | Rule | Owner | Step | Check |
|---|---|---|---|---|
| In a world, each `Default` screen blurs the world and the HUD before its texture. The pause screen and the options screens are such screens. | WID-SCR-004, WID-SCR-005, WID-SCR-008 | The blur node and `BackdropBlur` in `mcrs_minecraft_render`; the split of the draw and `project_backdrop` in `gui_render` | 2d | A pixel test of the blur with the radius 5 against the postconditions of WID-SCR-005, and a test that the radius 0 runs no pass. A screenshot of the pause screen in a world. |
| The death screen and the confirmation of its title button draw `Gradient { top: 60500000, bottom: A0803030 }`, with no texture and no blur. | WID-SCR-008, PLD-SCR-004 | `gui_session` | 2b | A test that opens the death screen and reads the backdrop layer: one gradient quad of these colours, no texture quad, `BackdropBlur` 0. |
| The pause with no menu exists. F3 and Escape open it, and it has `Backdrop::None`. | WID-SCR-008, MENU-PAUSE-001 | `gui_menus` | 2b | A test that sends the chord with a player and no screen: the stack holds the kind with no `Menu`, the backdrop layer is empty and `BackdropBlur` is 0. |

The blur needs render work, not only a new setting. Today `mcrs_minecraft_render` blurs only the
panorama, in a pass that runs before the transparent pass of the world. The HUD draws later, in the
GUI pass. So two changes are necessary. First, the blur node of `mcrs_minecraft_render` takes the
radius from `BackdropBlur` and runs a second time on the target after the HUD band. Second,
`gui_render` splits the draw of the batch into the HUD band and the rest, and the blur node runs
between them. The option "Menu Background Blur" (`options.accessibility.menu_background_blurriness`)
is a new field of `client_options`, with a slider in the accessibility screen. Step 2d adds it.

The pause with no menu is a second kind, `PauseOverlay`, in `gui_menus`. It requires
`Backdrop = None`, has no `Menu` and has no widget. The pause opener spawns it on F3 and Escape,
and the plain Escape still opens the pause screen with its menu. The specification of
MENU-PAUSE-001 names this case and does not state it yet. Its author states the rule for the close
key and for the pause of the game before step 2b starts.

**The initial screen.** `GuiPlugin` loses its field `screen`. The client app spawns the first
screen kind that its configuration names.

**Classification of each value** (skill `bevy-ecs`).

| Value | Class | Owner |
|---|---|---|
| The kind component and its data | Authoritative | The crate of the screen |
| `Menu` | Authoritative widget state (focus, scroll, values) with derived rects | The layout system and the menu input of its kind |
| `EscapeRefused`, `AllowsChat`, `replace` | Authoritative, fixed at spawn | The kind that requires them, or the spawner |
| The order of `ScreenStack` | Authoritative. No other value gives the order. | The observers of `ScreenEntry` |
| `TopScreen` | Materialized from `ScreenStack`, for query selectivity | The observers of `ScreenEntry` |
| The previous screen, the depth | Derived from `ScreenStack` | Nobody. Each reader computes it. |
| `MenuEvent` | Temporal fact | The menu input |
| `Backdrop` | Authoritative, fixed by the kind at spawn | The kind that requires it |
| `WorldShown` | Derived from the world state of the client | The client app, the one writer |
| `PanoramaView.draw`, the backdrop layer | Materialized from `Backdrop` and `WorldShown`, for the render world and the batch | `project_backdrop` |
| `BackdropBlur` | Materialized from `Backdrop` and the option, for the render world | `project_backdrop` |
| `PanoramaView.spin_degrees` | Temporal fact (the angle) | The spin system of `gui_render` |

### No central dispatcher

Three modules know every screen today. `next_screen` holds the table "back from X goes to Y".
`follow_menu_events` matches (screen, widget) for every screen through `TitleActions`, a
parameter of nine fields. `project_menu` builds the `Menu` of every screen. All three end in
steps 2b and 2c.

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
- `TitleScreenState` becomes data of `TitleScreen`.

### The container gesture kernel

The gestures of a container are a state machine: press, drag, release, quick move and the double
click. Today `Hover::Creative(CreativeHit)` and the argument `CreativeGrid` of `Viewer::input`
make the gesture code depend on the creative code.

The proposal inverts that edge.

- The kernel takes a resolved hit: what was clicked, the menu entry, whether the hit takes a left
  press, whether it permits a drag, and the stack presence of the target. It does not know
  creative.
- `gui_creative` maps `CreativeHit` to that value. `creative_clicks` holds the map.
- The kernel takes slot indices and stack presence, not `Entity`. It reads stacks through a small
  trait with one empty host, as `BodyView` and `NoBodies` in `mcrs_minecraft_physics` do.
- `MenuGesture` and `OpenDrag` get methods for what `gesture_frame` reads today. That ends the
  leaks of `release`, `remembered` and `kind`.
- `item_key_codes` and `wheel_notches` map Bevy input types. They stay on the Bevy side, in
  `gui_container`.

The kernel is `mcrs_minecraft_container_gesture`, in layer 3 beside `container_click`. It is
`#![no_std]` with `alloc`. Its dependencies `container_click` and `inventory_click` are `no_std`
today, with `inventory_player` and `inventory_transaction`. It must not take `item_stack`, which
needs `std`. It reads stack presence, as the list above says.

### What each crate holds

The lists use the file names of today. A name with an arrow marks a new module. "Merge" means
that two modules become one. Each crate keeps at most 15 Rust files, `lib.rs` and test modules
included.

**`mcrs_minecraft_font`** (11 files). Modules: `font`, `font_definition`, `font_glyph`,
`font_unihex`, `glyph_cache`, `language`, `text`, `span_events`, `event_index`,
`font_dump_tests`.

- Public names: `Font`, `FontOptions`, `Span`, `Glyph`, `GlyphImage`, `ImageSource`,
  `BitmapPage`, `GlyphCache`, `ClientLanguage`, `Wrap`, `text_spans`, `wrap`, `argb`,
  `shadow_color`, `SpanEvents`.
- Not `#![no_std]`: `text`, `lang` and `nbt` have no `no_std` build, and `text` also needs
  `wire`, `core` and `profile`. No cheap path exists.
- The code says: `Text` comes from `protocol_game::text`, a re-export of `mcrs_minecraft_text`.
  The font takes it from `text` and drops the edge to `protocol_game`. `ResourceSet` and
  `decode_png` come from `mcrs_minecraft_model`, which needs Bevy and `client_jar`, and
  `full_id` and `split_id` come from `mcrs_minecraft_model_file`. Only the file lookup is used.
  That lookup is `PackFiles` of `model_file`, which has no Bevy; `ResourceSet::files` returns
  it. Step 4 moves `PackFiles` and a PNG decoder (the `png` crate, not the Bevy image) down into
  a new Bevy-free crate `mcrs_minecraft_resource_pack`. `model_file`, the model, the atlas, the
  font and the language take them from there. The font takes `&PackFiles`. `full_id` and
  `split_id` end; `ResourceLocation::read` of `mcrs_minecraft_resource` replaces them. The new
  crate is not part of `mcrs_minecraft_resource`, because a user of locations alone must not pay
  for the PNG decoder.
- The code says: `font.rs` still re-exports `draw_text` and its siblings from `text_draw`. The
  re-export ends in step 4. `font_definition` uses `bevy::log::warn`, which becomes an error
  value. `font_unihex` names `client_jar::Directory`. The loader instead takes the bytes from its
  caller.
- Dependencies: `mcrs_minecraft_model_file`, `mcrs_minecraft_lang`, `mcrs_minecraft_text`,
  `mcrs_minecraft_text_flatten`, `bevy_math`. A `bevy` feature adds the `Resource` derive to `Font`, `GlyphCache` and
  `ClientLanguage`.

**`mcrs_minecraft_gui_scene`** (15 files). Modules: `gui_size`, `gui_config`, `gui_quad`,
`gui_layer` (from `gui_scene`, with `SceneIcon`), `gui_phase`, `gui_pointer`, `gui_clock` (→),
`gui_atlas_layout`, `gui_atlas_load`, `menu_background`, `sprite`, `gui_sprite_id`,
`gui_texture_id`, `gui_scene_plugin`.

- Public names: `GuiSize`, `GuiConfig`, `GuiPointer`, `GuiClock`, `GuiQuad`, `GuiLayer`,
  `GuiBand`, `SceneIcon`, `GuiPhase`, `GuiAtlasLayout`, `GuiAtlasParts`, `load_gui_atlas`,
  `GuiSprite`, `GuiSpriteId`, `GuiTextureId`, `GuiScenePlugin`.
- The code says: `gui_atlas_load` stitches the container backgrounds that `menu_background`
  lists, so the scene depends on `menu_geometry` and `menu_key`.
- The code says: the scene has no edge to `client_world`. `SceneIcon` needs only `item_icon`.
  `GuiPlugin` adds `spawn_inventory_figure`, `pose_inventory_figure`, the panorama projection,
  `CreativeScreenPlugin` and `TitleScreenState` today. Step 1b moves each to its crate.
- The crate is at the limit. A new module in it needs a merge or a new crate.
- Dependencies: `font`, `mcrs_minecraft_model`, `mcrs_minecraft_item_icon`,
  `mcrs_minecraft_menu_geometry`, `mcrs_minecraft_menu_key`, `client_widget` (for `GuiPoint`),
  Bevy (image, window, asset).

**`mcrs_minecraft_gui_primitives`** (10 files). Modules: `text_draw`, `tooltip`,
`item_decorations`, `item_icons`, `slot_icon` (the component only), `texture_tile`,
`outlined_box`, `irect`, and `menu_test_app` under a `test-support` feature.

- Public names: `draw_text`, `draw_str`, `draw_centered_text`, `Tooltip`, `LineGap`,
  `Decorated`, `CountLabel`, `decorations`, `ItemIcons`, `SlotIcon`, `GuiTooltip`.
- The code says: `slot_icon` orders its projection after `ClientNetworkSystems`. The projection
  system and `SlotIconPlugin` move to `gui_container`, so primitives take no network edge.
- The code says: `menu_item_tooltip` reads the player, the holders and the open menu. It is not
  a primitive. It moves to `gui_container`.
- Dependencies: `scene`, `font`, `client_widget`, `item_icon`, `item_stack`, `item_table`,
  `mcrs_minecraft_registries`, `mcrs_minecraft_model`.

**`mcrs_minecraft_gui_render`** (8 files and the shader). Modules: `atlas_quad`, `gui_batch`,
`gui_batch_build`, `gui_items`, `gui_glint` (→), `glyph_uploads` (→), `gui_batch_extract` (→).

- Public names: `GuiRenderPlugin`, `GuiBatch`, `GuiDraw`, `GuiDrawKind`, `GLINT_ALPHA`.
- Determination 20 adds the modules `project_backdrop` (→) and `panorama_spin`. The first holds
  the one backdrop system. The second holds `spin` from today's `panorama_view`. Both leave
  `gui_menus`. Step 2d splits the draw of the batch into the HUD band and the rest, so that the
  blur node of `mcrs_minecraft_render` runs between them. The crate then has 10 files.
- Dependencies: `primitives`, `scene`, `font`, `client_screen`, `client_options` (the blur radius),
  `mcrs_minecraft_render`, the Bevy render crates, wgpu.

**`mcrs_minecraft_gui_menu`** (15 files). Modules: `menu`, `menu_draw`, `menu_input`,
`menu_plugin`, `menu_cursor`, `menu_event`, `menu_input_systems`, `scroll_area_draw`,
`tab_bar_draw`, `edit_box_draw`, `leaf_draw`, `cursor_grab`, `edit_input`, `search_input`.

- Public names: `Menu`, `MenuEvent`, `MenuPlugin`, `MenuInputSystems`,
  `draw_menu`, `screen_key`, `pastes`, `clipboard_text`, `copy_text`, `grab`, `SearchInput`.
- The code says: `edit_input` maps keys to `Key` and `TabDirection` of `chat_client`. So the menu
  host depends on `chat_client`, a crate without Bevy.
- The crate is at the limit.
- Dependencies: `primitives`, `scene`, `font`, `client_screen`, `client_widget`, `client_options`,
  `chat_client`.

**`mcrs_minecraft_client_screen`** (about 8 files, new). Modules: `screen_entry`, `screen_stack`,
`top_screen`, `escape_refused`, `allows_chat`, `close_top_screen`, `client_screen_plugin`.
Dependencies: `bevy_ecs`, `bevy_app`, `bevy_input`. It is `#![no_std]`: `bevy_ecs` and
`bevy_app` already serve `no_std` crates such as `engine_inventory_click`. No crate of the
workspace takes `bevy_input` yet. Step 2a checks that it builds without `std`.

**`mcrs_minecraft_container_gesture`** (10 files, new). Modules: `gesture_frame`, `hover`
(becomes the resolved hit), `viewer`, `modifiers`, `last_press`, `menu_gesture`, `open_drag`,
`press_kind`, `release_rule`. `#![no_std]` with `alloc`. Dependencies: `container_click`,
`inventory_click`. Today `menu_gesture` names `ItemStack`; step 1c replaces it with
`StackPresence`.

**`mcrs_minecraft_gui_chat`** (5 files). Modules: `chat`, `chat_input`, `chat_suggestions`,
`chat_view`.

- The chat key system runs on `input_just_pressed` of the chat key or the slash key. It spawns a
  `ChatScreen` when the stack is empty or the top has `AllowsChat`.
- Public names: `ChatDrawPlugin`, `ChatScreenPlugin`, `ChatLines`.
- Dependencies: `menu`, `primitives`, `client_screen`, `client_world`, `chat_client` with
  `bevy`, `mcrs_minecraft_command`, `mcrs_minecraft_chat`.

**`mcrs_minecraft_gui_container`** (14 files). Modules: `menu_screen`, `menu_stacks`,
`menu_label_text`, `inventory_figure`, `hotbar`, `drag_preview`, `inventory_clicks`,
`container_screen` (→, merges `close_menu_screen` and `follow_open_menu`), `container_draw`,
`menu_item_tooltip`, `slot_icon_plugin` (→), `item_key_codes`, `wheel_notches`.

- `ContainerScreen` requires `AllowsChat` and `EscapeRefused`.
- Public names: `ContainerScreen`, `InventoryClicksPlugin`, `HotbarPlugin`, `SlotIconPlugin`,
  `InventoryFigure`, `MenuStacks`, `DragPreview`, `item_key_codes`, `wheel_notches`.
- Dependencies: `container_gesture`, `menu`, `primitives`, `client_screen`, `client_world`,
  `container_click`, `inventory_menu`, `engine_inventory_click`, `engine_inventory_menu`,
  `entity_render`, `pose`, `player_holders`, `item_tooltip`.

**`mcrs_minecraft_gui_creative`** (15 files). Modules: `creative_clicks`, `creative_grid`,
`creative_item_tooltip`, `creative_screen_draw`, `creative_screen_input` (merges
`creative_screen_edit` and `creative_pointer`), `creative_screen_plugin`, `creative_screen_state`,
`creative_search_edit`, `creative_search_index`, `creative_tab_content`, `creative_tab_view`, and
the three test modules.

- The code says: `mark_creative_screen`, `creative_screen_shown` and `creative_screen_open` end
  with the screen entity. `loaded_creative_report` leaves: `mcrs_minecraft_creative_report` reads
  the embedded report, and its `bevy` feature derives the resource.
- The crate is at the limit.
- Public names: `CreativeScreenPlugin`, `CreativeContentSystems`.
- Dependencies: `container`, `container_gesture`, `menu`, `primitives`, `client_screen`,
  `creative_content`, `creative_report`.

**`mcrs_minecraft_gui_hud`** (3 files). Modules: `health_hud`, `game_mode_switcher`.

- `HudTicks` and `count_hud_ticks` end. The heart shake reads the tick from `Time<Fixed>`.
- Public names: `HealthHudPlugin`, `GameModeSwitcherPlugin`.
- Dependencies: `primitives`, `client_screen`, `client_world`, `game_mode`, `damage`,
  `entity_attribute`, `network`.

**`mcrs_minecraft_gui_debug`** (9 files). Modules: `debug_screen`, `debug_displayer`,
`debug_entry_fps`, `debug_entry_network`, `debug_entry_position`, `debug_entry_system_specs`,
`chunk_map`, `light_levels`.

- The crate moves today's modules as they are (determination 15). The inner design of the F3
  screen is outside this RFD.
- The refresh condition of the F3 screen becomes a named function with
  `mcrs::event_condition`. It is true only on a frame with a due refresh.
- Public names: `DebugScreenPlugin`, `DebugScreenDisplayer`, `DebugScreenSet`, `ChunkMapPlugin`,
  `LightLevelsPlugin`.
- Dependencies: `primitives`, `client_world`, `engine_world`, `network`, `render_probe`,
  `volume`. It has no edge to `client_screen`.

**`mcrs_minecraft_gui_session`** (8 files). Modules: `death_screen`, `death_screen_plugin`,
`disconnected_screen`, `level_loading_screen`, `message_screen`, `session_screens`,
`session_plugin` (→).

**`mcrs_minecraft_gui_worlds`** (10 files). Modules: `select_world_screen`,
`create_world_screen`, `delete_world_screen`, `world_entry`, `world_rows`,
`world_list_error_screen`, `follow_create_world`, `follow_list_answer`, `worlds_plugin` (→). The
plugin inserts `SingleplayerEntry` (determination 16). It depends on `session` for the message
and the level loading screens, on `menus` for `SingleplayerEntry`, and on `save`.

**`mcrs_minecraft_gui_menus`** (15 files). Modules: `title_screen`, `title_draw`,
`pause_screen`, `options_screen`, `option_sub_screen`, `option_caption`,
`accessibility_settings_screen`, `controls_screen`, `mouse_settings_screen`,
`video_settings_screen`, `menu_options_systems`, `menus_plugin` (→), and `singleplayer_entry` (→),
which holds the resource of determination 16. It depends on neither `worlds` nor `session`. Each
kind of the crate requires its own `Backdrop` (determination 20). The title requires `Panorama`.
The pause screen and the options screens keep `Default`. `PauseOverlay` requires `Backdrop = None`.
`panorama_view` moves to `gui_render`.

**Not in a GUI crate.** `sign_text` (763 lines) reads `SignFamily` and `SignMount` from the
client world and draws no GUI. It moves into `mcrs_minecraft_client` (determination 8), beside
its two callers `sign_text.rs` and `name_tag.rs`.

### Field leaks

The 32 leaks fall into these groups. Each group has one owner and one fix.

| Group | Leaks | Owner | Fix | Step |
|---|---|---|---|---|
| Chat view | `ChatScreen::commands`, `ChatScreen::edit`, `CommandInput::list`, `CommandInput::entries`, `CommandInput::highlights`, `SuggestionMenu::entries` (6) | `mcrs_minecraft_chat_client` | A view API in that crate: the input runs, the highlights and the rows of the suggestion list | 2a |
| Creative search | `CreativeSession::items`, `::language`, `::registries`, `CreativeScreenState::search`, `CreativeTabContent::search`, `CreativeContent::search_list` (6) | `mcrs_minecraft_creative_content` | One type for a search: the query and its results. The screen state holds it and asks it. | 9 |
| Creative tab | `CreativeTab::kind` (1) | `mcrs_minecraft_menu_geometry` | An accessor for the tab kind | 9 |
| Hubs | `TitleActions` (8), `ClickState::last_screen`, `ClickState::last_menu`, `Widgets::screen` (11) | `gui_menus`, `gui_worlds`, `gui_container` | The hubs dissolve. `Widgets::set_screen` has no caller after `project_menu` ends. | 2a, 2c |
| Gestures | `MenuGesture::release`, `MenuGesture::remembered`, `OpenDrag::kind` (3) | `container_gesture` | Methods on the kernel types | 1c |
| Small | `Pointer::buttons`, `Table::stored`, `MenuLabelText::title`, `MenuLabelText::inventory`, `TitleScreenState::logo` (5) | `gui_scene`, `font`, `gui_container`, `gui_menus` | `GuiPointer`; a length method on `Table`; a label method; data of `TitleScreen` | 1b, 1c, 2b |

No step moves a leak to a new crate. Each leak ends before the move of its owner.

### Dialog readiness

`docs/spec/dialog.md` section 18 lists the widgets. Sections 11 and 13 give the flow. The split
must leave room for a dialog crate. This page does not design that crate.

The limits that the split puts on it:

- It depends on `font`, `scene`, `primitives`, `menu`, `client_screen` and `client_widget`. It
  does not depend on `client_world`, on a player or on a container crate. A dialog can show in
  the configuration phase, where no world exists (DLG-FLOW-005). The scene has no edge to the
  client world, so this holds.
- It draws through the atlas and the font only. That is why `WorldPresence` ends and the kind
  requires `Backdrop` (determination 20). The dialog requires nothing, and `Default` applies,
  as WID-SCR-008 states for a server dialog. `WorldShown` is `false` in the configuration phase.
- A dialog kind requires `EscapeRefused`, because Escape runs the cancel action (DLG-KEY-002).
- A dialog replaces a dialog with `replace: Top` (DLG-FLOW-002). A `close` after action despawns
  the top, and the previous screen becomes the top. A disconnect uses `replace: All`
  (DLG-FLOW-004). The waiting screen and the warning screen are entities of the same stack.
- A dialog over another screen is one more depth in the layer key. It needs no new set.
- The pause flag (DLG-KEY-003) has no reader today. The dialog crate adds `PausesGame` to
  `client_screen` together with its first reader. The game is paused when the top screen has
  it. Nothing stores the pause a second time.
- `client_widget` has 69 files, which `deslop-please` also reports. The dialog depends on it.
  The split of that crate is a separate decision.

## Migration

The migration is one pull request. Each step is one commit, and each commit builds and passes
the tests. The step that moves the last caller of a path deletes
the path. The old crate loses modules as they leave. A step that changes a client crate runs the
skill `run-client`.

Line counts come from `wc -l` of the modules that change or move. They are estimates of the
added lines.

| Step | Crate group | Content | Lines |
|---|---|---|---|
| 1 | `client_gui` | Done: break the cycles inside the crate | about 3,200 added, 2,700 deleted |
| 1b | `client_gui`, `client` | `GuiSize`, `GuiPointer`, retained layers, the atlas asset, the glyph upload list | about 2,300 |
| 1c | `client_gui` | The gesture kernel without Bevy, without creative and without `item_stack` | about 700 |
| 2a | `client_screen`, `client_world`, `chat_client`, `inventory_client`, `client_gui`, `client` | The stack (`no_std`), chat, container, creative and the game mode switcher as entities | about 1,900 |
| 2b | `client_gui`, `client` | `Menu` as a component, the session screens, title, pause and options as entities | about 2,200 |
| 2c | `client_gui`, `client_world`, `client` | The world screens as entities; `SingleplayerEntry`; delete `Screen`, `next_screen`, `follow_menu_events`, `project_menu` | about 1,700 |
| 2d | `mcrs_minecraft_render`, `client_options`, `client_gui` | The option "Menu Background Blur" and its slider; `BackdropBlur`; the blur node after the HUD band; the split of the batch draw; the radius in `project_backdrop` | about 900 |
| 3 | `client` | Move `sign_text` | about 780 |
| 4 | `font`, `client` | Move the font; `client` reads the jar | about 2,000 |
| 5 | `container_gesture` | Move the kernel as a `no_std` crate | about 600 |
| 6 | `gui_scene`, `gui_primitives` | Move the scene and the primitives | about 2,900 |
| 7 | `gui_render` | Move the batch, the pipeline and the extract | about 1,300 |
| 8 | `gui_menu` | Move the menu host | about 2,200 |
| 9 | `creative_content`, `creative_report`, `menu_geometry`, `client_gui` | The search type, the tab accessor, the embedded report | about 400 |
| 10 | `gui_chat` | Move chat | about 1,350 |
| 11 | `gui_container` | Move the container screens | about 2,700 |
| 12 | `gui_creative` | Move the creative screen | about 2,900 |
| 13 | `gui_hud`, `gui_debug` | The HUD, then the F3 screen | about 900, 1,400 |
| 14 | `gui_session`, `gui_menus`, `gui_worlds` | In this order | about 1,000, 1,600, 1,800 |
| 15 | `client`, workspace | Delete `client_gui`; update `docs/crates.md` | small |

Step 3 is independent. It can come at any point before step 15.

### Step 1: done

Step 1 changed the crate only. It is in the working tree and not merged yet.

- `scene.rs` (1,208 lines) split into one file per item: `gui_size`, `gui_config`, `gui_quad`,
  `gui_scene`, `gui_atlas_layout`, `gui_atlas_image`, `gui_atlas_load`, `gui_batch`,
  `gui_batch_build`, `gui_frame`, `gui_phase`, `gui_pointer`, `gui_scene_plugin`.
- `GuiPhase` was added with five sets. `GuiPlugin` no longer names a screen draw.
  `draw_screens` became `container_draw`.
- `Font` became a resource. The atlas has no font field. `FontCells` carries the unifont cells to
  the render world.
- The `draw_*` functions left `font.rs` for `text_draw`. `font_bitmap` merged into
  `font_definition`. `debug_screen_overlay` merged into `debug_screen`. `hud_systems` ended.
- `edit_input` left `chat_input`. The `Player` read left `menu_draw` for `world_presence`.
- New files that the first draft did not list: `chat_view`, `next_screen`, `irect`, `leaf_draw`,
  `outlined_box`, `menu_background`, `creative_pointer`, `event_index`.
- Result: module cycles 5 to 0, field leaks 34 to 32. The first draft predicted 2 cycles.

### Step 1b: the retained scene

1. Add `GuiSize`, `GuiPointer` and `GuiClock` as resources. Delete `MenuExtent` and
   `begin_gui_frame`.
2. Add `GuiLayer` and `GuiBand`. Each HUD plugin spawns its layer entities at startup. Each
   screen draw writes a layer entity of its plugin. Until step 2a, the batch takes every layer,
   and a screen layer is empty while its screen is not shown.
3. Gate each draw system by the triggers of section "The retained scene". Shrink `GuiPhase` to
   `Begin`, `Draw` and `Batch`.
4. Gate `build_gui_batch`. Move the batch and `GlyphUploads` by `ResMut<MainWorld>`. Add
   `GuiGlint`. Gate `upload_glyphs` and `write_gui_buffers` on the arrival of new data. Gate
   `prepare_gui_pass` on a change of the window view or of the atlas asset.
5. Make the atlas and the glint `Image` assets. Delete `FontCells` and `gui_atlas_image`.
6. Move the leftovers of step 1 out of `GuiPlugin`: the creative plugin, `TitleScreenState`, the
   panorama projection and the inventory figure.
7. Delete `HudTicks`. Mark the F3 refresh condition as an event condition.
8. Fix the leaks `Pointer::buttons` and `Table::stored`.

The step changes no behaviour. The existing client tests and a screenshot of the title, a
container, the creative screen and the F3 screen check it.

### Step 1c: the gesture kernel

The step inverts the creative edge, removes Bevy from the gesture modules and adds the methods
that end three leaks. It replaces `ItemStack` in the kernel with stack presence, so the kernel
needs no `std` crate. It also fixes the two leaks of `MenuLabelText`. The client test
`creative_screen_clicks` and the test `screen_clicks` check it.

### Steps 2a to 2c: the screen entities

The step 2 of the first draft estimated 1,200 mechanical lines. The code says that this is
wrong. The step must also dissolve the three central modules (1,653 lines with their tests),
because they match on `Screen`. The work is about 5,800 added lines, so it splits in three.

- **2a** adds `mcrs_minecraft_client_screen` and moves the screens of the game: chat, container,
  creative and the game mode switcher. `ChatScreen` gets its `bevy` feature, and `ChatInput`
  ends. `CreativeScreen` moves to the open-menu entity, and `mark_creative_screen` ends. The
  chat view API ends six leaks. `ClickState` loses two fields.
- **2b** makes `Menu` a component, adds the layout system of each kind, and moves the session
  screens, the title, the pause screen and the options screens. `WorldPresence` and
  `MenuBackground` end. `Backdrop` and `WorldShown` join `client_screen`. `project_backdrop` is a module of `client_gui`
  until step 7 moves it to `gui_render`. The table of
  `panorama_view` ends. The client app writes `WorldShown`. The death screen and its confirmation
  require `Backdrop::Gradient`. `PauseOverlay` arrives with the chord F3 and Escape. The central
  modules lose the arms of these screens.
- **2c** moves the world screens, deletes the central modules, and deletes `Screen` and its three
  parent enums from `client_world`. The title opens the world list through `SingleplayerEntry`.
- **2d** adds the option "Menu Background Blur", `BackdropBlur`, the blur node after the HUD band and
  the split draw of the batch. Each `Default` screen in a world then blurs the world and the HUD.

Step 2d changes the render crate, the options crate and the GUI crate in one commit. The blur
needs a node, a radius and a split draw together. A split in two would leave a node with no
writer.

Between 2a and 2c, `Screen` loses variants as screens move. A system that asks "is a screen
open" reads both the enum and `ScreenStack` until 2c. That is one migration with one end. 2c
deletes the old path.

Each step reproduces today's screens, their parents and their Escape rules. It adds no feature,
except the three behaviours of section "The screen model": step 2b carries the death gradient and
the pause with no menu, and step 2d carries the blur of the world.

### Steps 3 to 15: the moves

Each step moves modules with no edit besides `use` paths and `Cargo.toml`. Each step shortens
the dependency list of the old crate and gives the new crate only what it uses. Step 4 also
changes the font loader to take bytes. Step 14 moves `menus` before `worlds`, because `worlds`
depends on `menus`. Step 7 adds `GuiRenderPlugin` to the client app only when
the app has a window.

### What deslop-please and the ECS lint report

The first row is the measured state. The other rows are predictions that each step
checks. A step must not add a finding or an expectation. "Entries" is the count of entries in
the old `src/`, which stays one `wide-directory` finding until the crate ends.

| After step | Cycles | Leaks | Entries | `alloc_in_tick` | `ungated_system` | `blocking_in_system` | `marker_toggle` | `missed_set_if_neq` |
|---|---|---|---|---|---|---|---|---|
| now (1) | 0 | 32 | 142 | 20 | 6 | 1 | 1 | 1 |
| 1b | 0 | 30 | 143 | 6 | 1 | 1 | 1 | 0 |
| 1c | 0 | 25 | 143 | 6 | 1 | 1 | 1 | 0 |
| 2a | 0 | 17 | 138 | 5 | 0 | 1 | 0 | 0 |
| 2b | 0 | 16 | 139 | 5 | 0 | 1 | 0 | 0 |
| 2c | 0 | 7 | 136 | 2 | 0 | 1 | 0 | 0 |
| 3 to 14 | 0 | 7, then 0 after step 9 | falls with each move | 2 | 0 | 1 | 0 | 0 |
| 15 | 0 | 0 | the crate no longer exists | 2 | 0 | 1 | 0 | 0 |

The reasons for each change:

- 1b: the six draw systems and the seven F3 systems get event conditions (−13 `alloc_in_tick`).
  `prepare_gui_pass` gets its condition (−1 `alloc_in_tick`). The batch build, the glyph upload,
  the buffer write, `count_hud_ticks` and `prepare_gui_pass` get gates or end
  (−5 `ungated_system`). `begin_gui_frame` ends (−1 `missed_set_if_neq`). The old `src/` gains
  one entry: four modules come and three go. It stays one `wide-directory` finding.
- 2a: the chat opens in a system on `input_just_pressed` (−1 `alloc_in_tick`). `click_in_menu`
  loses `ClickState` (−1 `ungated_system`, the last one). `mark_creative_screen` ends (−1 `marker_toggle`).
- 2c: `follow_menu_events`, `follow_create_world` and `world_rows` run on `on_message` or on a
  layout trigger (−3 `alloc_in_tick`).
- What stays: `chunk_map` formats while the F6 view shows. The creative search copies the
  selected text on a key press. `debug_entry_fps` reads the wall
  clock to measure frame times.

The seven expectations of `clippy::too_many_lines` fall to five in step 2c, because
`follow_menu_events` and `project_menu` end.

Each new crate starts with no finding. `crates/` stays wide; `.deslop-please.allow` allows it.

## Options

The maintainer considered these options.

- **Keep one crate and allow the findings.** Rejected (determination 1). An entry in
  `.deslop-please.allow` hides a directory of 142 files and stops the report on the next one.
- **Modules inside one crate.** Directories, one per screen. The layout rule rejects it: `src/`
  is flat, and a crate that needs `foo/mod.rs` is too big. The dependency list also stays at 69.
- **Keep the immediate-mode scene.** Rejected (determination 10). It clears the scene on each
  frame. So each draw runs on each frame, the batch is rebuilt and cloned on each frame, and the
  glyph list is copied and compared again in the render world. That is the cause of 10 of the 29
  lint expectations. A split that keeps it only spreads the expectations over more crates.
- **The draw order as a chain of sets.** Rejected (determination 10). The first draft claimed
  that two screens never draw in one frame. The code says otherwise: the title draws a menu and a
  logo, a world screen draws a menu and a message, and the HUD and the chat draw under a screen.
  Each case adds an `after`. A key is data and needs no schedule edit.
- **Move the atlas by `ResMut<MainWorld>`.** Rejected (determination 11). It rests on the
  assumption that the main world reads no pixel after the load, and it needs a custom extract
  and a rule for a pack reload. A Bevy `Image` with `RENDER_WORLD` usage gives the same single
  owner with no custom code.
- **Fifteen crates with provisional seams.** Rejected (determination 12). `gui_creative_state`
  and `gui_gesture` existed only to break a cycle, and three menu crates split by topic. A review
  after step 8 would have decided the map late. The inverted gesture kernel removes the cycle.
- **Eleven crates.** The review proposed about eleven. The code says that three merges exceed
  the limit of 15 entries: scene with primitives, container with creative, and the session
  screens with title, pause and options. This page splits those three and states the force.
- **Moves before the redesign.** Rejected (determination 13). Each move would carry the
  immediate-mode scene and the central modules into the new crates. Each later fix would then
  edit several new crates. A redesign inside one crate edits each file once.
- **The F3 screen stays in `gui_hud`.** Rejected (determination 15). It adds `render_probe` and
  `volume` to a crate that otherwise needs neither.
- **The title depends on `gui_worlds`.** Rejected (determination 16). A client that only joins
  servers would take `save`. A `SystemId` in the resource is a second choice: it needs a
  registration and a deferred run, and a function pointer needs neither. A trait object is
  not allowed.
- **A central table from screen to backdrop.** Rejected (determination 20). Today `panorama_view`
  maps 19 variants of `Screen` to three panorama modes. In `gui_menus` such a table must know the
  kinds of `session` and `worlds`. A `Backdrop` component on each kind needs no table.
- **Several marker components for the backdrop.** Rejected (determination 20). The variants
  exclude each other, and a set of markers allows a contradiction. One enum states one backdrop.
- **`WorldPresence` or `MenuBackground` as a field of a screen.** Rejected (determination 20). A
  world is a fact of the client app, not of a screen. A screen would copy it from the opener and
  would go stale when the world appears or ends.
- **Bevy `States` for the screen.** Rejected. `States` holds one value at a time and has no
  stack. The set of values is closed.
- **A stack of ids, or the `Screen` enum with a stack.** Rejected. An id table is a second
  registry beside the ECS. The enum names every screen in one crate, so a dialog cannot join it.
- **Messages `EscapePressed` and `CloseTopScreen`.** Rejected. The first draft routed Escape
  through two messages. One system with a `Single` query does the same with no hop.
- **A relationship for the stack.** Rejected. Section "The screen model" gives the reason.
- **A tick system `mark_top_screen`.** Rejected. It moves a marker in a tick system, which the
  lint reports as `marker_toggle`. The observers of `ScreenEntry` move the marker instead.

## What would reopen this

- A crate of the split cannot stay at 15 files or fewer without a merge of two primary items.
- A measurement shows that the retained scene costs more main-world time than the immediate
  scene in a frame with a change, for example a pointer move over a large menu.
- A pack reload loses glyphs or shows a frame with a wrong atlas.
- A screen needs a fact that the stack of entities cannot give.
- The dialog crate needs the player or the world in the configuration phase.

## Objections

These are the strongest objections that the text does not already answer.

- Fifteen crates for one concern. `rust-style` says to split a crate only when a dependency
  forces it. The answer is the column "What forces it". Twelve crates have a dependency that the
  others do not take. Three exist because of the limit of 15 entries, which is also a rule. It
  stands that `gui_primitives` has no dependency of its own.
- `gui_hud` has three files and `gui_debug` has nine. The answer is determination 15: the F3
  screen takes two dependencies that no other HUD module needs.
- `SingleplayerEntry` hides an edge: the title runs world code. The answer is that a user can
  insert another function, or none, and the edge is one `fn` pointer with no `dyn`.
- Four crates sit at the limit (`gui_scene`, `gui_menu`, `gui_creative`, `gui_menus`). The next module in
  them forces a merge or a split. The answer is that the limit then does its job.
- The retained scene adds a key, a gate on each draw and a change trigger for each input. A
  missed trigger shows a stale layer. The answer is that the trigger list is short and the same
  for every draw. A test that changes one trigger and reads the layer catches a stale one.
- The redesign comes first and touches every draw system. A mistake there costs more than a
  mistake in a move. The answer is that steps 1b and 1c stay in one crate, change no behaviour,
  and have the client tests and a screenshot run.
- `gui_render` reads `ScreenStack`, so the renderer knows the stack. The answer is that it knows
  the generic stack and no screen kind.
- With no id registry, no code lists all screens. A query over `ScreenEntry` lists the open
  screens at run time, and each kind component is a type that a search finds.

## Open questions

None is left. Determinations 15 to 20 answer the questions of the second draft.

## References

- [RFD 1: Requests for Discussion](0001-requests-for-discussion.md).
- [RFD 2: Mission, principles and values](0002-mission-principles-values.md).
- `docs/spec/dialog.md`: sections 11, 13 and 18, DLG-FLOW-001 to DLG-FLOW-005, DLG-KEY-002 and
  DLG-KEY-003.
- `docs/ecs-lint.md`: section 5 (rules) and section 10 (event conditions).
- `CLAUDE.md`: Layers, ECS, Crossing a boundary, Ownership, locks, dispatch and allocation.
- `.agents/skills/rust-style/SKILL.md`: Crates and modules.
- `.agents/skills/bevy-ecs/SKILL.md`: the classification of values.
