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
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

use bevy_app::{App, FixedUpdate, Last};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Has, Query, ResMut, Resource, With};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_math::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_chunk::{BlockLight, LightStorage, SkyLight};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine::ability::{PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::block_simulation::{BlockSimulationPlugin, DimensionBlockRules};
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::coordinator::ops::DefaultOpLevel;
use mcrs_minecraft_engine::muxer::muxer_link::MuxerLink;
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_world::block_drops::{BlockDrops, DroppingBlock, ItemDropped};
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{
    BlockTicking, EntityTicking, SimulationDistance,
};
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item::{ItemHoverMap, ItemStack};
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, Kind};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::chunk::{LightChunk, LightData};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundBlockEvent, ClientboundLevelChunkWithLight,
};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{ServerboundChatCommand, ServerboundPlayerAction};
use mcrs_minecraft_protocol_game::text::Text;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, RandomTicks, SimulationVolume, TickPriority,
};
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, LocalPos, SectionPos};

use crate::host_app::{self, Arrival, PlayServer, label_of, replicated, sent_to};
use crate::world_server::{
    Holder, OVERWORLD, WorldServer, block_state, cell_worlds, holder, lockstep_tick, place,
    position, wait_for_generation, walk,
};

const TICKS: usize = 90;
/// The sections of a column of the beta overworld, which is 128 blocks high.
const SECTIONS: usize = 8;
const COLD_TICKS: usize = 100;
const COLD_LIGHT_TICKS: usize = 220;

/// Places a session in `label` and sends the spawn of its player at `position`, with a client
/// that sees `view_distance` columns far.
fn admit(
    app: &mut App,
    label: Entity,
    name: &str,
    position: DVec3,
    view_distance: u8,
) -> PlayerSession {
    // The runs compare the entity bytes of their clients, which carry the uuid of each player.
    let uuid = Uuid::from_u128(name.bytes().fold(0, |id, byte| id << 8 | u128::from(byte)));
    let arrival = Arrival {
        uuid,
        name,
        position,
        view_distance,
    };
    let anchor = host_app::admit(app, label, &arrival);
    host_app::session_of(app, anchor).expect("a placed session")
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
                PacketPayload::SectionBlocksUpdate(update) => {
                    let origin = update.chunk_pos;
                    for entry in update.blocks.iter() {
                        let pos = (
                            origin.x * 16 + i32::from(entry.off_x()),
                            origin.y * 16 + i32::from(entry.off_y()),
                            origin.z * 16 + i32::from(entry.off_z()),
                        );
                        self.blocks.insert(pos, entry.block_state());
                    }
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
            &InDimension,
            &SectionStage,
            Option<&BlockLight>,
            Option<&SkyLight>,
        )>();
        for (pos, &InDimension(dim), stage, block, sky) in query.iter(world) {
            let column = ColumnPos::from(*pos);
            if !columns.contains(&column) || owned.is_some_and(|owned| !owned.owns(column)) {
                continue;
            }
            let blocks = world
                .get::<SectionIndex>(dim)
                .and_then(|index| index.blocks(*pos));
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

fn server(cells: u32) -> App {
    WorldServer {
        cells,
        ..WorldServer::default()
    }
    .start()
}

fn server_with(cells: u32, rules: DimensionBlockRules) -> App {
    WorldServer {
        cells,
        rules,
        ..WorldServer::default()
    }
    .start()
}

fn run_cold_boundary(cells: u32) -> ColdRun {
    let mut app = server(cells);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let glowstone = block_state(&app, "minecraft:glowstone");

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
        lockstep_tick(&mut app, label);
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
    let glowstone = block_state(&app, "minecraft:glowstone");
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
                .resource_mut::<MuxerLink>()
                .acknowledge_batch(viewer, 64.0);
        }
        let packets = lockstep_tick(&mut app, label);
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
    let glowstone = block_state(&app, "minecraft:glowstone");
    let stone = block_state(&app, "minecraft:stone");

    // Region -1 and region 0 meet at x = 0: one cell owns each side.
    let west = admit(&mut app, label, "west", DVec3::new(-8.5, 100.0, 8.5), 2);
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    let sessions = [west, east];
    let mut clients = [Client::default(), Client::default()];
    let mut first_holders = Vec::new();
    for at in 0..TICKS {
        if at == 30 {
            for session in sessions {
                let Holder { cell, .. } =
                    holder(&mut app, label, session).expect("the player joined");
                first_holders.push(cell);
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
        let packets = lockstep_tick(&mut app, label);
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
            let Holder { cell, .. } = holder(&mut app, label, session).expect("the player stays");
            [first, cell]
        })
        .collect();
    let positions = sessions
        .iter()
        .map(|&session| position(&mut app, label, session))
        .collect();
    Run {
        sections,
        positions,
        holders,
        clients: clients.into(),
    }
}

/// Every section of `one` equals the same section of `two`.
fn assert_same_sections(
    one: &BTreeMap<(i32, i32, i32), Section>,
    two: &BTreeMap<(i32, i32, i32), Section>,
) {
    for (pos, section) in one {
        assert_eq!(
            two.get(pos),
            Some(section),
            "the section {pos:?} differs at its owner"
        );
    }
}

#[test]
fn a_world_of_two_cells_is_the_world_of_one_cell() {
    let one = run(1);
    let two = run(2);

    assert_eq!(
        one.sections.len(),
        5 * 6 * SECTIONS,
        "every column of the check landed"
    );
    assert_same_sections(&one.sections, &two.sections);
    assert_eq!(one.positions, two.positions);
    assert_eq!(
        two.holders,
        [[1, 0], [0, 1]],
        "each player starts in the cell of its region and walks into the other"
    );
    for (index, (one, two)) in one.clients.iter().zip(&two.clients).enumerate() {
        assert_eq!(one.logins, 1, "client {index} logs in once");
        assert_eq!(
            one.respawns, 0,
            "client {index} sees no change of dimension"
        );
        assert!(
            one.count(ClientboundAddEntity::ID) > 0,
            "client {index} sees the other player"
        );
        assert_eq!(one, two, "client {index} gets another world in two cells");
    }
}

#[test]
fn a_cold_column_at_the_boundary_comes_back_in_the_other_cell_as_in_one_cell() {
    let one = run_cold_boundary(1);
    let two = run_cold_boundary(2);

    assert_eq!(
        one.sections.len(),
        5 * 6 * SECTIONS,
        "every column of the check landed"
    );
    assert_same_sections(&one.sections, &two.sections);
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

impl BlockDrops for Relay {}

fn lit_lamp(app: &App) -> BlockStateId {
    lit_lamp_in(app.world().resource::<Blocks>())
}

fn relay_server(cells: u32) -> App {
    server_with(cells, DimensionBlockRules(Relay::install))
}

/// The block at `pos` at the cell that owns its column, while that cell holds it.
fn block_at(app: &mut App, label: Entity, pos: BlockPos) -> Option<BlockStateId> {
    let section = SectionPos::from(pos);
    for world in cell_worlds(app, label) {
        let owned = world.get_resource::<OwnedColumns>().copied();
        if owned.is_some_and(|owned| !owned.owns(ColumnPos::from(pos))) {
            continue;
        }
        let mut query = world.query::<&SectionIndex>();
        if let Some(blocks) = query.iter(world).find_map(|index| index.blocks(section)) {
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
    let unlit = block_state(&app, "minecraft:redstone_lamp");
    let lit = lit_lamp(&app);
    let source = block_state(&app, "minecraft:redstone_block");

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
        lockstep_tick(&mut app, label);
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
    assert_same_sections(&one.sections, &two.sections);
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
    let slow = block_state(&app, "minecraft:note_block");
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
        lockstep_tick(&mut app, label);
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
    lockstep_tick(&mut app, label);
    let settled = runs(&mut app, label);
    for _ in 0..10 {
        lockstep_tick(&mut app, label);
    }
    assert_eq!(
        runs(&mut app, label),
        settled,
        "an idle cell ran its schedules"
    );

    admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    for _ in 0..10 {
        lockstep_tick(&mut app, label);
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

/// Probe rules, not the rules of any game block: a note block that is placed queues a block
/// event, which it takes.
#[derive(Clone, Copy, Debug)]
struct Chime {
    note: BlockStateId,
}

const CHIME_ACTION: u8 = 1;
const CHIME_PARAM: u8 = 2;
const CHIME: BlockPos = BlockPos::new(-3, 120, 5);

impl Chime {
    fn install(sub_app: &mut bevy_app::SubApp) {
        let note = sub_app
            .world()
            .resource::<Blocks>()
            .default_state("minecraft:note_block")
            .expect("a note block");
        sub_app.add_plugins(BlockSimulationPlugin::new(Chime { note }));
    }
}

impl BlockBehaviour<WorldKinds> for Chime {
    fn placed<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        new: BlockStateId,
        _old: BlockStateId,
        _moved_by_piston: bool,
    ) {
        if new == self.note {
            let (action, param) = (i32::from(CHIME_ACTION), i32::from(CHIME_PARAM));
            cx.queue_block_event(at, Block::NoteBlock.id(), action, param);
        }
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        _cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
        _a: i32,
        _b: i32,
    ) -> bool {
        true
    }
}

impl BlockDrops for Chime {}

/// The block events that each client got after the note block was placed, and the columns that
/// the far client held.
struct ChimeRun {
    near: Vec<ClientboundBlockEvent>,
    far: Vec<ClientboundBlockEvent>,
    far_loaded: BTreeSet<(i32, i32)>,
}

fn block_events(packets: Vec<PacketPayload>) -> Vec<ClientboundBlockEvent> {
    packets
        .into_iter()
        .filter_map(|packet| match packet {
            PacketPayload::BlockEvent(event) => Some(event),
            _ => None,
        })
        .collect()
}

/// In two cells, the note block is in the west cell. Both clients stand in the east cell: the
/// near one holds the column of the note block, the far one does not.
fn run_chime() -> ChimeRun {
    let mut app = server_with(2, DimensionBlockRules(Chime::install));
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let note = block_state(&app, "minecraft:note_block");
    let near = admit(&mut app, label, "near", DVec3::new(8.5, 100.0, 8.5), 2);
    let far = admit(&mut app, label, "far", DVec3::new(200.5, 100.0, 8.5), 2);
    let column = ColumnPos::from(CHIME);
    let mut near_client = Client::default();
    let mut far_client = Client::default();
    let deadline = Instant::now() + Duration::from_secs(120);
    while !near_client.loaded.contains(&(column.x, column.z)) {
        assert!(
            Instant::now() < deadline,
            "the near client never got the column"
        );
        for session in [near, far] {
            app.world_mut()
                .resource_mut::<MuxerLink>()
                .acknowledge_batch(session, 64.0);
        }
        let packets = lockstep_tick(&mut app, label);
        near_client.take(sent_to(near, packets.clone()));
        far_client.take(sent_to(far, packets));
    }

    place(&mut app, label, near, CHIME, note);
    let mut near_events = Vec::new();
    let mut far_events = Vec::new();
    for _ in 0..5 {
        let packets = lockstep_tick(&mut app, label);
        near_events.extend(block_events(sent_to(near, packets.clone())));
        far_client.take(sent_to(far, packets.clone()));
        far_events.extend(block_events(sent_to(far, packets)));
    }
    ChimeRun {
        near: near_events,
        far: far_events,
        far_loaded: far_client.loaded,
    }
}

#[test]
fn only_a_client_that_holds_the_column_gets_the_block_event_of_another_cell() {
    let run = run_chime();
    let event = ClientboundBlockEvent {
        pos: CHIME,
        action: CHIME_ACTION,
        param: CHIME_PARAM,
        block: VarInt(i32::from(Block::NoteBlock.id().number())),
    };
    assert_eq!(run.near, [event]);

    let column = ColumnPos::from(CHIME);
    assert!(
        !run.far_loaded.is_empty() && !run.far_loaded.contains(&(column.x, column.z)),
        "the far client holds other columns only: {:?}",
        run.far_loaded
    );
    assert_eq!(run.far, []);
}

/// Probe rules, not the rules of any game block: a random tick turns a note block into a lit
/// lamp.
#[derive(Clone, Copy, Debug)]
struct Sprout {
    note: BlockStateId,
    lit: BlockStateId,
}

impl Sprout {
    fn install(sub_app: &mut bevy_app::SubApp) {
        let blocks = sub_app.world().resource::<Blocks>();
        let lamp = blocks.block("minecraft:redstone_lamp").expect("a lamp");
        let sprout = Sprout {
            note: blocks
                .default_state("minecraft:note_block")
                .expect("a note block"),
            lit: lamp
                .with(lamp.default_state_id, "lit", &PropertyValue::Bool(true))
                .expect("a lit lamp"),
        };
        sub_app.add_plugins(BlockSimulationPlugin::new(sprout));
    }
}

impl BlockBehaviour<WorldKinds> for Sprout {
    fn random_ticks(&self, state: BlockStateId) -> RandomTicks {
        if state == self.note {
            RandomTicks::BLOCK
        } else {
            RandomTicks::empty()
        }
    }

    fn random_tick<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        _state: BlockStateId,
        _random: &mut LegacyRandom,
    ) {
        cx.set(at, self.lit, BlockUpdateFlags::CLIENTS, 512);
    }
}

impl BlockDrops for Sprout {}

/// A layer of note blocks on each side of the boundary, and the blocks of each layer that a random
/// tick reached.
fn run_sprouts(cells: u32) -> BTreeSet<(i32, i32)> {
    let mut app = server_with(cells, DimensionBlockRules(Sprout::install));
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let note = block_state(&app, "minecraft:note_block");
    let lit = lit_lamp(&app);
    let west = admit(&mut app, label, "west", DVec3::new(-8.5, 100.0, 8.5), 2);
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    let layer = |x: i32, z: i32| BlockPos::new(x, 120, z);
    let mut reached = BTreeSet::new();
    for at in 0..TICKS {
        if at == 25 {
            for x in -16..16 {
                let owner = if x < 0 { west } else { east };
                for z in 0..16 {
                    place(&mut app, label, owner, layer(x, z), note);
                }
            }
        }
        lockstep_tick(&mut app, label);
    }
    for x in -16..16 {
        for z in 0..16 {
            if block_at(&mut app, label, layer(x, z)) == Some(lit) {
                reached.insert((x, z));
            }
        }
    }
    reached
}

/// The random ticks of the simulation area reach the blocks that take them on both sides of the
/// boundary, in a world of one cell and in a world of two cells. Each side takes about
/// 3 / 16 of a block a tick.
#[test]
fn random_ticks_reach_the_blocks_of_the_simulation_area_in_every_cell() {
    for cells in [1, 2] {
        let reached = run_sprouts(cells);
        let west = reached.iter().filter(|(x, _)| *x < 0).count();
        let east = reached.len() - west;
        assert!(
            west > 0 && east > 0 && reached.len() <= 2 * 3 * (TICKS - 25),
            "in {cells} cells, random ticks reached {west} blocks in the west and {east} in the east"
        );
    }
}

/// Probe rules, not the rules of any game block: a shape update breaks a note block, which
/// drops one note block item.
#[derive(Clone, Copy, Debug)]
struct Fragile {
    note: BlockStateId,
}

impl Fragile {
    fn install(sub_app: &mut bevy_app::SubApp) {
        let note = sub_app
            .world()
            .resource::<Blocks>()
            .default_state("minecraft:note_block")
            .expect("a note block");
        sub_app.add_plugins(BlockSimulationPlugin::new(Fragile { note }));
        sub_app.init_resource::<SeenDrops>();
        sub_app.add_systems(Last, record_drops);
    }
}

impl BlockBehaviour<WorldKinds> for Fragile {
    fn update_shape<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        _cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        _at: BlockPos,
        state: BlockStateId,
        _from: Direction,
        _neighbour: BlockPos,
        _neighbour_state: BlockStateId,
    ) -> BlockStateId {
        if state == self.note {
            BlockStateId::AIR
        } else {
            state
        }
    }
}

impl BlockDrops for Fragile {
    fn drops(
        &self,
        block: &DroppingBlock,
        _block_at: impl Fn(BlockPos) -> Option<BlockStateId>,
        drops: &mut Vec<ItemStack>,
    ) {
        if block.state == self.note {
            drops.push(ItemStack::new(Item::NoteBlock.id(), 1));
        }
    }
}

#[derive(Resource, Default)]
struct SeenDrops(Vec<ItemDropped>);

fn record_drops(mut dropped: MessageReader<ItemDropped>, mut seen: ResMut<SeenDrops>) {
    seen.0.extend(dropped.read().copied());
}

/// A block that a shape update destroys leaves the items that the rules name for it, at its
/// position.
#[test]
fn a_block_that_a_shape_update_destroys_drops_the_items_of_the_rules() {
    let mut app = server_with(1, DimensionBlockRules(Fragile::install));
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let note = block_state(&app, "minecraft:note_block");
    let stone = block_state(&app, "minecraft:stone");
    let pos = BlockPos::new(3, 120, 5);
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5), 2);
    for at in 0..40 {
        match at {
            25 => place(&mut app, label, east, pos, note),
            30 => place(&mut app, label, east, BlockPos::new(4, 120, 5), stone),
            _ => {}
        }
        lockstep_tick(&mut app, label);
    }

    let seen = cell_worlds(&mut app, label)[0]
        .resource::<SeenDrops>()
        .0
        .clone();
    assert_eq!(
        seen,
        [ItemDropped {
            at: pos,
            stack: ItemStack::new(Item::NoteBlock.id(), 1),
        }]
    );
    assert_eq!(block_at(&mut app, label, pos), Some(BlockStateId::AIR));

    let world = cell_worlds(&mut app, label)
        .into_iter()
        .next()
        .expect("a cell");
    let mut items = Vec::new();
    for (_, rows) in world.resource::<EntityRows>().tiles() {
        items.extend(rows.rows().map(|row| (row.id, row.state.body.position)));
    }
    let [(_, position)] = items[..] else {
        panic!("one item row: {items:?}");
    };
    // PHY-CMP-025: 0.25 of random offset, then at most 0.1 per tick for the ten ticks since.
    assert!(
        (position.x - 3.5).abs() < 1.25 && (position.z - 5.5).abs() < 1.25,
        "the item starts at the block: {position}"
    );
    let mut stacks = world.query_filtered::<(&Kind, &Count), With<Dropped>>();
    let stacks: Vec<_> = stacks
        .iter(world)
        .map(|(&kind, count)| (kind, count.get()))
        .collect();
    assert_eq!(stacks, [(Kind(Item::NoteBlock.id().number()), 1)]);

    // The item starts with 0.2 upward, so it is back near its start ten ticks later.
    for _ in 0..20 {
        lockstep_tick(&mut app, label);
    }
    let world = cell_worlds(&mut app, label)
        .into_iter()
        .next()
        .expect("a cell");
    let later = world
        .resource::<EntityRows>()
        .tiles()
        .flat_map(|(_, rows)| {
            rows.rows()
                .map(|row| row.state.body.position.y)
                .collect::<Vec<_>>()
        })
        .next();
    assert!(
        later.is_some_and(|y| y < position.y),
        "the pass moves the item: it falls from {position} to {later:?}"
    );
}

#[test]
fn a_command_at_a_player_of_another_cell_reads_the_columns_of_that_cell() {
    let mut app = server(2);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    for world in cell_worlds(&mut app, label) {
        world.insert_resource(DefaultOpLevel(PlayerOpLevel(2)));
    }
    let west_anchor = host_app::admit(
        &mut app,
        label,
        &Arrival {
            uuid: Uuid::from_u128(1),
            name: "west",
            position: DVec3::new(-8.5, 100.0, 8.5),
            view_distance: 2,
        },
    );
    let west = host_app::session_of(&app, west_anchor).expect("a placed session");
    let east_position = DVec3::new(200.5, 100.0, 8.5);
    let east = admit(&mut app, label, "east", east_position, 2);
    let column = ColumnPos::from(east_position);
    let mut east_client = Client::default();
    let deadline = Instant::now() + Duration::from_secs(120);
    while !east_client.loaded.contains(&(column.x, column.z)) {
        assert!(
            Instant::now() < deadline,
            "the east client never got its column"
        );
        for session in [west, east] {
            app.world_mut()
                .resource_mut::<MuxerLink>()
                .acknowledge_batch(session, 64.0);
        }
        east_client.take(sent_to(east, lockstep_tick(&mut app, label)));
    }

    let mut server = PlayServer { app, dim: label };
    server.queue(
        west_anchor,
        &ServerboundChatCommand {
            command: Bounded("execute at east if loaded ~ ~ ~"),
        },
    );
    let chats: Vec<Text> = server
        .ticks(west_anchor, 5)
        .into_iter()
        .filter_map(|packet| match packet {
            PacketPayload::SystemChat(chat) => Some(
                chat.content
                    .to_text()
                    .expect("a chat text")
                    .map(&ItemHoverMap)
                    .expect("a chat text"),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        chats,
        [Text::translate(
            "commands.execute.conditional.pass",
            Vec::<Text>::new()
        )]
    );
}

const HALO_PROBE: BlockPos = BlockPos::new(1, 120, 1);

/// Whether the entity pass of the last tick of a cell saw the probe block in its copy.
#[derive(Resource, Default)]
struct SeenAtSimulate {
    probe: Option<BlockStateId>,
    seen: bool,
}

fn look_at_probe(mut seen: ResMut<SeenAtSimulate>, dimensions: Query<&SectionIndex>) {
    let section = SectionPos::from(HALO_PROBE);
    let state = dimensions
        .iter()
        .find_map(|index| index.blocks(section))
        .map(|blocks| blocks.get(LocalPos::from(HALO_PROBE)));
    seen.seen = state.is_some() && state == seen.probe;
}

/// For each cell that holds the column of the probe, the tick at which its entity pass first
/// saw the block that the owner placed.
fn run_halo_probe(cells: u32) -> Vec<usize> {
    let mut app = server(cells);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let probe = block_state(&app, "minecraft:stone");
    for sub_app in app
        .world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .cell_sub_apps_mut(label)
    {
        sub_app.insert_resource(SeenAtSimulate {
            probe: Some(probe),
            seen: false,
        });
        sub_app.add_systems(FixedUpdate, look_at_probe.in_set(TickSet::Simulate));
    }
    let owner = admit(&mut app, label, "owner", DVec3::new(8.5, 100.0, 8.5), 2);
    for (name, x, z) in [
        ("west", -8.5, 8.5),
        ("north", 8.5, -8.5),
        ("corner", -8.5, -8.5),
    ] {
        admit(&mut app, label, name, DVec3::new(x, 100.0, z), 2);
    }
    let mut first_seen: Vec<Option<usize>> = vec![None; cells as usize];
    for at in 0..40 {
        if at == 30 {
            place(&mut app, label, owner, HALO_PROBE, probe);
        }
        lockstep_tick(&mut app, label);
        for (cell, world) in cell_worlds(&mut app, label).into_iter().enumerate() {
            if world.resource::<SeenAtSimulate>().seen {
                first_seen[cell].get_or_insert(at);
            }
        }
    }
    first_seen
        .into_iter()
        .map(|at| at.expect("each cell holds the column of the probe"))
        .collect()
}

/// PHY-INV-014, PHY-CMP-009: a block that the owner sets in tick `T` is in the copy of every
/// other cell before the entities of `T + 1` move, as in a world of one cell.
#[test]
fn a_change_reaches_the_halo_of_every_cell_before_the_next_entity_pass() {
    let one = run_halo_probe(1);
    let four = run_halo_probe(4);
    assert_eq!(one, [31], "the entity pass after the write sees it");
    assert_eq!(four, [31; 4]);
}

/// Probe rules, not the rules of any game block: a note block that is placed queues a block
/// event, and the event lights a lamp south of it.
#[derive(Clone, Copy, Debug)]
struct Echo {
    note: BlockStateId,
    lit: BlockStateId,
}

const ECHO: BlockPos = BlockPos::new(1, 120, 1);
const ECHO_BESIDE: BlockPos = BlockPos::new(2, 120, 1);
const ECHO_LAMP: BlockPos = BlockPos::new(1, 120, 2);

impl Echo {
    fn install(sub_app: &mut bevy_app::SubApp) {
        let echo = Echo {
            note: sub_app
                .world()
                .resource::<Blocks>()
                .default_state("minecraft:note_block")
                .expect("a note block"),
            lit: lit_lamp_in(sub_app.world().resource::<Blocks>()),
        };
        sub_app.add_plugins(BlockSimulationPlugin::new(echo));
    }
}

fn lit_lamp_in(blocks: &Blocks) -> BlockStateId {
    let lamp = blocks.block("minecraft:redstone_lamp").expect("a lamp");
    lamp.with(lamp.default_state_id, "lit", &PropertyValue::Bool(true))
        .expect("a lit lamp")
}

impl BlockBehaviour<WorldKinds> for Echo {
    fn placed<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        at: BlockPos,
        new: BlockStateId,
        _old: BlockStateId,
        _moved_by_piston: bool,
    ) {
        if new == self.note {
            cx.queue_block_event(at, Block::NoteBlock.id(), 1, 0);
        }
    }

    fn block_event<V: SimulationVolume, F: BlockFacts<WorldKinds>>(
        &self,
        cx: &mut Cx<'_, WorldKinds, V, F, Self>,
        _at: BlockPos,
        _state: BlockStateId,
        _a: i32,
        _b: i32,
    ) -> bool {
        cx.set(ECHO_LAMP, self.lit, BlockUpdateFlags::CLIENTS, 512);
        true
    }
}

impl BlockDrops for Echo {}

/// The block packets and block events of one tick, in wire order.
fn block_traffic(packets: Vec<PacketPayload>) -> Vec<String> {
    packets
        .into_iter()
        .filter_map(|packet| match packet {
            PacketPayload::BlockUpdate(update) => Some(format!("block {:?}", update.block_pos)),
            PacketPayload::SectionBlocksUpdate(update) => {
                Some(format!("section {} blocks", update.blocks.len()))
            }
            PacketPayload::BlockEvent(event) => Some(format!("event {:?}", event.pos)),
            _ => None,
        })
        .collect()
}

/// SIM-SET-005, SIM-MODEL-107(a), SIM-TICK-002: the writes of a tick leave in one packet per
/// section before the block events, and a write of a block event leaves in the next broadcast.
#[test]
fn a_broadcast_sends_one_packet_per_section_before_the_block_events() {
    let mut app = server_with(1, DimensionBlockRules(Echo::install));
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let note = block_state(&app, "minecraft:note_block");
    let stone = block_state(&app, "minecraft:stone");
    let viewer = admit(&mut app, label, "viewer", DVec3::new(8.5, 100.0, 8.5), 2);
    let column = ColumnPos::from(ECHO);
    let mut client = Client::default();
    let deadline = Instant::now() + Duration::from_secs(120);
    while !client.loaded.contains(&(column.x, column.z)) {
        assert!(Instant::now() < deadline, "the client never got the column");
        app.world_mut()
            .resource_mut::<MuxerLink>()
            .acknowledge_batch(viewer, 64.0);
        client.take(sent_to(viewer, lockstep_tick(&mut app, label)));
    }

    place(&mut app, label, viewer, ECHO, note);
    place(&mut app, label, viewer, ECHO_BESIDE, stone);
    let ticks: Vec<Vec<String>> = (0..2)
        .map(|_| block_traffic(sent_to(viewer, lockstep_tick(&mut app, label))))
        .collect();

    assert_eq!(
        ticks,
        [
            vec!["section 2 blocks".to_owned(), format!("event {ECHO:?}")],
            vec![format!("block {ECHO_LAMP:?}")],
        ]
    );
}

/// A player breaks a block of the column of the other cell, across the boundary at x = 0. The
/// cell of the player reads the block from its halo and sends the write to the owner. A client
/// of the other cell that holds the column sees the break as a client of the same cell does.
#[test]
fn a_block_of_the_other_cell_breaks_for_a_client_of_that_cell() {
    let mut app = server(2);
    let label = label_of(&app, OVERWORLD);
    wait_for_generation(&mut app, label);
    let stone = block_state(&app, "minecraft:stone");
    let target = BlockPos::new(1, 100, 5);
    let breaker_anchor = host_app::admit(
        &mut app,
        label,
        &Arrival {
            uuid: Uuid::from_u128(1),
            name: "breaker",
            position: DVec3::new(-2.5, 100.0, 5.5),
            view_distance: 2,
        },
    );
    let breaker = host_app::session_of(&app, breaker_anchor).expect("a placed session");
    let watcher = admit(&mut app, label, "watcher", DVec3::new(8.5, 100.0, 8.5), 2);
    let column = ColumnPos::from(target);
    let mut breaker_client = Client::default();
    let mut watcher_client = Client::default();
    let deadline = Instant::now() + Duration::from_secs(120);
    while !(breaker_client.loaded.contains(&(column.x, column.z))
        && watcher_client.loaded.contains(&(column.x, column.z)))
    {
        assert!(
            Instant::now() < deadline,
            "the clients never got the column"
        );
        for session in [breaker, watcher] {
            app.world_mut()
                .resource_mut::<MuxerLink>()
                .acknowledge_batch(session, 64.0);
        }
        let packets = lockstep_tick(&mut app, label);
        breaker_client.take(sent_to(breaker, packets.clone()));
        watcher_client.take(sent_to(watcher, packets));
    }
    let Holder { cell, player } = holder(&mut app, label, breaker).expect("the breaker");
    let owner = holder(&mut app, label, watcher).expect("the watcher").cell;
    assert_ne!(cell, owner, "the players stand in different cells");
    cell_worlds(&mut app, label)
        .swap_remove(cell)
        .get_mut::<PlayerGameMode>(player)
        .expect("a game mode")
        .0 = GameMode::Creative;
    place(&mut app, label, watcher, target, stone);
    for _ in 0..3 {
        lockstep_tick(&mut app, label);
    }
    assert_eq!(block_at(&mut app, label, target), Some(stone));

    let mut server = PlayServer { app, dim: label };
    server.queue(
        breaker_anchor,
        &ServerboundPlayerAction {
            action: PlayerAction::StartDestroyBlock,
            pos: target,
            direction: Direction::Up,
            sequence: VarInt(5),
        },
    );
    let packets = server.ticks_all(4);

    let seen = |session| -> Vec<String> {
        sent_to(session, packets.clone())
            .into_iter()
            .filter_map(|packet| match packet {
                PacketPayload::LevelEvent(event) if event.pos == target => Some(format!(
                    "level_event {} {}",
                    event.effect, event.effect_data
                )),
                PacketPayload::BlockUpdate(update) if update.block_pos == target => {
                    Some(format!("block_update {}", update.block_state_id.0))
                }
                PacketPayload::BlockChangedAck(ack) => Some(format!("ack {}", ack.sequence.0)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        seen(watcher),
        [
            format!("level_event 2001 {}", stone.number()),
            "block_update 0".to_owned()
        ]
    );
    // The owner applies the write one tick after the cell of the breaker acknowledges it.
    assert_eq!(seen(breaker), ["ack 5", "block_update 0"]);
    assert_eq!(
        block_at(&mut server.app, label, target),
        Some(BlockStateId::AIR)
    );
}
