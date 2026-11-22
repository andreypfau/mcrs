use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::bus::{PacketPayload, to};
use mcrs_minecraft_engine::muxer::bridge::dispatch_encode;
use mcrs_minecraft_engine::muxer::bridge_queue::OutboundQueue;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_mob_attributes::AttributeEdit;
use mcrs_minecraft_engine_mob_attributes::MobAttributes;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    InstanceEdit, InstanceModifier, ModifierKind, ModifierOperation,
};
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_lang::Language;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_network::{OutboundReceiver, ServerSideConnection};
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol::packets::table::Decoded;
use mcrs_minecraft_protocol::packets::table::game_clientbound::ClientboundGamePacket as BasePacket;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{PositionPath, VecDelta};
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::position_delta;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::table::game_clientbound::{self, ClientboundGamePacket};
use mcrs_minecraft_protocol_game::text::{Text, TextContent, TranslateArg};
use mcrs_minecraft_server::{VanillaEntityAttributesPlugin, VanillaEntityPhysicsPlugin};

use crate::commands::{chats, load_stone_column};
use crate::host_app::{self, PlayServer};
use crate::mock_connection;

const OPERATOR: Uuid = Uuid::from_u128(1);

#[derive(Debug, PartialEq)]
enum Seen {
    Delimiter,
    Add {
        id: i32,
        kind: EntityType,
        uuid: Uuid,
        pos: DVec3,
    },
    Data {
        id: i32,
        health: Option<f32>,
    },
    Attributes {
        id: i32,
        entries: Vec<SeenEntry>,
    },
    Move {
        id: i32,
    },
    Sync {
        id: i32,
        on_ground: bool,
    },
    Motion {
        id: i32,
    },
    Event {
        id: i32,
        event: i8,
    },
    Hurt {
        id: i32,
    },
    Dying {
        id: i32,
        health: Option<f32>,
    },
    Remove {
        ids: Vec<i32>,
    },
}

#[derive(Debug, PartialEq)]
struct SeenEntry {
    attribute: Attribute,
    base: f64,
    modifiers: Vec<String>,
}

struct Viewer {
    world: World,
    connection: Entity,
    frames: OutboundReceiver,
    seen: Vec<Seen>,
    positions: Vec<(i32, DVec3)>,
}

impl Viewer {
    fn new() -> Self {
        let mut world = World::new();
        world.init_resource::<BridgeTelemetry>();
        world.init_resource::<mcrs_minecraft_engine::muxer::bridge::PendingConnections>();
        let (raw, frames) = mock_connection::make_mock_raw_connection();
        let connection = world
            .spawn((
                ServerSideConnection { raw: Box::new(raw) },
                OutboundQueue::default(),
            ))
            .id();
        Self {
            world,
            connection,
            frames,
            seen: Vec::new(),
            positions: Vec::new(),
        }
    }

    fn receive(&mut self, payloads: Vec<PacketPayload>) {
        let mut queue = self
            .world
            .get_mut::<OutboundQueue>(self.connection)
            .unwrap();
        for data in payloads {
            if matches!(data, PacketPayload::Replication(_)) {
                queue.push(to(PlayerSession(0), data));
            }
        }
        self.world.run_system_cached(dispatch_encode).unwrap();
        while let Ok(frames) = self.frames.try_recv() {
            let mut rest: &[u8] = &frames;
            while !rest.is_empty() {
                let length = VarInt::decode(&mut rest).unwrap().0 as usize;
                let (mut frame, tail) = rest.split_at(length);
                rest = tail;
                let id = VarInt::decode(&mut frame).unwrap().0;
                if let Decoded::Packet(packet) = game_clientbound::decode(id, &mut frame).unwrap() {
                    assert!(
                        frame.is_empty(),
                        "packet {id} has {} trailing bytes",
                        frame.len()
                    );
                    self.apply(packet);
                }
            }
        }
    }

    fn apply(&mut self, packet: ClientboundGamePacket<'_>) {
        let seen = match packet {
            ClientboundGamePacket::Base(BasePacket::ClientboundBundleDelimiter(_)) => {
                Seen::Delimiter
            }
            ClientboundGamePacket::ClientboundAddEntity(add) => {
                self.positions.push((add.id.0, add.pos));
                Seen::Add {
                    id: add.id.0,
                    kind: add.kind,
                    uuid: add.uuid,
                    pos: add.pos,
                }
            }
            ClientboundGamePacket::ClientboundSetEntityData(data) => {
                let id = data.entity_id.0;
                let health = data.metadata.0.iter().find_map(|entry| match entry.value {
                    MetaDataValue::Float(health) if entry.index == 9 => Some(health),
                    _ => None,
                });
                let dying = data
                    .metadata
                    .0
                    .iter()
                    .any(|entry| matches!(entry.value, MetaDataValue::Pose(Pose::Dying)));
                if dying {
                    Seen::Dying { id, health }
                } else {
                    Seen::Data { id, health }
                }
            }
            ClientboundGamePacket::ClientboundEntityEvent(event) => Seen::Event {
                id: event.entity_id,
                event: event.entity_status,
            },
            ClientboundGamePacket::ClientboundDamageEvent(event) => Seen::Hurt {
                id: event.entity_id.0,
            },
            ClientboundGamePacket::ClientboundRemoveEntities(remove) => {
                let ids: Vec<i32> = remove.entity_ids.iter().map(|id| id.0).collect();
                self.positions.retain(|(id, _)| !ids.contains(id));
                Seen::Remove { ids }
            }
            ClientboundGamePacket::ClientboundUpdateAttributes(update) => Seen::Attributes {
                id: update.entity_id.0,
                entries: update
                    .attributes
                    .iter()
                    .map(|entry| SeenEntry {
                        attribute: entry
                            .attribute
                            .resolve()
                            .expect("the server sends known ids"),
                        base: entry.base,
                        modifiers: entry
                            .modifiers
                            .iter()
                            .map(|modifier| modifier.id.to_string())
                            .collect(),
                    })
                    .collect(),
            },
            ClientboundGamePacket::ClientboundMoveEntityPos(step) => {
                self.step(step.entity_id.0, &step.delta);
                Seen::Move {
                    id: step.entity_id.0,
                }
            }
            ClientboundGamePacket::ClientboundEntityPositionSync(sync) => {
                let to = match &sync.position {
                    PositionPath::Linear(to) => *to,
                    PositionPath::Stepped { steps, .. } => steps.last().unwrap().position,
                };
                self.place(sync.entity_id.0, to);
                Seen::Sync {
                    id: sync.entity_id.0,
                    on_ground: sync.on_ground,
                }
            }
            ClientboundGamePacket::ClientboundSetEntityMotion(motion) => Seen::Motion {
                id: motion.entity_id.0,
            },
            _ => return,
        };
        self.seen.push(seen);
    }

    fn place(&mut self, id: i32, to: DVec3) {
        if let Some((_, at)) = self.positions.iter_mut().find(|(seen, _)| *seen == id) {
            *at = to;
        }
    }

    fn step(&mut self, id: i32, delta: &VecDelta) {
        let steps: Vec<[i16; 3]> = match delta {
            VecDelta::Linear(delta) => vec![*delta],
            VecDelta::Stepped { steps, .. } => steps.iter().map(|step| step.delta).collect(),
        };
        if let Some((_, at)) = self.positions.iter_mut().find(|(seen, _)| *seen == id) {
            for delta in steps {
                *at = position_delta::decode(*at, delta);
            }
        }
    }
}

fn start() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    PlayServer::start(app, &[(OPERATOR, 2)], 0)
}

pub(crate) struct MobEntity {
    pub(crate) entity: Entity,
    pub(crate) id: EntityId,
    pub(crate) state: EntityState,
    pub(crate) mob: Mob,
}

pub(crate) fn mob_entities(server: &mut PlayServer) -> Vec<MobEntity> {
    let world = server.world();
    let mut mobs: Vec<MobEntity> = world
        .query::<(Entity, &MotionId, &MotionState, &Mob)>()
        .iter(world)
        .map(|(entity, id, state, mob)| MobEntity {
            entity,
            id: id.0,
            state: state.0,
            mob: mob.clone(),
        })
        .collect();
    mobs.sort_by_key(|mob| mob.id);
    mobs
}

fn pig_rows(server: &mut PlayServer) -> Vec<DVec3> {
    mob_entities(server)
        .iter()
        .map(|mob| mob.state.body.position)
        .collect()
}

#[test]
fn trk_spn_001_summoned_pig_reaches_viewer_as_one_bundle_and_falls() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));

    let replies = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded("summon minecraft:pig 4 70 4"),
        },
    );
    let message = chats(&replies).pop().expect("the summon replies");
    let TextContent::Translate { translate, .. } = &message.content else {
        panic!("a translated reply: {message:?}");
    };
    assert_eq!(translate, "commands.summon.success");
    viewer.receive(replies);
    viewer.receive(server.ticks(operator, 60));

    let mobs = mob_entities(&mut server);
    assert_eq!(mobs.len(), 1, "one summon makes one mob");
    let id = mobs[0].id.0 as i32;
    let uuid = Uuid::from_u128(mobs[0].mob.uuid);
    assert_eq!(
        viewer.seen.get(..4),
        Some(
            &[
                Seen::Delimiter,
                Seen::Add {
                    id,
                    kind: EntityType::Pig,
                    uuid,
                    pos: DVec3::new(4.5, 70.0, 4.5),
                },
                Seen::Data {
                    id,
                    health: Some(10.0)
                },
                Seen::Delimiter,
            ][..]
        ),
        "{:#?}",
        viewer.seen
    );
    assert!(
        viewer.seen.contains(&Seen::Sync {
            id,
            on_ground: true
        }),
        "the landing sends the absolute position with the ground flag"
    );
    let rows = pig_rows(&mut server);
    assert_eq!(
        rows,
        [DVec3::new(4.5, 64.0, 4.5)],
        "the pig lies on the stone"
    );
    assert_eq!(
        viewer.positions,
        [(id, rows[0])],
        "the client shows the row"
    );
}

#[test]
fn atr_net_003_a_changed_instance_of_a_tracked_mob_reaches_its_viewer_once() {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins((VanillaEntityPhysicsPlugin, VanillaEntityAttributesPlugin));
    let mut server = PlayServer::start(app, &[(OPERATOR, 2)], 0);
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));
    run(
        &mut server,
        operator,
        &mut viewer,
        "summon minecraft:pig 4 64 4",
    );
    viewer.receive(server.ticks(operator, 5));
    let pig = &mob_entities(&mut server)[0];
    let (entity, id) = (pig.entity, pig.id.0 as i32);
    let attribute_updates = |viewer: &mut Viewer| -> Vec<Seen> {
        viewer
            .seen
            .drain(..)
            .filter(|seen| matches!(seen, Seen::Attributes { .. }))
            .collect()
    };
    let slow = |attribute| AttributeEdit {
        target: entity,
        attribute,
        edit: InstanceEdit::Put(InstanceModifier::new(
            "test:slow".parse().unwrap(),
            -0.5,
            ModifierOperation::AddMultipliedTotal,
            ModifierKind::Permanent,
        )),
    };
    assert_eq!(
        attribute_updates(&mut viewer),
        [],
        "a pig of its type has no entry"
    );

    server.world().write_message(slow(Attribute::MovementSpeed));
    viewer.receive(server.ticks(operator, 3));
    assert_eq!(
        attribute_updates(&mut viewer),
        [Seen::Attributes {
            id,
            entries: vec![SeenEntry {
                attribute: Attribute::MovementSpeed,
                base: 0.25,
                modifiers: vec!["test:slow".into()],
            }],
        }],
        "the next update holds the entry"
    );
    viewer.receive(server.ticks(operator, 10));
    assert_eq!(attribute_updates(&mut viewer), [], "the entry goes once");

    server.world().write_message(slow(Attribute::FollowRange));
    viewer.receive(server.ticks(operator, 10));
    assert_eq!(
        server
            .world()
            .get::<MobAttributes>(entity)
            .map(|attributes| attributes.set().instances().count()),
        Some(2),
        "the edit of follow_range applies"
    );
    assert_eq!(
        attribute_updates(&mut viewer),
        [],
        "follow_range is not syncable"
    );
}

fn translated(message: &Text) -> (String, Vec<String>) {
    let TextContent::Translate {
        translate, with, ..
    } = &message.content
    else {
        panic!("a translated message: {message:?}");
    };
    let args = with
        .iter()
        .map(|arg| match arg {
            TranslateArg::Text(text) => {
                mcrs_minecraft_text_flatten::plain_text(&Language::default(), text)
            }
            TranslateArg::Number(number) => format!("{number:?}"),
            TranslateArg::Bool(value) => value.to_string(),
        })
        .collect();
    (translate.to_string(), args)
}

fn run(
    server: &mut PlayServer,
    operator: Entity,
    viewer: &mut Viewer,
    line: &str,
) -> (String, Vec<String>) {
    let packets = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    let reply = chats(&packets)
        .pop()
        .unwrap_or_else(|| panic!("{line} replies"));
    viewer.receive(packets);
    translated(&reply)
}

#[test]
fn cmd_kill_summoned_pigs_are_found_hurt_killed_and_removed_after_their_death_time() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));
    for (index, x) in [2, 4, 6].into_iter().enumerate() {
        let line = format!(
            "summon minecraft:pig {x} 64 4 {{UUID:[I;7168627,0,0,{index}],CustomName:\"mbs{index}\",Tags:[\"bench\"]}}"
        );
        let (key, args) = run(&mut server, operator, &mut viewer, &line);
        assert_eq!(
            (key.as_str(), args),
            ("commands.summon.success", vec![format!("mbs{index}")]),
            "{line}"
        );
    }
    viewer.receive(server.ticks(operator, 5));

    assert_eq!(
        run(
            &mut server,
            operator,
            &mut viewer,
            "execute if entity @e[type=minecraft:pig,tag=bench]"
        ),
        (
            "commands.execute.conditional.pass_count".into(),
            vec!["3".into()]
        )
    );
    let (key, args) = run(
        &mut server,
        operator,
        &mut viewer,
        "data get entity @e[name=mbs1,limit=1] Pos",
    );
    assert_eq!(
        (key.as_str(), args[0].as_str()),
        ("commands.data.entity.query", "mbs1")
    );
    assert_eq!(args[1], "[4.5d, 64.0d, 4.5d]");
    let (_, args) = run(
        &mut server,
        operator,
        &mut viewer,
        "data get entity 006d6273-0000-0000-0000-000000000002 Pos",
    );
    assert_eq!(
        args,
        ["mbs2", "[6.5d, 64.0d, 4.5d]"],
        "a UUID target finds the mob"
    );
    let (key, _) = run(
        &mut server,
        operator,
        &mut viewer,
        "summon minecraft:pig 8 64 4 {UUID:[I;7168627,0,0,2]}",
    );
    assert_eq!(key, "commands.summon.failed.uuid");

    let (key, _) = run(
        &mut server,
        operator,
        &mut viewer,
        "execute as @e[type=minecraft:pig,name=mbs0] run damage @s 0 minecraft:cactus",
    );
    assert_eq!(key, "commands.damage.success");
    let (key, _) = run(
        &mut server,
        operator,
        &mut viewer,
        "damage 006d6273-0000-0000-0000-000000000000 0 minecraft:cactus",
    );
    assert_eq!(
        key, "commands.damage.invulnerable",
        "the cooldown rejects a second hit of 0"
    );

    let (key, args) = run(
        &mut server,
        operator,
        &mut viewer,
        "kill @e[type=minecraft:pig]",
    );
    assert_eq!(
        (key.as_str(), args),
        ("commands.kill.success.multiple", vec!["Int(3)".to_owned()])
    );
    let (key, _) = run(
        &mut server,
        operator,
        &mut viewer,
        "execute if entity @e[type=minecraft:pig]",
    );
    assert_eq!(
        key, "commands.execute.conditional.fail",
        "a dying pig is not selected"
    );
    viewer.receive(server.ticks(operator, 25));

    let mobs = mob_entities(&mut server).len();
    assert_eq!(mobs, 0, "the pigs left the world");
    assert!(pig_rows(&mut server).is_empty(), "and their rows");
    assert_each_death_in_order(&viewer);
}

#[test]
fn a_hit_in_the_cooldown_sends_the_lower_health_of_a_resting_pig() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));
    let (key, _) = run(
        &mut server,
        operator,
        &mut viewer,
        "summon minecraft:pig 4 64 4 {NoAI:1b}",
    );
    assert_eq!(key, "commands.summon.success");
    viewer.receive(server.ticks(operator, 5));
    let id = viewer
        .seen
        .iter()
        .find_map(|seen| match seen {
            Seen::Add { id, .. } => Some(*id),
            _ => None,
        })
        .expect("the viewer sees the pig");

    for line in [
        "damage @e[type=minecraft:pig,limit=1] 1 minecraft:cactus",
        "damage @e[type=minecraft:pig,limit=1] 3 minecraft:cactus",
    ] {
        let (key, _) = run(&mut server, operator, &mut viewer, line);
        assert_eq!(key, "commands.damage.success", "{line}");
    }
    viewer.receive(server.ticks(operator, 2));

    let hurts = viewer
        .seen
        .iter()
        .filter(|seen| **seen == Seen::Hurt { id })
        .count();
    assert_eq!(hurts, 1, "only the first hit sends damage_event");
    let healths: Vec<f32> = viewer
        .seen
        .iter()
        .filter_map(|seen| match seen {
            Seen::Data {
                id: seen,
                health: Some(health),
            } if *seen == id => Some(*health),
            _ => None,
        })
        .collect();
    assert_eq!(healths.last(), Some(&7.0), "{healths:?}");
}

fn assert_each_death_in_order(viewer: &Viewer) {
    let ids: Vec<i32> = viewer
        .seen
        .iter()
        .filter_map(|seen| match seen {
            Seen::Add { id, .. } => Some(*id),
            _ => None,
        })
        .collect();
    let hurt = ids[0];
    assert!(
        viewer.seen.contains(&Seen::Hurt { id: hurt }),
        "the hit sends damage_event"
    );
    for &id in &ids {
        let died = viewer
            .seen
            .iter()
            .position(|seen| *seen == Seen::Event { id, event: 3 });
        let dying = viewer.seen.iter().position(|seen| {
            *seen
                == Seen::Dying {
                    id,
                    health: Some(0.0),
                }
        });
        let poof = viewer
            .seen
            .iter()
            .position(|seen| *seen == Seen::Event { id, event: 60 });
        let removed = viewer
            .seen
            .iter()
            .position(|seen| matches!(seen, Seen::Remove { ids } if ids.contains(&id)));
        assert!(
            died < dying && dying < poof && poof < removed && died.is_some(),
            "{id}: the death event, the dying pose, the smoke, then the removal: {:?}",
            [died, dying, poof, removed]
        );
    }
    assert!(viewer.positions.is_empty(), "the client holds no pig");
}

#[test]
fn execute_as_sorted_entities_forks_players_and_mobs_in_one_order() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));
    for (offset, name) in [(2, "near"), (10, "far")] {
        let line = format!(
            "execute at @s run summon minecraft:pig ~{offset} ~ ~ {{CustomName:\"{name}\"}}"
        );
        let (key, _) = run(&mut server, operator, &mut viewer, &line);
        assert_eq!(key, "commands.summon.success", "{line}");
    }
    viewer.receive(server.ticks(operator, 1));

    let packets = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded("execute at @s positioned ~4 ~ ~ as @e[sort=nearest] run kill @s"),
        },
    );
    let names: Vec<Vec<String>> = chats(&packets)
        .iter()
        .map(|reply| translated(reply).1)
        .collect();
    assert_eq!(
        names,
        [["near"], ["op"], ["far"]],
        "the pig 2 blocks away, the player 4 blocks away, the pig 6 blocks away"
    );
}

#[test]
fn teleport_goes_to_a_mob_and_refuses_to_move_one() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));
    let (key, _) = run(
        &mut server,
        operator,
        &mut viewer,
        "summon minecraft:pig 6 64 4 {CustomName:\"dest\"}",
    );
    assert_eq!(key, "commands.summon.success");
    viewer.receive(server.ticks(operator, 1));

    let (key, args) = run(
        &mut server,
        operator,
        &mut viewer,
        "tp @s @e[type=minecraft:pig,limit=1]",
    );
    assert_eq!(
        (key.as_str(), args),
        (
            "commands.teleport.success.entity.single",
            vec!["op".to_owned(), "dest".to_owned()]
        )
    );

    let packets = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded("tp @e 0 64 0"),
        },
    );
    let reply = chats(&packets).pop().expect("the teleport replies");
    let TextContent::Text { text } = &reply.content else {
        panic!("a plain-text failure: {reply:?}");
    };
    assert_eq!(
        text.as_ref(),
        "teleport does not move or face a mob yet; only players can be victims or facing targets",
        "teleport refuses a mob victim"
    );
}

#[test]
fn kill_of_players_and_mobs_counts_each_victim() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    let mut viewer = Viewer::new();
    viewer.receive(server.ticks(operator, 2));
    for x in [2, 6] {
        let (key, _) = run(
            &mut server,
            operator,
            &mut viewer,
            &format!("summon minecraft:pig {x} 64 4"),
        );
        assert_eq!(key, "commands.summon.success");
    }
    viewer.receive(server.ticks(operator, 1));

    assert_eq!(
        run(&mut server, operator, &mut viewer, "kill @e"),
        (
            "commands.kill.success.multiple".to_owned(),
            vec!["Int(3)".to_owned()]
        )
    );
}

#[test]
fn two_summons_with_one_uuid_in_one_tick_make_one_mob() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    for x in [2, 6] {
        let line = format!("summon minecraft:pig {x} 64 4 {{UUID:[I;1,2,3,4]}}");
        server.queue(
            operator,
            &ServerboundChatCommand {
                command: Bounded(&line),
            },
        );
    }
    let packets = server.ticks(operator, 3);
    let keys: Vec<String> = chats(&packets)
        .iter()
        .map(|reply| translated(reply).0)
        .collect();
    assert_eq!(
        keys,
        ["commands.summon.success", "commands.summon.failed.uuid"],
        "the second summon answers the refusal"
    );
    assert_eq!(
        mob_entities(&mut server).len(),
        1,
        "the second pig with the UUID does not spawn"
    );
}
