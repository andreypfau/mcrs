//! Block placing as the cell of the placer hands it to the muxer: the packets for the placer
//! first, then the changes of the columns to their holders, then the acknowledgements.

use bevy_app::{App, Last};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use bevy_math::{DVec3, IVec3, Vec3};
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::{Direction, Look};
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::block_action_ack::BlockActionAck;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine::column_change::ColumnChange;
use mcrs_minecraft_engine::column_streams::ColumnStreams;
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine_item::{HolderCommandsExt, Slots};
use mcrs_minecraft_engine_registries::{install_item_table, test_item_table};
use mcrs_minecraft_engine_world::block_update::BlockSetRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::palette::air_section;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_engine_world::world::storage::random_ticks::RandomTickIndex;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::{Count, ItemStack, Kind};
use mcrs_minecraft_item_table::{DefinitionFile, ItemTable, bake};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::Hand;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundUseItemOn,
};
use mcrs_minecraft_protocol_game::text::TextContent;
use mcrs_minecraft_server::{
    VanillaBlockPlacePlugin, VanillaBlocksPlugin, VanillaEntityPhysicsPlugin,
    VanillaItemEntityPlugin,
};
use mcrs_minecraft_simulation::BlockUpdateFlags;
use mcrs_voxel::{BlockPos, BlockStateId, ColumnPos, SectionPos};

use crate::commands::chats;
use crate::host_app::{self, PlayServer};

const PLACER: Uuid = Uuid::from_u128(1);
const STONE: Kind = Kind(1);

#[derive(Resource, Default)]
struct Captured(Vec<Out>);

enum Out {
    Packet(OutboundPlayerPacket),
    Change(ColumnChange),
    Ack(BlockActionAck),
}

fn capture(
    mut packets: MessageReader<OutboundPlayerPacket>,
    mut changes: MessageReader<ColumnChange>,
    mut acks: MessageReader<BlockActionAck>,
    mut captured: ResMut<Captured>,
) {
    captured.0.extend(packets.read().cloned().map(Out::Packet));
    captured.0.extend(changes.read().cloned().map(Out::Change));
    captured.0.extend(acks.read().copied().map(Out::Ack));
}

/// The items `air` and `stone`, at their wire ids 0 and 1.
fn table() -> ItemTable {
    let files: Vec<String> = ["air", "stone"]
        .iter()
        .zip(0..)
        .map(|(name, id)| {
            format!(
                r#"{{"minecraft:item":{{"description":{{"identifier":"minecraft:{name}","protocol_id":{id}}},"components":{{"minecraft:max_stack_size":64}}}}}}"#
            )
        })
        .collect();
    bake(
        files.iter().map(|bytes| DefinitionFile {
            name: "test",
            bytes: bytes.as_bytes(),
        }),
        &ItemTable::leak([]),
    )
    .expect("the test items bake")
    .table
}

struct Placer {
    anchor: Entity,
    player: Entity,
    session: PlayerSession,
}

fn ticks(server: &mut PlayServer, n: usize) -> Vec<Out> {
    server.ticks_all(n);
    std::mem::take(&mut server.world().resource_mut::<Captured>().0)
}

fn add_section(server: &mut PlayServer, dimension: Entity, pos: BlockPos) {
    let section_pos = SectionPos::from(pos);
    let world = server.world();
    if world
        .get::<SectionIndex>(dimension)
        .unwrap()
        .blocks(section_pos)
        .is_some()
    {
        return;
    }
    let section = world.spawn((section_pos, InDimension(dimension))).id();
    let mut random_ticks = world.remove_resource::<RandomTickIndex>();
    let mut index = world.get_mut::<SectionIndex>(dimension).unwrap();
    index.insert(section_pos, section);
    index.land(
        section_pos,
        section,
        air_section(),
        random_ticks.as_mut(),
        false,
    );
    if let Some(random_ticks) = random_ticks {
        world.insert_resource(random_ticks);
    }
}

fn stone(server: &mut PlayServer) -> BlockStateId {
    server
        .world()
        .resource::<Blocks>()
        .default_state("minecraft:stone")
        .unwrap()
}

fn placing_app() -> App {
    let mut app = host_app::make_host_app();
    app.insert_resource(table());
    app.add_plugins((
        VanillaEntityPhysicsPlugin,
        VanillaItemEntityPlugin,
        VanillaBlockPlacePlugin,
    ));
    app
}

/// A creative placer with stone in its selected slot, standing two blocks north of stone at
/// `ground`, in a column that the muxer subscribes to.
fn placer_beside(
    ground: impl Fn(&DimensionTypeConfig) -> BlockPos,
) -> (PlayServer, Placer, BlockPos) {
    placer_in(placing_app(), ground)
}

fn placer_in(
    app: App,
    ground: impl Fn(&DimensionTypeConfig) -> BlockPos,
) -> (PlayServer, Placer, BlockPos) {
    let mut server = PlayServer::start(app, &[], 0);
    let sub_app = host_app::sub_app(&mut server.app, server.dim);
    sub_app.init_resource::<Captured>();
    sub_app.add_systems(Last, capture);
    let (anchor, _) = server.join(PLACER, "Placer");
    let session = server.session_of(anchor).unwrap();
    let player = server.player(anchor);
    let world = server.world();
    world.get_mut::<PlayerGameMode>(player).unwrap().0 = GameMode::Creative;
    let dimension = world.get::<InDimension>(player).unwrap().0;
    let ground = ground(world.get(dimension).unwrap());
    world
        .entity_mut(dimension)
        .insert(ColumnStreams::subscribed_to([ColumnPos::from(ground)]));
    add_section(&mut server, dimension, ground);
    let state = stone(&mut server);
    server
        .world()
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos: ground,
            new_state: state,
            flags: BlockUpdateFlags::all(),
            recursion_left: 1,
        });
    ticks(&mut server, 2);
    let world = server.world();
    world
        .commands()
        .entity(player)
        .give(ItemStack::new(STONE, Count::new(64).unwrap()));
    world.flush();
    let placer = Placer {
        anchor,
        player,
        session,
    };
    move_to(
        &mut server,
        &placer,
        ground.as_dvec3() + DVec3::new(0.5, 0.0, -1.5),
    );
    (server, placer, ground)
}

/// Moves the placer, and accepts the teleport that the move sends, so that no teleport is open
/// (PLC-TEL-001).
fn move_to(server: &mut PlayServer, placer: &Placer, to: DVec3) {
    server
        .world()
        .get_mut::<Transform>(placer.player)
        .unwrap()
        .translation = to;
    ticks(server, 2);
    let id = server
        .world()
        .get::<TeleportState>(placer.player)
        .and_then(|teleport| teleport.pending())
        .map(|pending| pending.id)
        .unwrap();
    server.queue(
        placer.anchor,
        &ServerboundAcceptTeleportation {
            teleport_id: VarInt(id),
            position: to.into(),
            look: Look::default(),
        },
    );
    ticks(server, 2);
}

fn use_on(pos: BlockPos, face: Direction, sequence: i32) -> ServerboundUseItemOn {
    let cursor = Vec3::splat(0.5) + face.normal().as_vec3() * 0.5;
    ServerboundUseItemOn {
        hand: Hand::Main,
        block_pos: pos,
        face,
        cursor_x: cursor.x,
        cursor_y: cursor.y,
        cursor_z: cursor.z,
        inside_block: false,
        world_border_hit: false,
        sequence: VarInt(sequence),
    }
}

fn describe(payload: &PacketPayload) -> Option<String> {
    match payload {
        PacketPayload::BlockUpdate(update) => Some(format!(
            "block_update {} {}",
            update.block_pos.y, update.block_state_id.0
        )),
        PacketPayload::SystemChat(chat) => {
            let text = chats(std::slice::from_ref(payload)).pop()?;
            let TextContent::Translate { translate, .. } = &text.content else {
                return None;
            };
            Some(format!("system_chat {translate} overlay {}", chat.overlay))
        }
        _ => None,
    }
}

struct Answer {
    placer: Vec<String>,
    watcher: Vec<String>,
}

fn send(server: &mut PlayServer, placer: &Placer, packet: &ServerboundUseItemOn) -> Answer {
    server.queue(placer.anchor, packet);
    let out = ticks(server, 2);
    let placer_gets = |out: &Out| match out {
        Out::Packet(packet) if matches!(packet.target, PacketTarget::SinglePlayer(target) if target == placer.session) => {
            describe(&packet.data)
        }
        Out::Change(change) if change.except != Some(placer.session) => describe(&change.payload),
        Out::Ack(ack) if ack.session == placer.session => Some(format!("ack {}", ack.sequence)),
        _ => None,
    };
    let watcher_gets = |out: &Out| match out {
        Out::Change(change) => describe(&change.payload),
        _ => None,
    };
    Answer {
        placer: out.iter().filter_map(placer_gets).collect(),
        watcher: out.iter().filter_map(watcher_gets).collect(),
    }
}

fn on_the_ground(_: &DimensionTypeConfig) -> BlockPos {
    BlockPos::new(10, 64, 8)
}

#[test]
fn plc_pla_001_a_creative_placer_gets_the_refresh_the_new_state_and_then_the_ack() {
    let (mut server, placer, ground) = placer_beside(on_the_ground);
    let stone = stone(&mut server).number();

    let answer = send(&mut server, &placer, &use_on(ground, Direction::Up, 7));

    assert_eq!(
        answer.placer,
        [
            format!("block_update 64 {stone}"),
            format!("block_update 65 {stone}"),
            format!("block_update 65 {stone}"),
            "ack 7".to_owned(),
        ]
    );
    assert_eq!(answer.watcher, [format!("block_update 65 {stone}")]);
}

#[test]
fn plc_hgt_001_a_click_above_the_height_limit_gets_the_message_and_then_the_ack() {
    let top = |config: &DimensionTypeConfig| {
        BlockPos::new(10, config.min_y + config.height as i32 - 1, 8)
    };
    let (mut server, placer, ground) = placer_beside(top);
    let above = ground + IVec3::Y;

    let answer = send(&mut server, &placer, &use_on(above, Direction::Up, 3));

    assert_eq!(
        answer.placer,
        ["system_chat build.tooHigh overlay true", "ack 3"]
    );
    assert_eq!(answer.watcher, Vec::<String>::new());
}

#[test]
fn plc_hgt_002_stone_at_the_top_refreshes_with_void_air_above_it() {
    let top = |config: &DimensionTypeConfig| {
        BlockPos::new(10, config.min_y + config.height as i32 - 1, 8)
    };
    let (mut server, placer, ground) = placer_beside(top);
    let stone = stone(&mut server).number();
    let void_air = server
        .world()
        .resource::<Blocks>()
        .default_state("minecraft:void_air")
        .unwrap()
        .number();

    let answer = send(&mut server, &placer, &use_on(ground, Direction::Up, 4));

    assert_eq!(
        answer.placer,
        [
            "system_chat build.tooHigh overlay true".to_owned(),
            format!("block_update {} {stone}", ground.y),
            format!("block_update {} {void_air}", ground.y + 1),
            "ack 4".to_owned(),
        ]
    );
    assert_eq!(answer.watcher, Vec::<String>::new());
}

fn assert_refused_with_a_refresh(answer: &Answer, stone: u16, sequence: i32) {
    assert_eq!(
        answer.placer,
        [
            format!("block_update 64 {stone}"),
            "block_update 65 0".to_owned(),
            format!("ack {sequence}"),
        ]
    );
    assert_eq!(answer.watcher, Vec::<String>::new());
}

#[test]
fn plc_ent_002_the_placer_blocks_a_block_in_its_own_box() {
    let (mut server, placer, ground) = placer_beside(on_the_ground);
    let stone = stone(&mut server).number();
    move_to(
        &mut server,
        &placer,
        (ground + IVec3::Y).as_dvec3() + DVec3::new(0.5, 0.0, 0.5),
    );

    let answer = send(&mut server, &placer, &use_on(ground, Direction::Up, 9));

    assert_refused_with_a_refresh(&answer, stone, 9);
}

#[test]
fn plc_tel_001_an_open_teleport_refuses_with_a_refresh() {
    let (mut server, placer, ground) = placer_beside(on_the_ground);
    let stone = stone(&mut server).number();
    server
        .world()
        .get_mut::<Transform>(placer.player)
        .unwrap()
        .translation = ground.as_dvec3() + DVec3::new(0.5, 0.0, -2.5);
    ticks(&mut server, 2);

    let answer = send(&mut server, &placer, &use_on(ground, Direction::Up, 11));

    assert_refused_with_a_refresh(&answer, stone, 11);
}

#[test]
fn plc_inv_006_a_survival_placer_spends_its_last_block_once() {
    let (mut server, placer, ground) = placer_beside(on_the_ground);
    let stone = stone(&mut server).number();
    let world = server.world();
    world.get_mut::<PlayerGameMode>(placer.player).unwrap().0 = GameMode::Survival;
    let stack = world.get::<Slots>(placer.player).unwrap().get(0).unwrap();
    world.entity_mut(stack).insert(Count::new(1).unwrap());
    let above = ground + IVec3::Y;
    server.queue(placer.anchor, &use_on(ground, Direction::Up, 1));

    let answer = send(&mut server, &placer, &use_on(above, Direction::Up, 2));

    assert_eq!(
        answer.watcher,
        [format!("block_update 65 {stone}")],
        "only the first request places"
    );
    assert_eq!(
        server.world().get::<Slots>(placer.player).unwrap().get(0),
        None
    );
}

/// PLC-SUP-001, SUP-RUN-002, SUP-VEG-001: a poppy is refused on stone, which its tag does not
/// hold, and placed on dirt.
#[test]
fn plc_sup_001_a_poppy_is_placed_only_where_it_can_stay() {
    let mut app = host_app::make_host_app();
    install_item_table(app.world_mut(), test_item_table()).unwrap();
    app.add_plugins((
        VanillaBlocksPlugin,
        VanillaEntityPhysicsPlugin,
        VanillaItemEntityPlugin,
        VanillaBlockPlacePlugin,
    ));
    let (mut server, placer, ground) = placer_in(app, on_the_ground);
    let stone = stone(&mut server).number();
    let blocks = server.world().resource::<Blocks>();
    let dirt = blocks.default_state("minecraft:dirt").unwrap();
    let poppy = blocks.default_state("minecraft:poppy").unwrap().number();
    let world = server.world();
    let stack = world.get::<Slots>(placer.player).unwrap().get(0).unwrap();
    world
        .entity_mut(stack)
        .insert(Kind(Item::Poppy.id().number()));

    let refused = send(&mut server, &placer, &use_on(ground, Direction::Up, 3));
    assert_refused_with_a_refresh(&refused, stone, 3);

    let dimension = server.world().get::<InDimension>(placer.player).unwrap().0;
    server
        .world()
        .resource_mut::<Messages<BlockSetRequest>>()
        .write(BlockSetRequest {
            dimension,
            pos: ground,
            new_state: dirt,
            flags: BlockUpdateFlags::all(),
            recursion_left: 1,
        });
    ticks(&mut server, 2);
    let placed = send(&mut server, &placer, &use_on(ground, Direction::Up, 4));
    assert_eq!(placed.watcher, [format!("block_update 65 {poppy}")]);
}
