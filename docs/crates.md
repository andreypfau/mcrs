# Crate map

This file is generated from the `description` of each crate under `crates/`.
Do not edit it. Run `cargo run -p xtask -- crate-map` to regenerate it.

Read this file before you write a helper or a value type.
Use the crate that owns the item. Do not write a second copy.

## bedrock

- `mcrs_minecraft_bedrock_protocol`: Bedrock Edition protocol crate: batch framing, compression and the handshake packets
- `mcrs_minecraft_bedrock_protocol_game`: Bedrock Edition game packets: StartGame, Text, MovePlayer, PlayerAuthInput, SetTime, UpdateBlock, ChunkColumn
- `mcrs_minecraft_bedrock_wire`: Bedrock Edition wire encoding: varints, little-endian primitives, byte arrays and the packet header
- `mcrs_minecraft_bedrock_wire_macros`: Derive macros for the codec traits of mcrs_minecraft_bedrock_wire

## biome

- `mcrs_minecraft_biome`: Biomes, climate parameters and biome sources
- `mcrs_minecraft_biome_file`: The biome file as data packs write it, its built-in draft, and the columns it is stored as

## block

- `mcrs_minecraft_block`: Block and fluid definitions, block tags and map colours
- `mcrs_minecraft_block_break`: The survival dig kernel: the rate of one tick, the progress sum, the crack stage and what a finish action does
- `mcrs_minecraft_block_entity`: Block entities in the typed shape the save, the chunk packet and the anvil reader share
- `mcrs_minecraft_block_predicate`: Block states, block predicates and block state providers as data packs write them
- `mcrs_minecraft_block_support`: Block support: the support predicates of the block definitions, baked and evaluated, and the shape function that removes a block without support
- `mcrs_minecraft_block_transformer`: The block transformer registry value and its keys

## chat

- `mcrs_minecraft_chat`: The chat rules that the client and the server share: the characters a message may hold, its length limit and its normal form
- `mcrs_minecraft_chat_client`: The client chat screen without rendering: the input box, the history, client-side command suggestions, hint lines and the chat log
- `mcrs_minecraft_chat_server`: The chat handling of the server as a plugin that a user can replace

## client

- `mcrs_minecraft_client`: Minecraft client: window, asset corpus, environment registries and the voxel renderer
- `mcrs_minecraft_client_control`: A local socket that reads the state of a Minecraft client and drives its input, one text line in and one JSON line out
- `mcrs_minecraft_client_entity`: The packet side of the client entities that the server adds: the packets that add, move and change an entity, and the network id of each entity
- `mcrs_minecraft_client_gui`: The GUI of a Minecraft client: hotbar, inventory and chat screens, game mode switcher, slot item models and the F3 debug screen
- `mcrs_minecraft_client_jar`: Locates, downloads and unpacks the vanilla client jar
- `mcrs_minecraft_client_options`: The options a user sets on the client, with their ranges and their JSON form
- `mcrs_minecraft_client_widget`: The widgets of the client GUI without rendering: rectangles, layout, pointer and keyboard input, focus and widget state
- `mcrs_minecraft_client_world`: The world state of a Minecraft client without a window or a renderer: session registries, columns, the local player, game mode and chat

## command

- `mcrs_command`: A command tree, a parser and suggestions that agree with Brigadier, without allocation on the parse
- `mcrs_minecraft_command`: The command source, permissions, identifiers, and the error text the vanilla argument families share, and the command packets of a player
- `mcrs_minecraft_command_argument`: The resource, keyword, coordinate, rotation, swizzle and time arguments
- `mcrs_minecraft_command_argument_block`: The block predicate argument and its test of a block state
- `mcrs_minecraft_command_argument_entity`: The entity selector argument and the search for the entities that a selector names
- `mcrs_minecraft_command_argument_vanilla`: The vanilla argument set, its errors, its parsers on the wire and the client command tree
- `mcrs_minecraft_command_engine`: The command tree, the registration of commands and the run of the command lines of the players inside the tick of each dimension world
- `mcrs_minecraft_command_registry`: The registries and the block table that a host lends to the command arguments
- `mcrs_minecraft_command_tree`: The nodes of the command tree that the commands packet carries
- `mcrs_minecraft_command_vanilla`: The vanilla commands execute, gamemode, teleport, locate, summon and time, and the command effects they write
- `mcrs_minecraft_command_world`: The commands that change the world and the server: fill, forceload, gamerule, weather clear, and tick query and sprint

## creative

- `mcrs_minecraft_creative_content`: The tab lists and the search list of the creative screen, built from the creative report and the session
- `mcrs_minecraft_creative_report`: The creative report: the stacks of each items tab of the creative screen and the feature flags of each item

## dimension

- `mcrs_minecraft_dimension`: The dimension and dimension type registry values
- `mcrs_minecraft_dimension_environment`: Dimension types and the environment attribute layers of a dimension

## engine

- `mcrs_engine_cell`: Worlds that tick in lockstep: owned input in, owned output out, each world placed inline or on a thread of its own
- `mcrs_engine_link`: The link between two parts of the engine: owned messages in, owned messages out, with the far end placed inline or on a thread of its own
- `mcrs_engine_muxer`: Fans the changes of a world out to its viewers: tiles and tile streams for entities, version streams for chunks, and bytes encoded once and shared
- `mcrs_engine_store`: The data layer: versioned chunks by key in an in-memory cache over a disk store that the game supplies, reached as a stage on a link
- `mcrs_minecraft_engine`: The packet bus, the bridge between the network and the dimension worlds, sessions, the host side of the dimension channels, the player state that the host and the dimensions share, the world clocks of the host, and heightmap upkeep
- `mcrs_minecraft_engine_attributes`: The attribute instances of the players of a dimension world: each player requires them with the bases of its type
- `mcrs_minecraft_engine_brain`: The AI stage of the mobs of a dimension world: the snapshot of the mob entities, the stage run per tile beside the entity pass, the decisions applied through the mob controls
- `mcrs_minecraft_engine_damage`: The hit pipeline of a dimension world in Bevy: the queue of hit requests, the sorted apply step, the counters of the health state and the damage type rules from the registry
- `mcrs_minecraft_engine_entity_motion`: The motion of the moving entities that are not living, such as items and falling blocks, as Bevy components: one parallel pass with the physics kernel
- `mcrs_minecraft_engine_entity_motion_replication`: The replication of the moving entities, such as mobs, items and falling blocks: their network ids and their tile events for the muxer
- `mcrs_minecraft_engine_inventory_click`: The driver of the Java click kernel over menu entities: the plan of a click against the open menu of a viewer and the drag of each menu
- `mcrs_minecraft_engine_inventory_menu`: Menus as entities of a viewer: layout, parts and buttons, with the commands that open and close them
- `mcrs_minecraft_engine_item`: Item stacks as entities: their components, their placement in slots or in the world, inventory requests and the equipped marker
- `mcrs_minecraft_engine_mob_attributes`: The attribute instances of the mob entities of a dimension world: the apply system of their edits and the motion values of their bodies
- `mcrs_minecraft_engine_physics`: The entity pass of a dimension world in Bevy: the motion plugin, the mob spawn and the crowd damage over the entity components
- `mcrs_minecraft_engine_player_vitals`: The health state of the players of a dimension world: each player gets its vitals, hit record and defence when it spawns
- `mcrs_minecraft_engine_registries`: The RegistryLoadPlugin: loads the registries and items from the pack files, walks the app to playing, and hands the snapshot to each dimension world
- `mcrs_minecraft_engine_server`: The server plugins of the engine without vanilla behaviour: the core, the dimension worlds, the listener with login, configuration, keep-alive and status, and the server loop
- `mcrs_minecraft_engine_world`: Chunk and column entities, dimensions, tickets, area of interest, sessions, the server loop, block updates and drops, game rules, and the world seed, birth clock and entity ids that new entities take

## entity

- `mcrs_minecraft_entity`: Plain identity types of the entity domain registries and the type data table of the entity types
- `mcrs_minecraft_entity_animation`: Entity animation on the CPU: clips, keyframes, the play list of a client entity, its scripts and the bone pose of a frame
- `mcrs_minecraft_entity_attribute`: The entity attributes, their modifier operations and the computation of an attribute value from its base value and its modifiers
- `mcrs_minecraft_entity_key`: The keys of an entity without the physics kernel: its id, its UUID as its order key, the sources of a new UUID, its keyed random stream and the tile that holds it
- `mcrs_minecraft_entity_model`: Entity geometry files of the Bedrock format: reading, baking to a static model and the GPU model records
- `mcrs_minecraft_entity_render`: GPU-driven entity rendering: persistent slots, the entity cull, the pose and expand passes and the gbuffer draw of the face records
- `mcrs_minecraft_entity_visual`: The state of an entity that a viewer draws: placement and interpolation, age, head and body yaw, walk, hurt and death, entity data and equipment, the tick that advances them and the values of one frame

## game

- `mcrs_minecraft_game_mode`: The game modes of a player, their names and their wire forms, and the abilities that a game mode gives
- `mcrs_minecraft_game_registries`: The vanilla registry declarations and their loader: chat types, villager trades, world presets, test instances, enchantment providers and their keys
- `mcrs_minecraft_game_rule`: Game rules, their categories, value types, defaults and bounds

## inventory

- `mcrs_minecraft_inventory_click`: The operations of a Java Edition menu click: pick up, quick move, swap with a hotbar key, clone, throw, drag and pick up all
- `mcrs_minecraft_inventory_client`: The client edge of the player inventory: the inventory packets of the server applied to the item holders of the local player, and the clicks of the local player predicted and sent
- `mcrs_minecraft_inventory_client_view`: The state id of a menu and the server's view of the stacks that the client holds
- `mcrs_minecraft_inventory_file`: The stacks of a player and its selected slot, read from and written to the player file
- `mcrs_minecraft_inventory_menu`: The menu layout as a plain value: entries, quick-move routes, pick up all and drag permits
- `mcrs_minecraft_inventory_player`: The slots of a player: their indices, slot limits and filters, and the insert rule
- `mcrs_minecraft_inventory_server`: The packet edge of the player inventory: packet decode, clicks to requests, and the sync of the inventory menu
- `mcrs_minecraft_inventory_transaction`: The planner of an atomic inventory request: split, merge, swap and drop over slots, applied in full or rejected with a reason

## item

- `mcrs_minecraft_item`: Item definitions and the item table, data components, ComponentPatch, wire and hashed stack forms, and the ItemStack of an item id and a count
- `mcrs_minecraft_item_entity`: The rules of an item entity: its counters, its start values, its pickup and its merge
- `mcrs_minecraft_item_icon`: The icon of an item stack as a value, and the GUI-posed shaded meshes of the item models it draws
- `mcrs_minecraft_item_key`: The keys of the item registry and of its tags
- `mcrs_minecraft_item_stack`: The item stack value of kind, count and typed component patch, with every item component type and its saved and hashed forms
- `mcrs_minecraft_item_table`: The prototype of each item kind and the effective components of an item stack
- `mcrs_minecraft_item_tooltip`: The parts of an item tooltip as text values, from an item stack, the item table and the inputs the caller fills

## light

- `mcrs_minecraft_light_color`: Light colours of block states and the light a held item emits
- `mcrs_minecraft_light_engine`: Block and sky light propagation over sections, without Bevy
- `mcrs_minecraft_light_render`: Coloured block light for the lighting pass: a paged light volume propagated on the GPU

## loot

- `mcrs_minecraft_loot`: The keys of the loot registries
- `mcrs_minecraft_loot_condition`: The loot context, the loot conditions and the context number providers, baked and evaluated
- `mcrs_minecraft_loot_table`: Loot tables as a data pack writes them, baked by id and evaluated

## menu

- `mcrs_minecraft_menu_geometry`: The geometry of a menu screen as data: the window area, the entry areas and the entry under the pointer
- `mcrs_minecraft_menu_key`: The keys of the menu registry

## model

- `mcrs_minecraft_model`: Block and item models baked from a Minecraft resource pack: the pack, model files, sprite atlas, block catalog and item model trees
- `mcrs_minecraft_model_file`: Block model files, block state files and the face bake of a resource pack, with no Bevy

## molang

- `mcrs_molang`: Molang compiler, scalar and batch evaluator, independent of any game host
- `mcrs_molang_wgsl`: Lowering of pure Molang channels to WGSL and the WGSL math library

## move

- `mcrs_minecraft_move_predict`: The movement checks with a world: the prediction of each client tick of a player from the physics model
- `mcrs_minecraft_move_validate`: The movement checks without a world: one serverbound packet, and the stream of one connection

## nbt

- `mcrs_minecraft_nbt`: NBT tags and the Nbt type, serde serializers and deserializers, the three encodings, compression and the read budget
- `mcrs_minecraft_nbt_path`: An NBT path read from text, and its selection, get-or-create and set on NBT tags

## protocol

- `mcrs_minecraft_protocol`: Protocol library for Minecraft: Java Edition: framing, compression, and the handshake, status, login and configuration packets
- `mcrs_minecraft_protocol_game`: The play-state packets of Minecraft: Java Edition that carry game data: chunks, entities, items, recipes and chat

## registries

- `mcrs_minecraft_registries`: The Registries store: registry tables, tags, declarations, diagnostics, and the loader and wire forms of a registry snapshot
- `mcrs_minecraft_registries_derive`: The registry_value attribute and the Resolve derive of mcrs_minecraft_registries

## render

- `mcrs_minecraft_render`: Deferred terrain and sky renderer: a G-buffer, one fullscreen lighting pass and forward translucents, drawn in ordered world pass stages
- `mcrs_minecraft_render_controller`: Render controllers of a client entity: the layers, their geometry, texture and visible bones in a frame
- `mcrs_minecraft_render_probe`: GPU pass timings, CPU stage timings, draw counters and GPU warm-up for the terrain renderer

## server

- `mcrs_minecraft_server`: The vanilla server plugin group MinecraftServerPlugins: it adds the engine, the vanilla gameplay plugins and the commands to an app
- `mcrs_minecraft_server_settings`: The settings of a server run that the MCRS_ environment variables set, read in one place

## text

- `mcrs_minecraft_text`: Text components: content, style, click and hover events, JSON and NBT codecs
- `mcrs_minecraft_text_bedrock`: A text component as a Bedrock formatted string or rawtext: dynamic parts resolved on the server, and the features Bedrock lacks counted
- `mcrs_minecraft_text_flatten`: A text component as flat runs of styled text: translations filled from a language, styles inherited, and the plain and the formatting-code strings made from the runs

## vanilla

- `mcrs_minecraft_vanilla_attributes`: Vanilla sources of attribute modifiers on the players of a dimension world: the sprint state and creative mode
- `mcrs_minecraft_vanilla_block_place`: Vanilla block placing in the dimension worlds: a block item used on a block face places its block
- `mcrs_minecraft_vanilla_death`: Vanilla consequences of the death of a player: the drop of its inventory and of the stacks it holds, and the death message
- `mcrs_minecraft_vanilla_falling_block`: The vanilla falling block: a requested fall moves with the motion pass and lands as a block or an item
- `mcrs_minecraft_vanilla_inventory`: The gameplay of the player inventory: the hand actions, the close, the drops and the player file
- `mcrs_minecraft_vanilla_item_entity`: The vanilla item entity: a dropped stack that moves with the motion pass, ages, merges and is picked up
- `mcrs_minecraft_vanilla_light`: Vanilla light wired into the dimension worlds: the block light table, the light engine driver of each dimension and the light updates sent to the players
- `mcrs_minecraft_vanilla_player_attack`: The melee attack of a player on an entity: the attack strength, the damage amount, the hit request from the attack packet and the knockback of the hit
- `mcrs_minecraft_vanilla_respawn`: Vanilla respawn of a dead player on its request, at the respawn position of the vanilla rules
- `mcrs_minecraft_vanilla_swing`: Vanilla arm swings in the dimension worlds: punch, stab, and attack_animation item components
- `mcrs_minecraft_vanilla_worldgen`: World generation wired into the dimension worlds: the routers, carvers, features and structures of each dimension, and the column scheduler that generates or reads its columns

## wire

- `mcrs_minecraft_wire`: Encode and decode traits, variable-length integers and primitive codecs for the Minecraft: Java Edition protocol
- `mcrs_minecraft_wire_macros`: Derive macros for the codec traits of mcrs_minecraft_wire

## worldgen

- `mcrs_minecraft_worldgen`: The world generator: the worldgen registries, their loaders and the beardifier
- `mcrs_minecraft_worldgen_builtin`: The worldgen registry entries the game and the beta preset define, built by code
- `mcrs_minecraft_worldgen_carver`: Cave and canyon carvers
- `mcrs_minecraft_worldgen_density`: Density functions, their compiler and evaluator, the noise router and the aquifer
- `mcrs_minecraft_worldgen_feature`: Configured and placed features: placement modifiers, tree placers, rule tests, template pools and structure templates; the generators are in worldgen_feature_place
- `mcrs_minecraft_worldgen_feature_place`: The feature generators: trees, ores, geodes, templates and the rest of what a placed feature writes
- `mcrs_minecraft_worldgen_generator`: The column pipeline of the noise chunk generator: fill, surface, carvers, features, structures and the read out of the save
- `mcrs_minecraft_worldgen_structure`: Structures, structure sets, template pools, jigsaw assembly and structure placement
- `mcrs_minecraft_worldgen_structure_place`: What the hardcoded structure pieces write: the piece canvas, the per-family painters and the after-place hooks
- `mcrs_minecraft_worldgen_surface`: Surface rules: the material rule compiler and evaluator
- `mcrs_minecraft_worldgen_testing`: The shipped asset corpus and the oracle dumps, read for tests

## Other

- `mcrs_minecraft_anvil`: Anvil region files and chunk records: RegionFile, parse_chunk, write_chunk, and the saved ticks, structure starts and retrogen data
- `mcrs_minecraft_assets`: Data pack loading: JSON loaders, the registry access and the load state
- `mcrs_minecraft_banner_pattern`: The banner pattern registry value, its keys and its tag keys
- `mcrs_minecraft_brain`: The AI stage of mobs without Bevy: the snapshot replica, the issue and the late application of decisions, keyed random values, the wake schedule and the goal selector
- `mcrs_minecraft_chunk`: Chunk storage: PalettedContainer, voxel palettes, section light storage and nibbles, column heights, and the chunk status keys
- `mcrs_minecraft_collision_table`: Builds the collision table of the physics kernel from the block definitions and the block tags
- `mcrs_minecraft_console`: The server console: command lines from the standard input, run as the server, and their answers in the log in English
- `mcrs_minecraft_container_click`: The content of a container click as a plain value: the click kind, the button and the clicked menu entry, with its wire codec
- `mcrs_minecraft_core`: Directions and axes, bounding boxes, voxel shapes, rotation and mirror, heightmap names, codec helpers such as is_default, and the game version
- `mcrs_minecraft_damage`: The hit procedure of a living entity without Bevy: the damage cooldown, the reductions, the absorption, the health change, the start of the death and the hit record
- `mcrs_minecraft_datapack`: The data pack stack: pack selection, pack.mcmeta, overlays, filters and the files each pack contributes
- `mcrs_minecraft_decorated_pot_pattern`: The decorated pot pattern registry value and its keys
- `mcrs_minecraft_enchantment`: The enchantment file and the keys of the enchantment registries
- `mcrs_minecraft_environment`: Environment attributes, timelines and world clocks
- `mcrs_minecraft_falling_block`: The rules of a falling block: which blocks fall, the start, the tick and the landing
- `mcrs_minecraft_fog`: The fog parameters of one frame of the client: the fog colour and the fog distances
- `mcrs_minecraft_goal_world`: The read-only block world of the goals of mobs: the ground cell of a block position, the build height, the line of sight and the straight route on walkable ground
- `mcrs_minecraft_goals`: The goals of passive mobs for the AI stage: float, panic, random stroll, look at a player and look around, the random position searches, the goal list of the pig and the brain of each mob type with goals
- `mcrs_minecraft_heightmap`: The four column heightmaps, the block state predicates they test, and the descent that builds them
- `mcrs_minecraft_keys`: Names of the vanilla registries no crate keys by a type, with their entries and tags
- `mcrs_minecraft_lang`: A language file of the game: translation patterns by key, and the filler of their placeholders
- `mcrs_minecraft_map_decoration`: The keys of the map decoration type registry
- `mcrs_minecraft_mesh`: Section meshing: greedy cube faces, fluids, block models, the packed quad layout and the buffer arena
- `mcrs_minecraft_mob_control`: The per-tick controls of a mob without Bevy: the path follower of a straight route, the move control, the jump control, the look control and the body rotation
- `mcrs_minecraft_network`: Protocol library for Minecraft: Java Edition
- `mcrs_minecraft_noise`: Perlin, simplex, normal and blended noise and the interval and grid types they sample over
- `mcrs_minecraft_numeric`: Bit-exact numeric operations: integer division, comparisons, interpolation, angle wrap, sine tables and the arctangent
- `mcrs_minecraft_pack_file`: Files of an entity pack: lenient JSON, the client entity file, load diagnostics and the compile of Molang fields
- `mcrs_minecraft_painting_variant`: The value of the painting variant registry
- `mcrs_minecraft_particle`: Particle types and their options as data packs and attributes write them
- `mcrs_minecraft_physics`: The entity physics kernel: boxes, block shapes, the collision algorithm, step-up and the move procedure
- `mcrs_minecraft_player_holders`: The holders of a player, their vanilla slot policy and the restore of their stacks
- `mcrs_minecraft_pose`: The pose of an entity with its wire codec, and the dimensions of a player in each pose
- `mcrs_minecraft_potion`: The keys of the potion registry and of its tags, and the effects of each potion
- `mcrs_minecraft_predicate`: The keys of the entity predicate and statistic registries, the views of the world and of the entities that a predicate reads, and the entity, location, block, fluid, light, distance and item predicates
- `mcrs_minecraft_profile`: Game profiles, their properties and the resolvable profile of a player head
- `mcrs_minecraft_random`: The Random trait, the legacy and Xoroshiro generators, position seeds, shuffle and the worldgen random splitters
- `mcrs_minecraft_recipe`: The ingredient and the keys of the recipe registries
- `mcrs_minecraft_region_disk`: The region disk layer: a column of the data layer as the anvil files under one dimension, with quarantine of damaged regions
- `mcrs_minecraft_registry_catalog`: The STATIC_REGISTRIES table: the entry locations of every static registry
- `mcrs_minecraft_replication`: The Minecraft side of entity replication: the muxer codec for players and mobs, mob trackers, delta bases for moves, spawn bundles, profile entries and the player list policy, as encoded packets
- `mcrs_minecraft_resource`: Resource locations, resource keys, tag keys, registry keys and registry ids
- `mcrs_minecraft_save`: The files of a world save other than its regions: the level, its world generation settings and each player's file
- `mcrs_minecraft_simulation`: Block simulation kernels: the step and update chain over a volume, their queues and ordering policies
- `mcrs_minecraft_snbt`: The read and the write of stringified NBT as the game reads and writes it
- `mcrs_minecraft_sound`: Sound events
- `mcrs_minecraft_static_registry`: The StaticEntry trait and the helpers that number the entries of a static registry by protocol id
- `mcrs_minecraft_swing`: Pure arm-swing cooldown window and status-effect duration scaling
- `mcrs_minecraft_trim`: The values of the trim material and trim pattern registries
- `mcrs_minecraft_value_provider`: Int, float and height providers and their codecs
- `mcrs_minecraft_volume`: One block access API over every store of world data: bounds, per-section access, section handles and cursors
- `mcrs_test_alloc`: A global allocator that counts the allocations of the calling thread, for tests
- `mcrs_voxel`: Owns the block, section, local and column positions and the block state id of a voxel world; the chunk position type is ColumnPos
