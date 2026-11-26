# RFD 5: Split the client GUI into crates by screen

- Authors: andreypfau
- State: prediscussion
- Labels: architecture, client, gui

This page proposes to replace the crate `mcrs_minecraft_client_gui` with fifteen small crates.
It also proposes a screen model that a dialog screen can use. Read it if you touch a screen of
the client, if you plan the dialog screen, or if you wonder why the GUI is not one crate.

The maintainer decided the direction: one crate per screen group, a font crate, a scene crate,
and a draw order that no screen has to name. The agent that wrote this page checked the
direction against the code. The code forced changes to the first sketch. The text marks each
one with the words "the code says". The maintainer then decided the open questions of the first
draft, the model of a screen and the chat key. Section "Determination" records each decision.
Section "Open questions" lists what is still open.

## The problem

The crate `mcrs_minecraft_client_gui` has 120 files in a flat `src/` and 23,536 lines. Its only
consumer is `mcrs_minecraft_client`. It breaks the layout rule of `CLAUDE.md` and three more
rules.

- **Wide directory.** `src/` holds 119 entries. The limit is 15. `deslop-please` reports
  `wide-directory`.
- **A module cycle of 34 modules.** `src/scene.rs` has 1,208 lines. It holds the low layer: the
  quad list, the atlas, the batch and the extract into the render world. It also holds the top
  layer: `GuiPlugin` names the draw system of each screen, and `draw_screens` draws the hotbar and
  the open container. Each screen uses `scene.rs`, and `scene.rs` uses each screen.
- **Four smaller cycles.** They are listed below.
- **A font inside the atlas.** `GuiAtlasData::font` is used in 33 places in 19 modules. The
  linter reports it as a field leak. A sign renderer or a map viewer that needs only text layout
  must take the atlas, the network and the client world.
- **A heavy dependency list.** The crate has 60 normal dependencies. A change to the title screen
  rebuilds the code of chat, creative and the F3 screen. A user of one screen pays for all.
- **A fixed screen enum.** `Screen` in `mcrs_minecraft_client_world` has 19 variants, and 30
  files of the GUI match on it. A dialog that the server shows in the configuration phase has no
  place in that enum. `docs/spec/dialog.md` needs a previous screen, an Escape that a screen can
  refuse, a screen that the network replaces, and a pause flag (section 18).

Nobody outside the repository uses the crate today. The cost falls on every agent that edits it
and on the dialog work that comes next.

## What is known

Each fact has the way it was found.

| Fact | Source |
|---|---|
| 120 files, 23,536 lines | `wc -l crates/mcrs_minecraft_client_gui/src/*.rs` |
| 34 field leaks, 5 module cycles, 2 wide directories | `deslop-please mcrs_minecraft_client_gui` |
| The cycle of 34 modules runs through `scene.rs` | the same report |
| `GuiAtlasData::font`: 33 places in 19 modules | the same report |
| Module references | `grep -o 'crate::[a-z_0-9]*'` over each file |
| `font.rs` imports `GuiQuad` from `scene.rs` | `src/font.rs`, lines 186 to 330 |
| The render world reads atlas pixels and the unifont cells | `src/gui_items.rs`, `prepare_gui_pass` and `upload_glyphs` |

The four smaller cycles are not the four of the first sketch. The report names these.

1. `creative_screen_edit`, `creative_screen_input` and `inventory_clicks`.
2. `debug_screen` and `debug_screen_overlay`.
3. `follow_menu_events`, `menu_screens_plugin` and `project_menu`.
4. `font_bitmap`, `font_definition` and `font_unihex`.

Cycle 1 matters most. It shows that the creative screen and the container screen share one
input system. Determination 5 and step 8 return to it.

This page does not know the build time of the split. It does not know the size of the binary.
Neither is a goal.

## Determination

The maintainer decides these points. The agent only worked out the details.

1. The GUI splits into crates by screen, as in the crate map below. The maintainer rejected
   the option to keep one crate and allow the findings.
2. The font is its own resource and its own crate. It is no field of the atlas.
3. The scene crate knows no screen. It defines a `SystemSet` for the draw order. Each screen
   plugin adds its own draw system to a set with `in_set`.
4. A screen is an entity, as point 12 says. The screen state keeps a previous screen, an Escape
   that a screen can refuse, replacement by the network and a pause flag. The dialog crate comes
   later and is not designed here.

The maintainer decided these points after the first draft.

5. **The creative and container seam.** Step 8 splits `inventory_clicks` by screen first. After
   that step, a review decides whether `gui_creative_state` and `gui_gesture` stay separate
   crates. Until then, the crate map shows both as provisional.
6. **`gui_render` stays separate from `gui_primitives`.** A headless client and a bot do not
   take wgpu.
7. **`sign_text` moves into `mcrs_minecraft_client`.**
8. **Step 2 may cross four crate groups.** The maintainer waives the limit for that step only.
9. **The menu crates split three ways:** `gui_menu_options`, `gui_menu_worlds` and
   `gui_menu_session`.
10. **`gui_font` has no dependency on `client_jar`.** The caller gives the font bytes or the
    loaded data, so a map viewer can take the crate alone. After the change, `mcrs_minecraft_client`
    reads the jar. It already depends on `client_jar` and already calls `FontOptions`.
11. **The name `GuiPhase` stays.**
12. **A screen is an entity.** This replaces `ScreenId`, `ScreenSpec` and the stack of ids. The
    section "The screen model" gives the design.
13. **`client_screen` is not layer 3.** It uses Bevy components, messages and observers. It sits
    in the client part of the layers, above the engine.
14. **The chat key is owned by `gui_chat`.** `gui_container` and `gui_creative` do not depend on
    `gui_chat`. `client_screen` has a marker component `AllowsChat`. A screen kind that lets the
    chat key open the chat requires `AllowsChat`, or its spawn inserts it. The input system of
    `gui_chat` reads the chat key. When the top screen has `AllowsChat`, that system opens the
    chat: it spawns the screen entity of the chat.

The agent proposes the rest: the extra crates, the names, the order of the pull requests, and the
way the atlas reaches the render world. The maintainer has not decided these yet.

## Proposal

### Layers and names

The names follow `CLAUDE.md`. A crate that is specific to the game is `mcrs_minecraft_*`. The
prefix `mcrs_minecraft_gui_` groups the new crates, so that one pull request names one crate
group. The old name `client_gui` ends with the old crate. The font and the scene are useful
without a client, so the word `client` does not belong in their names.

| Crate | Layer | Job |
|---|---|---|
| `mcrs_minecraft_gui_font` | 6 Rendering | Font data, glyph cache, text layout and wrap, translation of text |
| `mcrs_minecraft_gui_scene` | 6 | The quad list, the atlas, the draw order, the GUI size |
| `mcrs_minecraft_gui_primitives` | 6 | What any screen draws: text, tooltip, item and slot icon, decorations, tile, edit box part |
| `mcrs_minecraft_gui_render` | 6 | The batch, the pipeline, the extract into the render world |
| `mcrs_minecraft_gui_menu` | 6 | The host of a widget menu: draw, pointer and key input, cursor |
| `mcrs_minecraft_client_screen` | Client, above 4 Engine | The screen stack and the shared properties of a screen |
| `mcrs_minecraft_gui_chat` | 7 Applications | Chat screen, chat key, chat input, suggestions |
| `mcrs_minecraft_gui_creative_state` (provisional) | 7 | Creative content, tabs, grid, search, click mapping |
| `mcrs_minecraft_gui_gesture` (provisional) | 7 | The click, drag and quick-move gestures of a container |
| `mcrs_minecraft_gui_container` | 7 | Container screens, hotbar, container clicks |
| `mcrs_minecraft_gui_creative` | 7 | The creative screen: draw, input, tooltip |
| `mcrs_minecraft_gui_hud` | 7 | Health, game mode switcher, F3 screen |
| `mcrs_minecraft_gui_menu_options` | 7 | Options and its sub screens |
| `mcrs_minecraft_gui_menu_worlds` | 7 | World list, world create, world delete |
| `mcrs_minecraft_gui_menu_session` | 7 | Title, pause, death, disconnect, loading, message |

Layer 6 crates depend on neither the network, the client world nor the server. That is the
rule for rendering. The screen crates read the client world and the network, so they sit in
layer 7. `client_screen` uses Bevy components, messages and observers, so it is not a crate of
layer 3. It sits in the client part of the layers, above the engine. It depends on `bevy_ecs`
and on no player and no world, and it sits below the client world, so that the configuration
phase can use it.

The rows of `gui_creative_state` and `gui_gesture` are provisional. The review after step 8
decides whether they stay two crates (determination 5).

A later crate `mcrs_minecraft_gui_dialog` is layer 7. Section "Dialog readiness" states its
limits.

### Dependency graph

An arrow means "depends on". Each crate also depends on the crates below it that it names in
its own list. This graph shows only the direct edges between GUI crates.

```
font <- scene <- primitives <- render
                    ^  ^
                    |  +-- menu <- menu_options <- menu_session <- menu_worlds
                    |         ^
                    |         +--- chat
                    |         +--- dialog (later)
                    +-- creative_state <- gesture <- container <- creative
                    +-- hud

client_screen (client, above the engine): below client_world; used by client_world, by every
  screen crate and by dialog (later). It depends on bevy_ecs only. It defines AllowsChat,
  so chat, container and creative share no edge for the chat key.

provisional: creative_state, gesture (review after step 8)
```

Three rules hold for the graph.

- No edge goes up. `scene` names no screen.
- A screen crate opens a screen of another crate by spawning the kind component of that
  screen. So the opener names the type that it opens, and it depends on the owner. The title
  screen and the pause screen depend on `menu_options` this way. A world screen gives a message
  or a disconnect reason, so `menu_worlds` depends on `menu_session`. The other edges are the
  chain `gesture`, `container`, `creative`.
- The chat key has no edge. `gui_chat` owns the key and opens the chat when the top screen has
  `AllowsChat` (determination 14). `container` and `creative` do not depend on `chat`.
- `render` is the only crate that needs the render world. A headless client, such as a bot,
  leaves `render` out and keeps everything else.

### The draw order

The scene crate defines the set that orders a frame. The name `GuiDraw` is taken: it is the
struct of one draw call in the batch. The set is `GuiPhase`.

| Variant | What runs in it |
|---|---|
| `Begin` | Clear the scene. Read the window size. |
| `Screens` | The draw system of a screen. |
| `Overlay` | What draws above a screen: tooltip, drag preview, fade. |
| `Batch` | Turn the scene into the batch. |

`GuiScenePlugin` configures the four variants as a chain. Each screen plugin adds its draw
system with `in_set(GuiPhase::Screens)` and its own run condition. Order inside one variant is
free, because two screens never draw in one frame. The old `GuiPlugin` and its list of draw
systems end.

### What each crate holds

The lists use the file names of today. A name with an arrow marks a new module.
"Merge" means that two modules become one. Each crate keeps at most 15 files, `lib.rs` and the
test modules included. Test modules count, and they move with the code they test.

**`mcrs_minecraft_gui_font`** (9 files). Modules: `font`, `font_definition` (merged with
`font_bitmap`), `font_glyph`, `font_unihex`, `glyph_cache`, `language`, `text`,
`font_dump_tests`.

- The code says: `font.rs` also holds the functions that turn text into `GuiQuad`
  (`draw_text`, `draw_str`, `draw_centered_text`, `draw_glyphs`). They need the scene, so they
  move to `primitives` as `text_draw`. `Font` keeps metrics, `span_at` and `Span`.
- `language` and `text` move here. `ClientLanguage` produces `Span`, and `text` wraps `Span`.
- Public names: `Font`, `FontOptions`, `FontCells`, `Span`, `Glyph`, `GlyphImage`,
  `ImageSource`, `BitmapPage`, `GlyphCache`, `ClientLanguage`, `Wrap`, `text_spans`, `wrap`,
  `argb`, `shadow_color`.
- `FontCells` is new. It holds the unifont bitmaps that the render world uploads. The section
  "The atlas and the render world" says why.
- Dependencies: the resource set (`mcrs_minecraft_model`), `mcrs_minecraft_lang`,
  `mcrs_minecraft_text`, `mcrs_minecraft_text_flatten`, `bevy_math`. A `bevy` feature adds the
  `Resource` derive. The crate has no dependency on `mcrs_minecraft_client_jar`.
- The code says: only `font_unihex` names `client_jar::Directory`, to read the hex files. The
  loader instead takes the font bytes or the loaded definitions from its caller.
  `mcrs_minecraft_client` reads the jar through `Directory` and gives them to `FontOptions`. A
  map viewer takes `gui_font` alone and gives its own bytes.

**`mcrs_minecraft_gui_scene`** (14 files). Modules from `scene.rs`, one per item: `gui_size`,
`gui_config`, `gui_quad`, `gui_scene` (with `SceneIcon`), `gui_atlas_layout`,
`gui_atlas_image`, `gui_atlas_load`, `gui_phase`, `gui_scene_plugin`. From other files:
`sprite`, `gui_sprite_id`, `gui_texture_id`, and the atlas fixture of `menu_test_app` under a
`test-support` feature.

- Public names: `GuiSize`, `GuiConfig`, `GuiQuad`, `GuiScene`, `SceneIcon`, `GuiAtlasLayout`,
  `GuiAtlasImage`, `load_gui_atlas`, `GuiSprite`, `GuiSpriteId`, `GuiTextureId`, `GuiPhase`,
  `GuiScenePlugin`, `sprite` (the function that pushes a sprite quad).
- `GuiAtlasData` splits in two. `GuiAtlasLayout` holds regions, sprites, textures and origins.
  Screens read it. `GuiAtlasImage` holds the pixels, the glint image and the size. Only the
  render world reads it.
- Dependencies: `font`, `mcrs_minecraft_model`, `bevy` (render, image, window).

**`mcrs_minecraft_gui_primitives`** (9 files). Modules: `text_draw` (from `font.rs`),
`tooltip`, `item_tooltip` (from `menu_item_tooltip`), `item_decorations`, `item_icons`,
`slot_icon`, `texture_tile`, `edit_box_part` (the translator of an `EditBoxPart` into quads,
cut from `edit_box_draw`).

- The code says: `creative_search_index` calls `menu_item_tooltip` for item components and
  attack values. The container screen also uses that module. It belongs below both, so it moves
  here.
- Public names: `draw_text`, `draw_str`, `draw_centered_text`, `Tooltip`, `LineGap`,
  `Decorated`, `CountLabel`, `decorations`, `ItemIcons`, `IconMeshes`, `SlotIcon`,
  `SlotIconPlugin`, `draw_edit_box_part`, `item_tooltip_lines`.
- Dependencies: `scene`, `font`, `client_widget`, the item crates (`item_icon`, `item_tooltip`,
  `item_table`, `item_stack`), `mcrs_minecraft_model`.

**`mcrs_minecraft_gui_render`** (7 files). Modules: `atlas_quad`, `gui_batch` (`GuiBatch`,
`GuiDraw`, `GuiDrawKind`), `gui_batch_build` (`build_gui_batch` and the quad pushers),
`gui_items` (the pipeline and the shader), `gui_atlas_extract`, `gui_render_plugin`.

- Public names: `GuiRenderPlugin`, `GuiBatch`, `GuiDraw`, `GuiDrawKind`, `GLINT_ALPHA`.
- The plugin adds `build_gui_batch` to `GuiPhase::Batch`.
- Dependencies: `primitives`, `scene`, `font` and the Bevy render crates.

**`mcrs_minecraft_gui_menu`** (13 files). Modules: `menu`, `menu_draw`, `menu_input`,
`menu_plugin`, `menu_cursor`, `menu_event`, `menu_input_systems`, `scroll_area_draw`,
`tab_bar_draw`, `edit_box_draw` (the glue that finds the hint of a widget), `cursor_grab`,
`edit_input`.

- The code says: `chat_input` holds `screen_key`, `pastes`, `clipboard_text` and `copy_text`.
  The creative search and the container click system call them. They are not about chat. They
  move here as `edit_input`.
- The code says: `menu_draw` reads `Player` from the client world, only to know whether a world
  is present. A dialog in the configuration phase has no player. The value becomes a field of
  the menu that the screen sets (`WorldPresence`).
- Many items of `menu_input` are `pub(crate)` today and are used by other modules. They become
  public here. Step 6 reviews each one and keeps the public surface as small as it can be.
- Public names: `Menu`, `MenuBackground`, `MenuEvent`, `MenuPlugin`, `MenuInputSystems`,
  `WorldPresence`, `draw_menu`, `edit_input`.
- Dependencies: `primitives`, `scene`, `font`, `client_widget`, `client_options`.

**`mcrs_minecraft_client_screen`** (about 6 files, new). See "The screen model".

**`mcrs_minecraft_gui_chat`** (5 files). Modules: `chat`, `chat_input`, `chat_suggestions`,
`chat_screen_plugin`.

- `gui_chat` owns the chat key. Its input system opens the chat when the top screen has
  `AllowsChat`. The kind components of the container screens and of the creative screen require
  `AllowsChat`, so `gui_chat` names none of them.
- Dependencies: `menu`, `primitives`, `client_screen`, `mcrs_minecraft_chat_client`,
  `mcrs_minecraft_command`, `mcrs_minecraft_chat`.

**`mcrs_minecraft_gui_creative_state`** (11 files, provisional). Modules: `creative_screen_state`,
`creative_tab_content`, `creative_tab_view`, `creative_search_index`, `creative_grid`,
`creative_clicks`, `creative_screen_shown`, `loaded_creative_report`, `search_input`,
`creative_search_edit`.

- Dependencies: `primitives`, `menu` (for `edit_input`), `client_screen`,
  `mcrs_minecraft_creative_content`, `mcrs_minecraft_creative_report`.

**`mcrs_minecraft_gui_gesture`** (12 files, provisional). Modules: `gesture_frame`, `hover`, `viewer`,
`modifiers`, `last_press`, `menu_gesture`, `open_drag`, `press_kind`, `release_rule`,
`wheel_notches`, `item_key_codes`.

- These modules are pure state machines. They depend on `creative_clicks` and `creative_grid`
  to map a creative hit to a click. That is why `creative_state` sits below them.
- The review after step 8 may merge the creative parts into `creative`, or the gestures into
  `container`. Step 9 then moves the modules to the crates that the review names.
- Dependencies: `creative_state`, `mcrs_minecraft_inventory_client`,
  `mcrs_minecraft_menu_geometry`.

**`mcrs_minecraft_gui_container`** (12 files). Modules: `menu_screen`, `menu_stacks`,
`menu_label_text`, `inventory_figure`, `hotbar`, `drag_preview`, `inventory_clicks`,
`close_menu_screen`, `follow_open_menu`, `container_draw` (the body of `draw_screens`),
`container_screen_plugin`.

- The code says: `inventory_clicks` has branches for the creative screen (the pointer, the
  scroll drag, the search press) and the key that opens chat. The creative branches move to the
  creative crate in step 8. The chat key moves to `gui_chat` in step 7. This is the first pull
  request that changes a system, not only a path.
- Public names: `InventoryClicksPlugin`, `ContainerScreenPlugin`, `MenuStacks`, `DragPreview`,
  `ScreenEvent`.
- The kind components of the container screens require `AllowsChat`. The crate has no
  dependency on `gui_chat`.
- Dependencies: `gesture`, `menu`, `primitives`, `client_screen`, `client_world`,
  `mcrs_minecraft_container_click`, `mcrs_minecraft_inventory_menu`,
  `mcrs_minecraft_engine_inventory_click`.

**`mcrs_minecraft_gui_creative`** (12 files). Modules: `creative_screen_draw`,
`creative_screen_input` (merged with `creative_screen_edit`), `creative_screen_open`,
`creative_screen_plugin`, `creative_item_tooltip`, `mark_creative_screen`, the three test
modules, and `creative_pointer` (the branches that leave `inventory_clicks`).

- Public names: `CreativeScreenPlugin`, `CreativeContentSystems`.
- `CreativeScreen` requires `AllowsChat`. The crate has no dependency on `gui_chat`.
- Dependencies: `container`, `creative_state`, `menu`, `primitives`, `client_screen`.

**`mcrs_minecraft_gui_hud`** (13 files). Modules: `health_hud`, `hud_systems`,
`game_mode_switcher`, `debug_screen` (merged with `debug_screen_overlay`), `debug_displayer`,
the four `debug_entry_*`, `chunk_map`, `light_levels`, `hud_plugin`.

- Public names: `HudPlugin`, `DebugScreenPlugin`, `DebugScreenDisplayer`, `DebugScreenSet`,
  `HudSystems`.
- The client crate already uses `debug_displayer`. It is the extension point of the F3 screen.
- Dependencies: `primitives`, `client_screen`, `client_world`, `mcrs_minecraft_game_mode`.

**`mcrs_minecraft_gui_menu_options`** (10 files). Modules: `options_screen`,
`option_sub_screen`, `option_caption`, `accessibility_settings_screen`, `controls_screen`,
`mouse_settings_screen`, `video_settings_screen`, `menu_options_systems`,
`options_screen_plugin`.

**`mcrs_minecraft_gui_menu_session`** (13 files). Modules: `title_screen`, `title_draw`,
`panorama_view`, `pause_screen`, `death_screen`, `death_screen_plugin`, `disconnected_screen`,
`level_loading_screen`, `message_screen`, `session_screens`, `follow_menu_events`,
`session_screens_plugin`.

**`mcrs_minecraft_gui_menu_worlds`** (10 files). Modules: `select_world_screen`,
`create_world_screen`, `delete_world_screen`, `world_entry`, `world_rows`,
`world_list_error_screen`, `follow_create_world`, `follow_list_answer`, `worlds_plugin`. It
depends on `menu_session` for the message and the disconnect screen.

- The three menu crates depend on `menu`, `primitives` and `client_screen`. The edges
  between them are `menu_session` to `menu_options` and `menu_worlds` to `menu_session`. The
  title screen and the pause screen open options, and the message screen and the disconnect
  screen take data (a text, a reason). In each case the opener spawns the kind component.
- `project_menu` and `menu_screens_plugin` end. Each menu crate has an observer on the add of
  each kind component that it owns, and the observer builds the `Menu` of that screen. The
  dependency that the old pair had on all screens ends, because no central code builds menus.

**Not in a GUI crate.** `sign_text` has 761 lines. It lays out sign text and reads
`SignFamily` and `SignMount` from the client world. It draws no GUI. The maintainer
decided that it moves into `mcrs_minecraft_client` (determination 7). The client already has
`sign_text.rs` and `name_tag.rs`, which are its two callers.

### The atlas and the render world

`GuiAtlas(pub Arc<GuiAtlasData>)` breaks the rule "no `Arc`". `ExtractResource` also clones
the handle each time the value changes. The render world needs only three things from the
atlas: the pixels, the glint image, and the unifont cells that the glyph cache asks for.

`CLAUDE.md` gives two mechanisms for a buffer. The proposal picks the second one.

- A copy when the version changes. The pixel buffer is a few megabytes. A copy doubles the
  memory for the life of the process and has no use.
- An extract system that takes `ResMut<MainWorld>`. The main world reads no pixel after the
  load. The system removes `GuiAtlasImage` and `FontCells` from the main world and inserts them
  in the render world. One owner holds each value at all times, and no copy exists.

`GuiAtlasLayout` and `Font` stay in the main world. Screens read both. The extract system
lives in `gui_render`. A pack reload inserts a new `GuiAtlasImage`, and the next extract moves it.
The test fixture today holds the atlas in a `LazyLock<Arc<..>>`. It becomes a `LazyLock` of the
two values, which a test app clones. That is test code.

This design rests on one assumption: the main world reads no pixel after the load. The
search `grep -n '\.pixels'` over the crate shows only `scene.rs`, `gui_items.rs` and a test.
Step 4 checks the assumption again. If it fails, the step falls back to the copy.

### The screen model

`Screen` is a `Resource` enum with 19 variants. Six variants carry a payload, and three
payloads are the parent screen (`CreateWorldParent`, `ControlsParent`,
`AccessibilityParent`). The enum is closed, so a screen that the server defines cannot join it.

The maintainer decided that a screen is an entity (determination 12). The model replaces
`ScreenId`, `ScreenSpec`, `ScreenSpecs` and the stack of ids. The new crate
`mcrs_minecraft_client_screen` holds the shared part. It depends on `bevy_ecs` only. It depends
on no player and no world. `client_world` depends on it, not the reverse. It is not a crate of
layer 3, because it uses Bevy components, messages and observers.

**The kind component.** Each open screen is one entity. Its kind is a component that the crate
of the screen owns. Examples are `ChatScreen`, `CreativeScreen` and `PauseScreen`. A later
dialog component holds the data of its dialog. The data of a screen is a field of its kind
component, not a resource. `GameModeSwitcher(GameMode)` becomes the state of the switcher.
`Message(ScreenMessage)` becomes the data of `MessageScreen`. The set of screens is open. No
central enum and no id registry exist. A dialog of the configuration phase is an entity with
no world and no player.

**The shared components** belong to `client_screen`.

| Item | What it is |
|---|---|
| `ScreenEntry` | The component that every kind component requires (`#[require]`). Its add and remove observers keep the stack. It holds `opened_at`, the tick at which the screen opened. |
| `PausesGame` | A marker. A kind component requires it when the screen pauses the game. |
| `EscapeRefused` | A marker. A kind component requires it when Escape must not close the screen. |
| `TopScreen` | A marker on the entity at the top of the stack. |
| `ScreenStack` | A resource, `ScreenStack(Vec<Entity>)`. The last entity is the top. The entity before it is the previous screen. |

**The marker `AllowsChat`** belongs to `client_screen`. A kind component requires it when the
chat key may open the chat above that screen. Its spawn can also insert it. `gui_chat` reads it
(determination 14).

**The messages** are `EscapePressed`, `CloseTopScreen`, `ReplaceScreen`. The input system
writes `EscapePressed`. A system of `client_screen` reads it. When the top screen has no
`EscapeRefused`, the system writes `CloseTopScreen`. A screen with `EscapeRefused` reads
`EscapePressed` and decides for itself. A dialog runs its cancel action this way.
`ReplaceScreen` holds the entity that the sender spawned and a scope, `Top` or `All`. A system
of `client_screen` despawns the entity below the new one for `Top`, and every entity below it
for `All`. A disconnect uses `All`. A disconnect and a message from the network use
`ReplaceScreen`, so the network never writes the stack. A screen is opened by
the system that spawns its kind component, so the opener depends on the owner of the screen.

**Open and close are observers.** To open a screen, a system spawns an entity with the kind
component. The add observer of `ScreenEntry` pushes the entity on the stack. The add observer
of the kind component, in the crate of the screen, builds its menu or its state. To close a
screen, a system despawns the entity. The remove observer of `ScreenEntry` takes the entity
out of the stack, wherever it stands. So a despawn from any code leaves no stale entity in
the stack.

**Reads.** A draw system selects the top screen with a `Single` query, for example
`Single<&ChatScreen, With<TopScreen>>`. The system does not run when no match exists, so the
`Single` also works as the run condition. The parent screen is the entity before the top in
the stack. Pause is derived. The game is paused when `Query<(), (With<TopScreen>,
With<PausesGame>)>` is not empty. Nothing stores the pause a second time.

**Why a resource and not a relationship.** The stack needs a defined order, a top in constant
time, a replace of all entries and a removal from the middle. A Bevy relationship such as
`ChildOf` needs a root entity, and the order of its target list is not a contract of the
stack. A relationship that points to the previous screen gives the order, but "the top" and
"replace all" then need a walk. A resource holds the order in one place, and one owner writes
it. The cost is that the resource must follow the entities. The remove observer does that.

**The top marker is a structural change.** One system, `mark_top_screen`, runs when
`ScreenStack` changes. It removes `TopScreen` from the old top and inserts it on the new
top. `CLAUDE.md` forbids a structural change on a hot path. A screen opens a few times a
minute at most, so this path is not hot. The marker pays for itself, because every draw and
input system selects the top screen by archetype, and no system scans the stack.

**Classification of each value** (skill `bevy-ecs`).

| Value | Class | Owner |
|---|---|---|
| The kind component and its data | Authoritative | The crate of the screen |
| `PausesGame`, `EscapeRefused`, `opened_at` | Authoritative, fixed at spawn | The kind component that requires them |
| The order of `ScreenStack` | Authoritative. No other value gives the order. | The observers of `ScreenEntry` and the `ReplaceScreen` system |
| The age of a screen | Derived: the tick now minus `opened_at` | Nobody. Each reader computes it. |
| The parent screen | Derived: the entity before the top | Nobody |
| Pause | Derived from the top screen | Nobody |
| `TopScreen` | Materialized from `ScreenStack`, for query selectivity | `mark_top_screen`, the one writer |
| `AllowsChat` | Authoritative, fixed at spawn | The kind component that requires it |
| `EscapePressed`, `CloseTopScreen`, `ReplaceScreen` | Temporal facts and requests | The system that writes each message |

The model keeps the stack as the only list of screens. The old `Screen` enum, its three
parent enums and the table of specs end. The step that moves the last caller of `Screen`
deletes the enum (step 2).

**Behaviour does not change in step 2.** The step reproduces today's screens, their parents
and their Escape rules. It adds no feature. The dialog features (refusal, replacement by the
network, the age of a screen) have a mechanism in the crate. The step uses them only where
today's code already behaves that way.

### Dialog readiness

`docs/spec/dialog.md` section 18 lists the widgets. Sections 11 and 13 give the flow. The
split must leave room for a dialog crate. This page does not design that crate.

The limits that the split puts on it:

- It depends on `menu` (the widget host), `primitives`, `scene`, `font`, `client_screen` and
  `client_widget`. It does not depend on `client_world`, on a player or on any container
  crate. A dialog can show in the configuration phase, where no world exists
  (DLG-FLOW-005).
- It draws through the atlas and the font only. That is why `menu` loses the `Player` read.
- The screen entities serve it. A dialog replaces a dialog (DLG-FLOW-002) with `ReplaceScreen`.
  A `close` after action despawns the top entity, and the previous screen becomes the top.
  `clear_dialog` is a message of the network systems. The waiting screen and the warning screen
  are entities of the same stack. A disconnect uses `ReplaceScreen` with the scope `All`
  (DLG-FLOW-004).
- A dialog kind component requires `EscapeRefused`, because Escape runs the cancel action.
- `client_widget` has 69 files, which `deslop-please` also reports. The dialog depends on it,
  but the split of that crate is a separate decision.

## Migration

Each step is one pull request of fewer than about 3,000 added lines. A moved line counts as an
added line. Each step leaves `main` green and moves behaviour nowhere. The step that moves the
last caller of a path deletes the path. The old crate lives until step 14 and loses modules
as they leave. A step names its own crate group.

Line counts come from `wc -l` of the modules that move. They are estimates of the diff.

| Step | Crate group | Content | Lines |
|---|---|---|---|
| 1 | `client_gui` only | Prepare inside the crate | about 2,500 |
| 2 | `client_screen`, `client_world`, `client_gui`, `client`, `chat_client` | The screen entities and the stack (limit waived) | about 1,200 |
| 3 | `gui_font`, `client` | Move the font, and move the jar reading to `client` | about 1,900 |
| 4 | `gui_scene` | Move the scene and the atlas | about 1,900 |
| 5 | `gui_primitives` | Move the primitives | about 1,900 |
| 6 | `gui_render`, `gui_menu` | Move the batch, the pipeline, the extract and the menu host | about 2,900 |
| 7 | `gui_chat` | Move chat, and move the chat key into it | about 1,300 |
| 8 | `client_gui` only | Split `inventory_clicks` by screen, then review the seam | about 700 |
| 9 | `gui_creative_state`, `gui_gesture` (provisional) | Move the state and the gestures, as the review decides | about 2,200 |
| 10 | `gui_container` | Move the container | about 2,000 |
| 11 | `gui_creative` | Move the creative screen | about 2,400 |
| 12 | `gui_hud` | Move the HUD and the F3 screen | about 2,300 |
| 13 | `gui_menu_*` | Move the three menu crates, one pull request each | about 800 to 2,300 each |
| 14 | `client`, workspace | Delete `client_gui`, update `docs/crates.md` | small |

Step 6 puts two crates in one pull request because `gui_render` alone is small and `gui_menu`
needs the extract to run a screenshot. If the diff passes 3,000 lines, it splits in two.

### Step 1: prepare inside the crate

The goal is to remove the cycles while the code is still one crate. A move later is then a
file move with no edit.

1. Split `scene.rs` into one file per item (the modules of `scene`, `render` and `primitives`
   above), with no new crate.
2. Add `GuiPhase`. `GuiPlugin` stops naming screens. Each screen plugin adds its draw system
   with `in_set`. `draw_screens` becomes `container_draw` and joins the container plugin.
3. Take the font out of the atlas. `Font` becomes a resource, and `load_gui_atlas` takes it.
   The 33 places read `Res<Font>` or take `&Font` as a parameter.
4. Move the `draw_*` functions out of `font.rs` into `text_draw`.
5. Merge `font_bitmap` into `font_definition`. Merge `debug_screen_overlay` into
   `debug_screen`.
6. Split `GuiAtlasData` into `GuiAtlasLayout` and `GuiAtlasImage`. Keep the copy of the image
   for now. Step 4 replaces it with the extract by move.
7. Move `edit_input` out of `chat_input`, and the `Player` read out of `menu_draw`.

### Step 2: the screen entities and the stack

The step adds `mcrs_minecraft_client_screen`. It gives each of the 19 variants of `Screen` a
kind component in the crate that owns the screen, and today that crate is `client_gui`. It
adds `ScreenStack`, the shared components and `AllowsChat`. It deletes `Screen` and its three parent enums
from `client_world` in the same step, and moves all uses. About 30 files of the GUI, 7 in
`client` and a few tests change. The change is mechanical, and one path dies in it.

The step crosses four crate groups. The maintainer waived the limit for this step
(determination 8), because the other path is a second model for the length of the migration,
which the rules forbid.

The step precedes the moves of the screens, because the three menu crates need the kind
components. Without them, `project_menu` stays a hub, and its cycle of 3 modules does not
end. The kind components stay in `client_gui` until each screen moves. Each later step moves
the kind component with its screen.

### Steps 3 to 7: the lower layers

Step 3 also changes the font loader. It takes the font bytes or the loaded definitions from its
caller, and drops `mcrs_minecraft_client_jar` from `gui_font`. `mcrs_minecraft_client` reads the
jar and calls the loader. The move of `sign_text` into `mcrs_minecraft_client` is a small pull request of its own. It
lands before step 14, which deletes the old crate.

Each step moves modules with `git mv` semantics: no edit besides `use` paths and `Cargo.toml`.
Each step shortens the dependency list of the old crate, and gives the new crate only the
dependencies it uses. Step 4 turns the atlas into the extract by move. Step 6 adds
`GuiRenderPlugin` to the client app, and the client app adds it only when it has a window.

### Steps 8 to 13: the screens

Step 7 also edits one system. It moves the chat key out of `inventory_clicks` into the input
system of `gui_chat`, which opens the chat when the top screen has `AllowsChat`. The kind
components require `AllowsChat` since step 2. Step 8 is the other step that edits systems. It splits the creative branches out of
`inventory_clicks` into `creative_pointer`, which runs on the same gestures. It runs before the
moves, so that cycle 1 is gone before the crates split. A screenshot of the creative screen
and the container screen with `run-client` checks it, as does the existing client test
`creative_screen_clicks`.

After step 8 merges, a review decides whether `gui_creative_state` and `gui_gesture` stay
separate crates (determination 5). The review looks at the edges that remain between the
creative modules and the container modules. Step 9 follows the result. Until the review ends,
the crate map shows both crates as provisional.

Steps 9 to 12 move modules. Each plugin appears in the client app under its own name. Step 13
dissolves `project_menu` and `menu_screens_plugin` into observers of the three menu crates.

### What `deslop-please` reports

The first row is today's report. The rows after it are predictions, checked by the pull
request. A step must not add a finding. A step may remove one.

| After step | Module cycles | `wide-directory` of the old `src/` | Field leaks |
|---|---|---|---|
| now | 5 (34, 3, 3, 3, 2 modules) | 119 entries | 34 |
| 1 | 1 (`follow_menu_events`, `menu_screens_plugin`, `project_menu`) and 1 (`creative_screen_edit`, `creative_screen_input`, `inventory_clicks`) | 119 | 33 (the `font` leak ends) |
| 2 | 0 | 119 | 33 |
| 3 to 7 | 0 | falls by the modules moved, ends at 0 files left of that group | falls as owners move |
| 8 to 13 | 0 | 0 | each leaked field is fixed or leaves with its crate. The review after step 8 can change the crates of steps 9 to 11, and the row stays valid. |
| 14 | 0 | the crate no longer exists | the leaks live in the new crates |

The 34 field leaks are not solved by the move alone. A new crate that keeps a leaky field
still reports it. Each pull request fixes the leaks of the types that it moves, or says in its
body why it does not. The new crates start at zero `wide-directory` and zero cycles.

`crates/` stays wide. `.deslop-please.allow` already allows it.

## Options

The maintainer considered these options.

- **Keep one crate and allow the findings.** The maintainer rejected it. An entry in
  `.deslop-please.allow` hides a cycle of 34 modules and a directory of 119 files. The report
  then stops showing the next cycle too.
- **Modules inside one crate.** Replace the flat files with directories, one per screen. It
  keeps one `Cargo.toml` and costs no crate boundary. The layout rule rejects it. `src/` is
  flat, and a crate that needs `foo/mod.rs` is too big. The cycle stays too, since a module
  cycle is a reference between sibling modules, and the draw systems still need the scene.
  The dependency list also stays at 60 normal dependencies, so the user of one screen pays for
  all.
- **Split only the low layer.** Take out `font` and `scene` and keep one crate of screens. It
  ends the cycle of 34. It leaves a crate of about 90 files. The wide directory stays.
- **The decided crates only: font, scene, five screen crates.** The code says it does not fit.
  The scene crate would hold 22 files. Two screen crates would depend on each other in a
  circle (creative and container). One menu crate would hold 28 modules. This page keeps the
  decided idea and adds the crates that the 15-file rule and the references force.
- **Bevy `States` for the screen.** Rejected. `States` holds one value at a time and has no
  stack, so a previous screen has no place. The set of values is closed, and a screen that
  server data defines does not fit an enum.
- **A stack of ids (`ScreenId`, `ScreenSpec`, a table of specs).** Rejected. It is a second
  registry beside the ECS, and the owner of each screen must fill it. The entity and its
  components already hold the same facts.
- **Keep the `Screen` enum and add a stack of it.** Rejected. The enum names every screen in
  one crate, so a dialog or a mod screen cannot join it. A second model next to the first
  also breaks the rule on migrations.
- **`OpenScreen(ScreenKey)`, a message with a constant screen name.** Rejected. A constant name
  is a registry of ids beside the ECS, which the screen-as-entity model rejects. A wrong key
  also fails silently at run time.
- **One message per target in `client_screen`.** Rejected. `client_screen` would know concrete
  screens, and the set of screens would close again.
- **A relationship for the stack.** Rejected. The section "The screen model" gives the reason.

## What would reopen this

- A crate of the split fails to stay under 15 files without a hack.
- Step 8 shows that the creative screen and the container screen cannot run on separate
  systems, so a seam crate does not fit. The two then merge into one crate of 24 modules,
  and that crate splits by file kind. The review after step 8 can also merge
  `gui_creative_state` and `gui_gesture` into their neighbours.
- A screen needs a fact that the stack of entities cannot give, for example a hot path that
  cannot take the cost of the top marker. The measurement then goes in the pull request.
- A measurement shows that the incremental build of one screen is slower after the split
  than before. The maintainer can then merge crates that always change together.
- The dialog crate needs the player or the world in the configuration phase. The screen stack
  and the menu host would then be wrong, and this page needs a note.
- The extract by move loses a frame on a pack reload that the copy does not lose.

## Objections

These are the strongest objections that the text does not already answer.

- Fifteen crates for one concern. `rust-style` says to split a crate only when a dependency
  forces it, and not by topic alone. The answer is that two forces exist. The 15-file rule
  forces a split of 120 files, and the dependency lists differ: world select needs the save
  crate, chat needs the command crate, creative needs the content crates, the font needs
  neither. The count of fifteen is an outcome of the references, not a goal. It stands that
  the count is high, and that some boundaries (`gesture`, `creative_state`) exist only to
  break a cycle.
- The split costs more than it saves, because one agent edits one screen at a time. The
  linter already shows the cost: each agent that reads the crate meets 120 files and a cycle
  of 34. The saving is not measured. It stands as an assumption.
- Step 2 changes the screen model first and for all crates. A mistake there costs more than a
  mistake in a move. The answer is that the step is a refactor with no new behaviour, and
  that it has the existing client tests and a screenshot run. The maintainer waived the
  crate-group rule for it.
- `gui_render` and `gui_primitives` are not in the maintainer's first list. The scene crate
  would otherwise hold 22 files, and a headless client would pay for the pipeline. The
  maintainer confirmed the deviation (determination 6).
- With no id registry, no code lists all screens. The answer is that the set must stay open.
  A query over `ScreenEntry` lists the open screens at run time, and each kind component is
  a type that a search finds.
- The stack resource can disagree with the entities. The answer is that two observers of
  `ScreenEntry` are the only writers, and they run on every spawn and despawn.
- Moving the atlas by `MainWorld` couples the main world to the render world in one more place.
  `CLAUDE.md` names this mechanism for buffers. The coupling is one system.

## Open questions

None is left. The maintainer decided the eight questions of the first draft (determinations 5
to 12) and the chat key (determination 14).

The review after step 8 (determination 5) is a planned decision, not an open question of this
page. It adds a dated note to this RFD when it ends.

## References

- [RFD 1: Requests for Discussion](0001-requests-for-discussion.md).
- [RFD 2: Mission, principles and values](0002-mission-principles-values.md).
- `docs/spec/dialog.md`: sections 11, 13 and 18, and DLG-FLOW-001 to DLG-FLOW-005.
- `CLAUDE.md`: Layers, Crossing a boundary, Ownership, locks, dispatch and allocation.
- `.agents/skills/rust-style/SKILL.md`: Crates and modules.
