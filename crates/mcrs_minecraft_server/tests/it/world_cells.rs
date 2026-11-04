//! A world divided into cells must look the same as the world of one cell: the blocks and the
//! light of every column at its owner, where each player stands, and what each client gets.
//!
//! The players stand on both sides of the boundary between two regions, place blocks and light
//! on both sides of it, and walk across it. Each run waits after every tick for the generation
//! and light steps of every cell, so the stages land at the same tick in both runs.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::{DVec3, Vec2};
use bevy_tasks::block_on;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_chunk::{BlockLight, LightStorage, SkyLight};
use mcrs_minecraft_engine::bus::{
    InboundPlayerSpawn, OutboundPlayerPacket, PacketPayload, PlayerTransferSnapshot,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::column_store::ColumnStoreLink;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::entity_replication::{self, Answers};
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle};
use mcrs_minecraft_engine_server::{Placement, WorldCells, runner};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::session::{
    Owner, Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundAddEntity, ClientboundRemoveEntities};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_minecraft_vanilla_light::LightLink;
use mcrs_minecraft_vanilla_worldgen::GenerationLink;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

use crate::configuration_registries::ASSETS;
use crate::host_app::{label_of, replicated, sent_to};

const OVERWORLD: &str = "minecraft:overworld";
const TICKS: usize = 90;
const PATIENCE: Duration = Duration::from_secs(120);

fn server(cells: u32) -> App {
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
    app.finish();
    app.cleanup();
    crate::support::update_until_playing(&mut app);
    runner::between_ticks(&mut app);
    app
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
    for world in cell_worlds(app, label) {
        if let Some(mut generation) = world.get_resource_mut::<GenerationLink>() {
            block_on(generation.answered());
        }
        if let Some(mut light) = world.get_resource_mut::<LightLink>() {
            block_on(light.answered());
        }
        if let Some(mut store) = world.get_resource_mut::<ColumnStoreLink>() {
            block_on(store.answered());
        }
    }
}

/// Runs one tick and returns the packets it wrote for the clients.
fn tick(app: &mut App, label: Entity) -> Vec<OutboundPlayerPacket> {
    app.update();
    runner::between_ticks(app);
    entity_replication::deliver(app.world_mut(), Answers::All);
    let packets = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();
    wait_for_steps(app, label);
    packets
}

/// Places a session in `label` and sends the spawn of its player at `position`.
fn admit(app: &mut App, label: Entity, name: &str, position: DVec3) -> PlayerSession {
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
                client_info: Some(crate::support::client_info_with_view_distance(2)),
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
            flags: BlockUpdateFlags::all(),
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
}

impl Client {
    fn take(&mut self, packets: Vec<PacketPayload>) {
        for packet in &packets {
            match packet {
                PacketPayload::PlayerLogin(_) => self.logins += 1,
                PacketPayload::Respawn(_) => self.respawns += 1,
                PacketPayload::ChunkLoad { column, .. } => {
                    self.loaded.insert((column.x, column.z));
                }
                PacketPayload::ChunkUnload(forget) => {
                    self.forgotten.insert((forget.x, forget.z));
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
    let deadline = Instant::now() + PATIENCE;
    while !cell_worlds(&mut app, label)
        .into_iter()
        .all(|world| world.contains_resource::<FillContext>())
    {
        assert!(
            Instant::now() < deadline,
            "the generation context never loaded"
        );
        tick(&mut app, label);
    }
    let blocks = app.world().resource::<Blocks>().clone();
    let state = |name: &str| {
        blocks
            .block(name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .default_state_id
    };
    let glowstone = state("minecraft:glowstone");
    let stone = state("minecraft:stone");

    // Region -1 and region 0 meet at x = 0: one cell owns each side.
    let west = admit(&mut app, label, "west", DVec3::new(-8.5, 100.0, 8.5));
    let east = admit(&mut app, label, "east", DVec3::new(8.5, 100.0, 8.5));
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
