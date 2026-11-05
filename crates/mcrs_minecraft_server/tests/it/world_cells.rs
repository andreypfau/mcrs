//! A world divided into cells must look the same as the world of one cell: the blocks, the light
//! and the ticking markers of every column at its owner, where each player stands, and what each
//! client gets.
//!
//! In one scenario, the players stand on both sides of the boundary between two regions, place
//! blocks and light on both sides of it, and walk across it. In the other, a column at the
//! boundary changes and goes cold, and then a player of the other cell comes near it. Each run
//! waits after every tick for the generation, light and store steps, so the stages land at the
//! same tick in both runs.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

use bevy_app::{App, Last, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Has, ResMut, Resource, With};
use bevy_ecs::world::World;
use bevy_math::{DVec3, Vec2};
use bevy_tasks::block_on;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_chunk::{BlockLight, LightStorage, SkyLight};
use mcrs_minecraft_engine::block_simulation::{BlockSimulationPlugin, DimensionBlockRules};
use mcrs_minecraft_engine::bus::{
    InboundPlayerSpawn, OutboundPlayerPacket, PacketPayload, PlayerTransferSnapshot,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::muxer::column_view_link::{self, ColumnViewLink};
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::entity_replication::{self, Answers};
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_server::{Placement, WorldCells, runner};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::{
    Owner, Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{
    BlockTicking, EntityTicking, SimulationDistance,
};
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::chunk::{LightChunk, LightData};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundLevelChunkWithLight, ClientboundRemoveEntities,
};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, SimulationVolume, TickPriority,
};
use mcrs_minecraft_vanilla_light::{ColdLightLink, LightLink};
use mcrs_minecraft_vanilla_worldgen::WorldGeneration;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::configuration_registries::ASSETS;
use crate::host_app::{label_of, replicated, sent_to};

const OVERWORLD: &str = "minecraft:overworld";
const TICKS: usize = 90;
const COLD_TICKS: usize = 100;
const COLD_LIGHT_TICKS: usize = 220;
const PATIENCE: Duration = Duration::from_secs(120);

fn server(cells: u32) -> App {
    server_with(cells, DimensionBlockRules::default())
}

fn server_with(cells: u32, rules: DimensionBlockRules) -> App {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.core.worlds = Placement::Inline;
    plugins.core.muxer = Placement::Inline;
    plugins.core.cells = WorldCells {
        cells: NonZeroU32::new(cells).unwrap(),
        ..WorldCells::default()
    };
    plugins.network.announce_on_lan = false;
    plugins.worlds.seed = 4;
    let mut app = App::new();
    app.add_plugins(plugins.build());
    app.insert_resource(rules);
    app.finish();
    app.cleanup();
    crate::support::update_until_playing(&mut app);
    runner::between_ticks(&mut app);
    app
}

/// Ticks until the world of `label` can generate: its first cell holds the context that the
/// generation of the world runs with.
fn wait_for_generation(app: &mut App, label: Entity) {
    let deadline = Instant::now() + PATIENCE;
    while !cell_worlds(app, label)
        .into_iter()
        .next()
        .is_some_and(|world| world.contains_resource::<FillContext>())
    {
        assert!(
            Instant::now() < deadline,
            "the generation context never loaded"
        );
        tick(app, label);
    }
}

fn state(app: &App, name: &str) -> BlockStateId {
    app.world()
        .resource::<Blocks>()
        .block(name)
        .unwrap_or_else(|| panic!("{name} is declared"))
        .default_state_id
}

/// The world of every cell of `label`.
fn cell_worlds(app: &mut App, label: Entity) -> Vec<&mut World> {
    app.world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .cell_sub_apps_mut(label)
        .map(|sub_app| sub_app.world_mut())
        .collect()
}

/// Waits between ticks until the generation, light and store steps of every cell have ended.
fn wait_for_steps(app: &mut App, label: Entity) {
    block_on(app.world_mut().resource_mut::<DimCells>().stores_answered());
    if let Some(mut generation) = app.world_mut().get_resource_mut::<WorldGeneration>() {
        block_on(generation.answered());
    }
    for world in cell_worlds(app, label) {
        if let Some(mut light) = world.get_resource_mut::<LightLink>() {
            block_on(light.answered());
        }
        if let Some(mut light) = world.get_resource_mut::<ColdLightLink>() {
            block_on(light.answered());
        }
    }
}

/// Runs one tick and returns the packets it wrote for the clients.
fn tick(app: &mut App, label: Entity) -> Vec<OutboundPlayerPacket> {
    app.update();
    runner::between_ticks(app);
    column_view_link::deliver(app.world_mut(), Answers::All);
    entity_replication::deliver(app.world_mut(), Answers::All);
    let packets = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();
    wait_for_steps(app, label);
    packets
}

/// Places a session in `label` and sends the spawn of its player at `position`, with a client
/// that sees `view_distance` columns far.
fn admit(
    app: &mut App,
    label: Entity,
    name: &str,
    position: DVec3,
    view_distance: u8,
) -> PlayerSession {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    let uuid = Uuid::from_u128(u128::from(session.0));
    let anchor = app
        .world_mut()
        .spawn((
            GameProfile {
                id: uuid,
                username: name.into(),
                properties: Vec::new(),
            },
            SessionBundle::placed(session, SessionPlacement::new(Place::Joining(label), 0)),
        ))
        .id();
    app.world_mut().spawn(HostAnchorRef(anchor));
    let sent = app.world_mut().resource_mut::<DimInputs>().send_control(
        label,
        ToDim::Spawn(InboundPlayerSpawn {
            session,
            snapshot: PlayerTransferSnapshot {
                uuid,
                username: name.into(),
                position,
                rotation: Vec2::ZERO,
                client_info: Some(crate::support::client_info_with_view_distance(
                    view_distance,
                )),
            },
            dimensions: Vec::new().into(),
        }),
    );
    assert!(sent, "the dimension takes the spawn");
    session
}

/// The world that holds the player of `session`, and the player.
fn holder(app: &mut App, label: Entity, session: PlayerSession) -> Option<(usize, Entity)> {
    let mut found = None;
    for (index, world) in cell_worlds(app, label).into_iter().enumerate() {
        let player = world
            .query_filtered::<(Entity, &Owner), With<Player>>()
            .iter(world)
            .find(|(_, owner)| owner.0 == session)
            .map(|(player, _)| player);
        if let Some(player) = player {
            assert!(found.is_none(), "two cells hold the player of {session:?}");
            found = Some((index, player));
        }
    }
    found
}

fn dimension_of(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .expect("the dimension entity")
}

/// Writes a block as a player of the world of `actor` would: into the world that holds it,
/// whoever owns the column.
fn place(app: &mut App, label: Entity, actor: PlayerSession, pos: BlockPos, state: BlockStateId) {
    let (index, _) = holder(app, label, actor).expect("the actor is in the world");
    let mut worlds = cell_worlds(app, label);
    let world = worlds.swap_remove(index);
    let dimension = dimension_of(world);
    world
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos,
            new_state: state,
            flags: BlockUpdateFlags::ALL,
            recursion_left: 512,
        });
}

fn walk(app: &mut App, label: Entity, session: PlayerSession, by: DVec3) {
    let (index, player) = holder(app, label, session).expect("the walker is in the world");
    let mut worlds = cell_worlds(app, label);
    let world = worlds.swap_remove(index);
    world
        .get_mut::<Transform>(player)
        .expect("the walker stands somewhere")
        .translation += by;
}

/// What one client got over the run: what a client keeps of it, with no order between ticks.
#[derive(Debug, Default, PartialEq)]
struct Client {
    logins: usize,
    respawns: usize,
    loaded: BTreeSet<(i32, i32)>,
    forgotten: BTreeSet<(i32, i32)>,
    blocks: BTreeMap<(i32, i32, i32), u16>,
    /// The entity packets of the client, in order.
    entities: Vec<(i32, Vec<u8>)>,
    /// The light that the client holds of each section row of each column, as the columns and
    /// the light updates set it.
    light: BTreeMap<(i32, i32), BTreeMap<usize, RowLight>>,
}

/// The block light and the sky light of one section row of a column on the client, one value
/// for each block. An empty value is a layer that the client got no light for.
#[derive(Clone, Debug, Default, PartialEq)]
struct RowLight {
    block: Vec<u8>,
    sky: Vec<u8>,
}

/// One light layer of a `LightData` row by row: the values of a row that the data sets, and a
/// dark row for a row that the data marks empty.
fn light_rows(mask: &[u64], empty: &[u64], arrays: &[LightChunk]) -> BTreeMap<usize, Vec<u8>> {
    let bit = |words: &[u64], row: usize| {
        words
            .get(row / 64)
            .is_some_and(|word| word >> (row % 64) & 1 == 1)
    };
    let rows = (mask.len().max(empty.len())) * 64;
    let mut arrays = arrays.iter();
    let mut out = BTreeMap::new();
    for row in 0..rows {
        if bit(mask, row) {
            let array = arrays.next().expect("an array for each row of the mask");
            out.insert(
                row,
                array
                    .0
                    .iter()
                    .flat_map(|byte| [byte & 0x0F, byte >> 4])
                    .collect(),
            );
        } else if bit(empty, row) {
            out.insert(row, vec![0; SectionPos::VOLUME]);
        }
    }
    out
}

impl Client {
    fn take_light(&mut self, column: (i32, i32), data: &LightData<'_>, whole: bool) {
        let rows = self.light.entry(column).or_default();
        if whole {
            rows.clear();
        }
        for (row, values) in light_rows(
            &data.block_light_mask,
            &data.empty_block_light_mask,
            &data.block_light_arrays,
        ) {
            rows.entry(row).or_default().block = values;
        }
        for (row, values) in light_rows(
            &data.sky_light_mask,
            &data.empty_sky_light_mask,
            &data.sky_light_arrays,
        ) {
            rows.entry(row).or_default().sky = values;
        }
    }
}

impl Client {
    fn take(&mut self, packets: Vec<PacketPayload>) {
        for packet in &packets {
            match packet {
                PacketPayload::PlayerLogin(_) => self.logins += 1,
                PacketPayload::Respawn(_) => self.respawns += 1,
                PacketPayload::ChunkLoad { column, packet } => {
                    self.loaded.insert((column.x, column.z));
                    let mut bytes = &packet[..];
                    VarInt::decode(&mut bytes).expect("the packet id");
                    let chunk = ClientboundLevelChunkWithLight::decode(&mut bytes)
                        .expect("a column packet");
                    self.take_light((column.x, column.z), &chunk.light_data, true);
                }
                PacketPayload::LightUpdate(update) => {
                    self.take_light((update.x.0, update.z.0), &update.light_data, false);
                }
                PacketPayload::ChunkUnload(forget) => {
                    self.forgotten.insert((forget.x, forget.z));
                    self.light.remove(&(forget.x, forget.z));
                }
                PacketPayload::BlockUpdate(update) => {
                    let pos = update.block_pos;
                    self.blocks
                        .insert((pos.x, pos.y, pos.z), update.block_state_id.0);
                }
                _ => {}
            }
        }
        self.entities.extend(replicated(&packets));
    }

    fn count(&self, id: i32) -> usize {
        self.entities
            .iter()
            .filter(|(found, _)| *found == id)
            .count()
    }
}

/// The blocks and the light of one section, at the cell that owns its column.
#[derive(Debug, PartialEq)]
struct Section {
    stage: String,
    blocks: String,
    block_light: Vec<u8>,
    sky_light: Vec<u8>,
}

fn light(storage: Option<&LightStorage>) -> Vec<u8> {
    let mut field = [0u8; SectionPos::VOLUME];
    match storage {
        Some(storage) => {
            storage.write_field(&mut field);
            field.to_vec()
        }
        None => Vec::new(),
    }
}

/// The sections of `columns` at their owners.
fn owned_sections(
    app: &mut App,
    label: Entity,
    columns: &[ColumnPos],
) -> BTreeMap<(i32, i32, i32), Section> {
    let mut sections = BTreeMap::new();
    for world in cell_worlds(app, label) {
        let owned = world.get_resource::<OwnedColumns>().copied();
        let mut query = world.query::<(
            &SectionPos,
            &SectionStage,
            Option<&ChunkBlocks>,
            Option<&BlockLight>,
            Option<&SkyLight>,
        )>();
        for (pos, stage, blocks, block, sky) in query.iter(world) {
            let column = ColumnPos::from(*pos);
            if !columns.contains(&column) || owned.is_some_and(|owned| !owned.owns(column)) {
                continue;
            }
            let previous = sections.insert(
                (pos.x, pos.y, pos.z),
                Section {
                    stage: format!("{stage:?}"),
                    blocks: format!("{:?}", blocks.map(|blocks| &blocks.0)),
                    block_light: light(block.map(|light| &light.0)),
                    sky_light: light(sky.map(|light| &light.0)),
                },
            );
            assert!(previous.is_none(), "two cells own the section {pos:?}");
        }
    }
    sections
}

/// The ticking markers of one section, at the cell that owns its column.
#[derive(Debug, PartialEq)]
struct Ticking {
    block: bool,
    entity: bool,
}

/// The ticking markers of the loaded sections of `columns` at their owners. No cell marks a
/// section of its halo.
fn ticking_sections(
    app: &mut App,
    label: Entity,
    columns: &[ColumnPos],
) -> BTreeMap<(i32, i32, i32), Ticking> {
    let mut ticking = BTreeMap::new();
    for world in cell_worlds(app, label) {
        let owned = world.get_resource::<OwnedColumns>().copied();
        let mut query = world.query::<(&SectionPos, Has<BlockTicking>, Has<EntityTicking>)>();
        for (pos, block, entity) in query.iter(world) {
            let column = ColumnPos::from(*pos);
            if owned.is_some_and(|owned| !owned.owns(column)) {
                assert!(!block && !entity, "the section {pos:?} of the halo ticks");
                continue;
            }
            if columns.contains(&column) {
                ticking.insert((pos.x, pos.y, pos.z), Ticking { block, entity });
            }
        }
    }
    ticking
}

/// What a world looks like after a column of one cell at the boundary changed, went cold, and
/// a player of the other cell came near it.
struct ColdRun {
    sections: BTreeMap<(i32, i32, i32), Section>,
    ticking: BTreeMap<(i32, i32, i32), Ticking>,
}

fn run_cold_boundary(cells: u32) -> ColdRun {
    let mut app = server(cells);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let glowstone = state(&app, "minecraft:glowstone");

    // Region 0 starts at x = 0, and the light of a block at x = 0 reaches into region -1.
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    for at in 0..COLD_TICKS {
        match at {
            30 => place(&mut app, label, east, BlockPos::new(0, 92, 3), glowstone),
            // Far enough that no ticket loads the boundary, so its owner writes the column.
            35 => walk(&mut app, label, east, DVec3::new(320.0, 0.0, 0.0)),
            60 => {
                admit(&mut app, label, "west", DVec3::new(-8.5, 100.0, 8.5), 2);
            }
            _ => {}
        }
        tick(&mut app, label);
    }
    let columns: Vec<ColumnPos> = (-3..=2)
        .flat_map(|x| (-2..=2).map(move |z| ColumnPos::new(x, z)))
        .collect();
    ColdRun {
        sections: owned_sections(&mut app, label, &columns),
        ticking: ticking_sections(&mut app, label, &columns),
    }
}

/// The light of a column at a box edge as a client got it from the data layer, and as the
/// dimension lit it once it was hot.
struct ColdLightRun {
    client: BTreeMap<(i32, i32), BTreeMap<usize, RowLight>>,
    hot: BTreeMap<(i32, i32), BTreeMap<usize, RowLight>>,
}

/// The light of `columns` at their owners, by wire row: the rows of the sections, one up from
/// the row below the world.
fn owned_light(
    app: &mut App,
    label: Entity,
    columns: &[ColumnPos],
) -> BTreeMap<(i32, i32), BTreeMap<usize, RowLight>> {
    let min_y = cell_worlds(app, label)
        .into_iter()
        .next()
        .map(|world| {
            world
                .query::<&DimensionTypeConfig>()
                .iter(world)
                .next()
                .expect("the dimension type")
                .min_y
                >> SectionPos::BITS
        })
        .expect("a cell");
    let mut light = BTreeMap::new();
    for ((x, y, z), section) in owned_sections(app, label, columns) {
        let dark = |values: Vec<u8>| match values.is_empty() {
            true => vec![0; SectionPos::VOLUME],
            false => values,
        };
        let row = usize::try_from(y - min_y + 1).expect("a row in the world");
        light.entry((x, z)).or_insert_with(BTreeMap::new).insert(
            row,
            RowLight {
                block: dark(section.block_light),
                sky: dark(section.sky_light),
            },
        );
    }
    light
}

/// A viewer sees the columns at the box edge from the data layer while their neighbour across
/// the edge is hot in its owner, with changes that its owner never wrote: one before the data
/// layer lit the cold column, one after the client got it.
fn run_cold_light(cells: u32) -> ColdLightRun {
    let mut app = server(cells);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    for world in cell_worlds(&mut app, label) {
        world.insert_resource(SimulationDistance {
            horizontal: 1,
            vertical: u8::MAX,
        });
    }
    let glowstone = state(&app, "minecraft:glowstone");
    let edge = [ColumnPos::new(-1, 0), ColumnPos::new(0, 0)];

    // Column 0 is in the outer ring of the hot area around column 4: hot, and lit from the
    // data layer. Column -1 is cold and in another region.
    let east = admit(&mut app, label, "east", DVec3::new(72.5, 100.0, 8.5), 2);
    let mut viewer = None;
    let mut client = Client::default();
    let mut seen = BTreeMap::new();
    for at in 0..COLD_LIGHT_TICKS {
        match at {
            40 => place(&mut app, label, east, BlockPos::new(0, 100, 4), glowstone),
            45 => {
                viewer = Some(admit(
                    &mut app,
                    label,
                    "viewer",
                    DVec3::new(72.5, 100.0, 8.5),
                    6,
                ));
            }
            110 => place(&mut app, label, east, BlockPos::new(1, 101, 12), glowstone),
            150 => {
                seen = client.light.clone();
                admit(&mut app, label, "probe", DVec3::new(-8.5, 100.0, 8.5), 2);
            }
            _ => {}
        }
        if let Some(viewer) = viewer {
            app.world_mut()
                .resource_mut::<ColumnViewLink>()
                .acknowledge_batch(viewer, 64.0);
        }
        let packets = tick(&mut app, label);
        if let Some(viewer) = viewer {
            client.take(sent_to(viewer, packets));
        }
    }
    let hot = owned_light(&mut app, label, &edge);
    // The rows below and above the world carry no section to compare with.
    let client = seen
        .into_iter()
        .filter(|(column, _)| edge.contains(&ColumnPos::new(column.0, column.1)))
        .map(|(column, mut rows)| {
            rows.retain(|row, _| {
                hot.get(&column)
                    .is_some_and(|sections| sections.contains_key(row))
            });
            (column, rows)
        })
        .collect();
    ColdLightRun { client, hot }
}

struct Run {
    sections: BTreeMap<(i32, i32, i32), Section>,
    positions: Vec<DVec3>,
    /// The cell of each player at the start and at the end of the scenario.
    holders: Vec<[usize; 2]>,
    clients: Vec<Client>,
}

fn run(cells: u32) -> Run {
    let mut app = server(cells);
    let label = label_of(&app, OVERWORLD);
    assert_eq!(
        app.world().resource::<DimCells>().cell_count(label),
        cells as usize
    );
    wait_for_generation(&mut app, label);
    let glowstone = state(&app, "minecraft:glowstone");
    let stone = state(&app, "minecraft:stone");

    // Region -1 and region 0 meet at x = 0: one cell owns each side.
    let west = admit(&mut app, label, "west", DVec3::new(-8.5, 100.0, 8.5), 2);
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    let sessions = [west, east];
    let mut clients = [Client::default(), Client::default()];
    let mut first_holders = Vec::new();
    for at in 0..TICKS {
        if at == 30 {
            for session in sessions {
                let (index, _) = holder(&mut app, label, session).expect("the player joined");
                first_holders.push(index);
            }
        }
        match at {
            30 => {
                place(&mut app, label, west, BlockPos::new(0, 90, 3), glowstone);
                place(&mut app, label, west, BlockPos::new(-1, 91, 6), glowstone);
                place(&mut app, label, east, BlockPos::new(-1, 95, 8), stone);
                place(&mut app, label, east, BlockPos::new(1, 95, 8), glowstone);
            }
            34 => walk(&mut app, label, west, DVec3::new(24.0, 0.0, 0.0)),
            40 => walk(&mut app, label, east, DVec3::new(-24.0, 0.0, 0.0)),
            45 => {
                place(
                    &mut app,
                    label,
                    east,
                    BlockPos::new(0, 90, 3),
                    BlockStateId::AIR,
                );
                place(&mut app, label, west, BlockPos::new(-2, 92, 1), glowstone);
            }
            _ => {}
        }
        let packets = tick(&mut app, label);
        for (client, session) in clients.iter_mut().zip(sessions) {
            client.take(sent_to(session, packets.clone()));
        }
    }
    let columns: Vec<ColumnPos> = (-3..=2)
        .flat_map(|x| (-2..=2).map(move |z| ColumnPos::new(x, z)))
        .collect();
    let sections = owned_sections(&mut app, label, &columns);
    let holders = sessions
        .iter()
        .zip(first_holders)
        .map(|(&session, first)| {
            let (index, _) = holder(&mut app, label, session).expect("the player stays");
            [first, index]
        })
        .collect();
    let positions = sessions
        .iter()
        .map(|&session| {
            let (index, player) = holder(&mut app, label, session).expect("the player stays");
            let mut worlds = cell_worlds(&mut app, label);
            worlds
                .swap_remove(index)
                .get::<Transform>(player)
                .expect("the player stands somewhere")
                .translation
        })
        .collect();
    Run {
        sections,
        positions,
        holders,
        clients: clients.into(),
    }
}

#[test]
fn a_world_of_two_cells_is_the_world_of_one_cell() {
    let one = run(1);
    let two = run(2);

    assert_eq!(
        one.sections.len(),
        5 * 6 * 24,
        "every column of the check landed"
    );
    for (pos, section) in &one.sections {
        assert_eq!(
            two.sections.get(pos),
            Some(section),
            "the section {pos:?} differs at its owner"
        );
    }
    assert_eq!(one.positions, two.positions);
    assert_eq!(
        two.holders,
        [[1, 0], [0, 1]],
        "each player starts in the cell of its region and walks into the other"
    );
    for (index, (one, two)) in one.clients.iter().zip(&two.clients).enumerate() {
        assert_eq!(one.logins, 1, "client {index} logs in once");
        assert_eq!(
            two.respawns, 0,
            "client {index} sees no change of dimension"
        );
        let add = ClientboundAddEntity::ID;
        let remove = ClientboundRemoveEntities::ID;
        assert!(one.count(add) > 0, "client {index} sees the other player");
        assert_eq!(
            (two.count(add), two.count(remove)),
            (one.count(add), one.count(remove)),
            "client {index} sees the other player spawn or despawn more often"
        );
        assert_eq!(
            one.entities, two.entities,
            "client {index} gets other entity bytes"
        );
        assert_eq!(one.loaded, two.loaded, "client {index} gets other columns");
        assert_eq!(
            one.forgotten, two.forgotten,
            "client {index} forgets other columns"
        );
        assert_eq!(one.blocks, two.blocks, "client {index} sees other blocks");
        assert_eq!(one, two);
    }
}

#[test]
fn a_cold_column_at_the_boundary_comes_back_in_the_other_cell_as_in_one_cell() {
    let one = run_cold_boundary(1);
    let two = run_cold_boundary(2);

    assert_eq!(
        one.sections.len(),
        5 * 6 * 24,
        "every column of the check landed"
    );
    for (pos, section) in &one.sections {
        assert_eq!(
            two.sections.get(pos),
            Some(section),
            "the section {pos:?} differs at its owner"
        );
    }
    assert!(
        one.ticking.values().any(|ticking| ticking.entity),
        "the player near the boundary ticks the sections around it"
    );
    for (pos, ticking) in &one.ticking {
        assert_eq!(
            two.ticking.get(pos),
            Some(ticking),
            "the section {pos:?} ticks otherwise at its owner"
        );
    }
}

const RELAY_DELAY: usize = 2;
const SLOW_DELAY: usize = 60;

/// Probe rules, not the rules of any game block. An unlit redstone lamp next to a lit one or to a
/// redstone block lights [`RELAY_DELAY`] ticks after a neighbour changed, so a line of lamps
/// lights one lamp after the other. A note block that is placed turns into a lit lamp
/// [`SLOW_DELAY`] ticks later.
#[derive(Clone, Copy, Debug)]
struct Relay {
    unlit: BlockStateId,
    lit: BlockStateId,
    source: BlockStateId,
    slow: BlockStateId,
}

impl Relay {
    fn install(sub_app: &mut bevy_app::SubApp) {
        let blocks = sub_app.world().resource::<Blocks>();
        let lamp = blocks.block("minecraft:redstone_lamp").expect("a lamp");
        let relay = Relay {
            unlit: lamp.default_state_id,
            lit: lamp
                .with(lamp.default_state_id, "lit", &PropertyValue::Bool(true))
                .expect("a lit lamp"),
            source: blocks
                .default_state("minecraft:redstone_block")
                .expect("a redstone block"),
            slow: blocks
                .default_state("minecraft:note_block")
                .expect("a note block"),
        };
        sub_app.add_plugins(BlockSimulationPlugin::new(relay));
    }
}

impl BlockBehaviour<WorldKinds> for Relay {
    fn block_tick<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
    ) {
        if state == self.unlit || state == self.slow {
            cx.set(at, self.lit, BlockUpdateFlags::ALL, 512);
        }
    }

    fn placed<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        new: BlockStateId,
        _old: BlockStateId,
        _moved_by_piston: bool,
    ) {
        if new == self.slow {
            let delay = SLOW_DELAY as i32;
            cx.schedule_block_tick(at, Block::NoteBlock.id(), delay, TickPriority::Normal);
        }
    }

    fn neighbour_changed<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        state: BlockStateId,
        _source: Id<Block>,
        _moved_by_piston: bool,
    ) {
        let lamp = Block::RedstoneLamp.id();
        if state != self.unlit || cx.has_scheduled_block_tick(at, lamp) {
            return;
        }
        let neighbours = [
            BlockPos::new(at.x - 1, at.y, at.z),
            BlockPos::new(at.x + 1, at.y, at.z),
            BlockPos::new(at.x, at.y - 1, at.z),
            BlockPos::new(at.x, at.y + 1, at.z),
            BlockPos::new(at.x, at.y, at.z - 1),
            BlockPos::new(at.x, at.y, at.z + 1),
        ];
        let powered = neighbours.into_iter().any(|pos| {
            cx.state(pos)
                .is_ok_and(|state| state == self.lit || state == self.source)
        });
        if powered {
            let delay = RELAY_DELAY as i32;
            cx.schedule_block_tick(at, lamp, delay, TickPriority::Normal);
        }
    }
}

fn lit_lamp(app: &App) -> BlockStateId {
    let blocks = app.world().resource::<Blocks>();
    let lamp = blocks.block("minecraft:redstone_lamp").expect("a lamp");
    lamp.with(lamp.default_state_id, "lit", &PropertyValue::Bool(true))
        .expect("a lit lamp")
}

fn relay_server(cells: u32) -> App {
    server_with(cells, DimensionBlockRules::new(Relay::install))
}

/// The block at `pos` at the cell that owns its column, while that cell holds it.
fn block_at(app: &mut App, label: Entity, pos: BlockPos) -> Option<BlockStateId> {
    let section = SectionPos::from(pos);
    for world in cell_worlds(app, label) {
        let owned = world.get_resource::<OwnedColumns>().copied();
        if owned.is_some_and(|owned| !owned.owns_block(pos)) {
            continue;
        }
        let mut query = world.query::<(&SectionPos, &ChunkBlocks)>();
        if let Some((_, blocks)) = query.iter(world).find(|(at, _)| **at == section) {
            return Some(blocks.get(LocalPos::from(pos)));
        }
    }
    None
}

/// The lamps of the line run from x = -6 to x = 5, across the boundary at x = 0.
const LINE: RangeInclusive<i32> = -6..=5;

fn lamp(x: i32) -> BlockPos {
    BlockPos::new(x, 120, 5)
}

/// The tick at which each lamp of the line lit, and the sections of the line at their owners.
struct LineRun {
    lit_at: BTreeMap<i32, usize>,
    sections: BTreeMap<(i32, i32, i32), Section>,
}

fn run_relay_line(cells: u32) -> LineRun {
    let mut app = relay_server(cells);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let unlit = state(&app, "minecraft:redstone_lamp");
    let lit = lit_lamp(&app);
    let source = state(&app, "minecraft:redstone_block");

    let west = admit(&mut app, label, "west", DVec3::new(-8.5, 100.0, 8.5), 2);
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    let mut lit_at = BTreeMap::new();
    for at in 0..TICKS {
        match at {
            25 => {
                for x in LINE {
                    let owner = if x < 0 { west } else { east };
                    place(&mut app, label, owner, lamp(x), unlit);
                }
            }
            30 => place(&mut app, label, west, lamp(LINE.start() - 1), source),
            _ => {}
        }
        tick(&mut app, label);
        for x in LINE {
            if block_at(&mut app, label, lamp(x)) == Some(lit) {
                lit_at.entry(x).or_insert(at);
            }
        }
    }
    let columns: Vec<ColumnPos> = LINE.map(|x| ColumnPos::from(lamp(x))).collect();
    LineRun {
        lit_at,
        sections: owned_sections(&mut app, label, &columns),
    }
}

/// A line of relays across the boundary lights in a world of two cells as in a world of one
/// cell. The first relay past the boundary hears of its neighbour one tick late, so it and every
/// relay after it light one tick later.
#[test]
fn a_relay_line_lights_across_the_boundary_one_tick_late() {
    let one = run_relay_line(1);
    let two = run_relay_line(2);

    let first = *one
        .lit_at
        .get(LINE.start())
        .expect("the first relay lights in one cell");
    for x in LINE {
        let steps = usize::try_from(x - LINE.start()).expect("the line runs east");
        let one_at = first + RELAY_DELAY * steps;
        assert_eq!(
            one.lit_at.get(&x),
            Some(&one_at),
            "in one cell, the relay at x = {x} lights {RELAY_DELAY} ticks after the one before it"
        );
        let delay = usize::from(x >= 0);
        assert_eq!(
            two.lit_at.get(&x),
            Some(&(one_at + delay)),
            "in two cells, the relay at x = {x} lights {delay} ticks later"
        );
    }
    assert!(!one.sections.is_empty(), "the sections of the line landed");
    for (pos, section) in &one.sections {
        assert_eq!(
            two.sections.get(pos),
            Some(section),
            "the section {pos:?} differs at its owner"
        );
    }
}

/// When a scheduled tick ran, and when its column went cold and ticked again.
#[derive(Debug, Default)]
struct SlowRun {
    cooled_at: Option<usize>,
    ticking_again_at: Option<usize>,
    ran_at: Option<usize>,
}

fn run_slow_tick(cells: u32) -> SlowRun {
    let mut app = relay_server(cells);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let slow = state(&app, "minecraft:note_block");
    let lit = lit_lamp(&app);
    let pos = BlockPos::new(0, 120, 3);
    let section = SectionPos::from(pos);

    // The column of x = 0 is in region 0: the cell of the player owns it in both worlds.
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    let mut run = SlowRun::default();
    for at in 0..220 {
        match at {
            30 => place(&mut app, label, east, pos, slow),
            // Far enough that no ticket loads the column; the tick has 55 ticks left then.
            35 => walk(&mut app, label, east, DVec3::new(320.0, 0.0, 0.0)),
            // Later than the tick was due, so a due time that counts the cold ticks shows.
            120 => walk(&mut app, label, east, DVec3::new(-320.0, 0.0, 0.0)),
            _ => {}
        }
        tick(&mut app, label);
        let held = block_at(&mut app, label, pos);
        let ticking = ticking_sections(&mut app, label, &[ColumnPos::from(pos)])
            .get(&(section.x, section.y, section.z))
            .is_some_and(|ticking| ticking.block);
        if at > 35 && held.is_none() {
            run.cooled_at.get_or_insert(at);
        }
        if run.cooled_at.is_some() && ticking && held.is_some() {
            run.ticking_again_at.get_or_insert(at);
        }
        if held == Some(lit) {
            run.ran_at.get_or_insert(at);
        }
    }
    run
}

/// A scheduled tick of a column that goes cold runs after the column ticks again, with the delay
/// that it had left when the column went cold: the cold ticks do not count.
#[test]
fn a_scheduled_tick_survives_a_cold_column_with_its_remaining_delay() {
    for cells in [1, 2] {
        let run = run_slow_tick(cells);
        let cooled_at = run.cooled_at.expect("the column goes cold");
        let back = run.ticking_again_at.expect("the column ticks again");
        let left = 30 + SLOW_DELAY - cooled_at;
        let ran_at = run.ran_at.unwrap_or_else(|| {
            panic!("the tick of the cold column never runs in {cells} cells: {run:?}")
        });
        assert!(
            ran_at.abs_diff(back + left) <= 1,
            "the tick runs {left} ticks after the column ticks again in {cells} cells: {run:?}"
        );
    }
}

type ColumnLight = BTreeMap<(i32, i32), BTreeMap<usize, RowLight>>;

/// The blocks whose light differs between `left` and `right`, by column, row and layer.
fn light_differences(left: &ColumnLight, right: &ColumnLight) -> Vec<String> {
    let mut differences = Vec::new();
    let columns: BTreeSet<&(i32, i32)> = left.keys().chain(right.keys()).collect();
    for column in columns {
        let (Some(left), Some(right)) = (left.get(column), right.get(column)) else {
            differences.push(format!("{column:?} is on one side only"));
            continue;
        };
        let rows: BTreeSet<&usize> = left.keys().chain(right.keys()).collect();
        for row in rows {
            let empty = RowLight::default();
            let (left, right) = (
                left.get(row).unwrap_or(&empty),
                right.get(row).unwrap_or(&empty),
            );
            for (layer, left, right) in [
                ("block", &left.block, &right.block),
                ("sky", &left.sky, &right.sky),
            ] {
                let differ = left.len().max(right.len())
                    - left.iter().zip(right).filter(|(l, r)| l == r).count();
                if differ > 0 {
                    differences.push(format!("{column:?} row {row} {layer}: {differ} blocks"));
                }
            }
        }
    }
    differences
}

#[test]
fn a_cold_column_at_a_box_edge_has_the_light_of_one_cell_and_of_the_dimension() {
    let one = run_cold_light(1);
    let two = run_cold_light(2);

    assert_eq!(
        one.client.len(),
        2,
        "the client got both columns of the edge"
    );
    assert_eq!(
        light_differences(&one.client, &one.hot),
        Vec::<String>::new(),
        "the client of one cell holds other light than the dimension"
    );
    assert_eq!(
        light_differences(&one.client, &two.client),
        Vec::<String>::new(),
        "the client of two cells holds other light"
    );
    assert_eq!(
        light_differences(&one.hot, &two.hot),
        Vec::<String>::new(),
        "two cells light the edge otherwise"
    );
}

#[derive(Resource, Default)]
struct LastRuns(usize);

fn count_last_runs(mut runs: ResMut<LastRuns>) {
    runs.0 += 1;
}

/// How often the schedules of each cell of `label` ran so far.
fn runs(app: &mut App, label: Entity) -> Vec<usize> {
    cell_worlds(app, label)
        .into_iter()
        .map(|world| world.resource::<LastRuns>().0)
        .collect()
}

#[test]
fn a_cell_with_no_player_and_no_remote_ticket_skips_its_schedules() {
    let mut app = server(4);
    let label = label_of(&app, OVERWORLD);
    for sub_app in app
        .world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .cell_sub_apps_mut(label)
    {
        sub_app.init_resource::<LastRuns>();
        sub_app.add_systems(Last, count_last_runs);
    }
    tick(&mut app, label);
    let settled = runs(&mut app, label);
    for _ in 0..10 {
        tick(&mut app, label);
    }
    assert_eq!(
        runs(&mut app, label),
        settled,
        "an idle cell ran its schedules"
    );

    admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    for _ in 0..10 {
        tick(&mut app, label);
    }
    assert!(
        runs(&mut app, label)
            .iter()
            .zip(&settled)
            .all(|(runs, idle)| *runs >= idle + 8),
        "every cell ticks the player or its remote ticket: {:?}",
        runs(&mut app, label)
    );
}
