use bevy_app::{App, FixedUpdate, Last};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use bevy_time::TimeUpdateStrategy;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_damage::{Defence, Vitals};
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClockPlugin;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_engine_damage::{DamageSet, PendingHits};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundDamageEvent, ClientboundSetEntityMotion,
};
use mcrs_minecraft_protocol_game::serverbound::{
    PlayerCommandAction, ServerboundAttack, ServerboundChatCommand, ServerboundPlayerCommand,
    ServerboundPlayerLoaded,
};
use mcrs_minecraft_replication::wire_id;
use mcrs_minecraft_server::{
    VanillaEntityAttributesPlugin, VanillaEntityPhysicsPlugin, VanillaPlayerAttackPlugin,
};

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer, replicated, sent_to};
use crate::mob_replication::mob_entities;

const ALICE: Uuid = Uuid::from_u128(111);
const BOB: Uuid = Uuid::from_u128(222);

#[derive(Component)]
struct Cancelled;

fn cancel_hits(mut pending: ResMut<PendingHits>, cancelled: Query<(), With<Cancelled>>) {
    pending.retain(|request| !cancelled.contains(request.target));
}

fn cancel_plugin(app: &mut App) {
    app.add_systems(FixedUpdate, cancel_hits.in_set(DamageSet::Filter));
}

#[derive(Resource, Default)]
struct Captured(Vec<OutboundPlayerPacket>);

fn capture(mut packets: MessageReader<OutboundPlayerPacket>, mut captured: ResMut<Captured>) {
    captured.0.extend(packets.read().cloned());
}

fn capture_plugin(app: &mut App) {
    app.init_resource::<Captured>();
    app.add_systems(Last, capture);
}

struct Arena {
    server: PlayServer,
    alice: Entity,
    bob: Entity,
    alice_player: Entity,
    bob_player: Entity,
}

impl Arena {
    fn start() -> Self {
        let mut app = host_app::make_host_app();
        app.add_plugins(BirthClockPlugin);
        app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        app.add_plugins(CommandPlugin::<VanillaCommands>::new());
        app.add_plugins(VanillaCommandsPlugin);
        app.add_plugins((
            VanillaEntityPhysicsPlugin,
            VanillaEntityAttributesPlugin,
            VanillaPlayerAttackPlugin,
        ));
        app.add_dimension_plugin(cancel_plugin);
        app.add_dimension_plugin(capture_plugin);
        let mut server = PlayServer::start(app, &[(ALICE, 2)], 0);
        load_stone_column(&mut server);
        let (alice, _) = server.join(ALICE, "Alice");
        let (bob, _) = server.join(BOB, "Bob");
        server.queue(alice, &ServerboundPlayerLoaded);
        server.queue(bob, &ServerboundPlayerLoaded);
        let _ = server.ticks_all(2);
        let alice_player = server.player(alice);
        let bob_player = server.player(bob);
        server
            .world()
            .get_mut::<Transform>(bob_player)
            .unwrap()
            .translation = DVec3::new(8.5, 64.0, 10.5);
        Self {
            server,
            alice,
            bob,
            alice_player,
            bob_player,
        }
    }

    fn set_mode(&mut self, player: Entity, mode: GameMode) {
        self.server
            .world()
            .get_mut::<PlayerGameMode>(player)
            .unwrap()
            .0 = mode;
    }

    fn summon_pig(&mut self, at: DVec3) -> (Entity, i32) {
        let command = format!(
            "summon minecraft:pig {} {} {} {{NoAI:1b}}",
            at.x, at.y, at.z
        );
        self.server.queue(
            self.alice,
            &ServerboundChatCommand {
                command: Bounded(&command),
            },
        );
        let _ = self.server.ticks_all(10);
        let pig = mob_entities(&mut self.server)
            .pop()
            .expect("the summon makes a pig");
        (pig.entity, wire_id(pig.id).0)
    }

    fn health(&mut self, entity: Entity) -> f32 {
        self.server.world().get::<Vitals>(entity).unwrap().health
    }

    fn attack(&mut self, id: i32) -> Vec<PacketPayload> {
        self.server.queue(
            self.alice,
            &ServerboundAttack {
                entity_id: VarInt(id),
            },
        );
        let session = self.server.session_of(self.bob).unwrap();
        sent_to(session, self.server.ticks_all(3))
    }

    fn start_sprint(&mut self) {
        let session = self.server.session_of(self.alice).unwrap();
        self.server.queue(
            self.alice,
            &ServerboundPlayerCommand {
                entity_id: VarInt(player_network_id(session)),
                action: PlayerCommandAction::StartSprinting,
                data: VarInt(0),
            },
        );
    }

    fn alice_disconnected(&mut self) -> bool {
        let session = self.server.session_of(self.alice).unwrap();
        std::mem::take(&mut self.server.world().resource_mut::<Captured>().0)
            .iter()
            .any(|packet| {
                matches!(
                    (&packet.target, &packet.data),
                    (PacketTarget::SinglePlayer(target), PacketPayload::Disconnect(_))
                        if *target == session
                )
            })
    }
}

fn decoded<P: Packet + for<'a> Decode<'a>>(packets: &[PacketPayload]) -> Vec<P> {
    replicated(packets)
        .into_iter()
        .filter(|(id, _)| *id == P::ID)
        .map(|(_, bytes)| P::decode(&mut bytes.as_slice()).unwrap())
        .collect()
}

#[test]
fn a_full_strength_fist_hit_hurts_and_knocks_back_a_pig() {
    let mut arena = Arena::start();
    arena.set_mode(arena.alice_player, GameMode::Survival);
    let (pig, id) = arena.summon_pig(DVec3::new(8.5, 64.0, 10.5));
    let before = arena.health(pig);

    let seen = arena.attack(id);

    assert_eq!(arena.health(pig), before - 1.0, "an empty hand deals 1");
    let hurt: Vec<i32> = decoded::<ClientboundDamageEvent>(&seen)
        .iter()
        .map(|event| event.entity_id.0)
        .collect();
    assert_eq!(hurt, [id], "the viewer sees one damage_event of the pig");
    let pushed = decoded::<ClientboundSetEntityMotion>(&seen)
        .into_iter()
        .find(|motion| motion.entity_id.0 == id)
        .expect("the viewer sees the knockback of the pig")
        .velocity
        .0;
    assert!(
        pushed.x == 0.0 && pushed.y > 0.0 && pushed.z > 0.0,
        "a hit from the north lifts the pig and pushes it south: {pushed}"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gait {
    Walk,
    Sprint,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Bob { pvp: bool, gait: Gait },
    Herself,
    Pig,
    InvulnerablePig,
    FarPig,
    CancelledPig,
    UnknownId,
}

struct Case {
    name: &'static str,
    attacker: GameMode,
    target: Target,
    lost: f32,
}

const CASES: [Case; 10] = [
    Case {
        name: "a player with pvp off",
        attacker: GameMode::Survival,
        target: Target::Bob {
            pvp: false,
            gait: Gait::Walk,
        },
        lost: 0.0,
    },
    Case {
        name: "a player with pvp on",
        attacker: GameMode::Survival,
        target: Target::Bob {
            pvp: true,
            gait: Gait::Walk,
        },
        lost: 1.0,
    },
    Case {
        name: "a player with pvp on hit by a sprint attack",
        attacker: GameMode::Survival,
        target: Target::Bob {
            pvp: true,
            gait: Gait::Sprint,
        },
        lost: 1.0,
    },
    Case {
        name: "an invulnerable pig hit in survival",
        attacker: GameMode::Survival,
        target: Target::InvulnerablePig,
        lost: 0.0,
    },
    Case {
        name: "an invulnerable pig hit in creative",
        attacker: GameMode::Creative,
        target: Target::InvulnerablePig,
        lost: 1.0,
    },
    Case {
        name: "a pig hit by a spectator",
        attacker: GameMode::Spectator,
        target: Target::Pig,
        lost: 0.0,
    },
    Case {
        name: "a pig out of reach",
        attacker: GameMode::Survival,
        target: Target::FarPig,
        lost: 0.0,
    },
    Case {
        name: "a pig whose hit a filter cancels",
        attacker: GameMode::Survival,
        target: Target::CancelledPig,
        lost: 0.0,
    },
    Case {
        name: "the attacker herself",
        attacker: GameMode::Survival,
        target: Target::Herself,
        lost: 0.0,
    },
    Case {
        name: "an id that names no entity",
        attacker: GameMode::Survival,
        target: Target::UnknownId,
        lost: 0.0,
    },
];

#[test]
fn the_server_accepts_or_refuses_an_attack() {
    let mut arena = Arena::start();
    arena.set_mode(arena.bob_player, GameMode::Survival);
    let mut walk_push = None;
    for (index, case) in CASES.iter().enumerate() {
        arena.set_mode(arena.alice_player, case.attacker);
        let near = DVec3::new(6.5 + index as f64, 64.0, 10.5);
        let (target, id) = match case.target {
            Target::Bob { pvp, .. } => {
                arena
                    .server
                    .world()
                    .resource_mut::<GameRules>()
                    .set(GameRule::Pvp, i32::from(pvp));
                let session = arena.server.session_of(arena.bob).unwrap();
                (arena.bob_player, player_network_id(session))
            }
            Target::Herself => {
                let session = arena.server.session_of(arena.alice).unwrap();
                (arena.alice_player, player_network_id(session))
            }
            Target::Pig => arena.summon_pig(near),
            Target::FarPig => arena.summon_pig(DVec3::new(14.5, 64.0, 15.5)),
            Target::InvulnerablePig => {
                let pig = arena.summon_pig(near);
                arena
                    .server
                    .world()
                    .get_mut::<Defence>(pig.0)
                    .unwrap()
                    .invulnerable = true;
                pig
            }
            Target::CancelledPig => {
                let pig = arena.summon_pig(near);
                arena.server.world().entity_mut(pig.0).insert(Cancelled);
                pig
            }
            Target::UnknownId => (arena.summon_pig(near).0, i32::MAX),
        };
        let _ = arena.server.ticks_all(10);
        let before = arena.health(target);

        if let Target::Bob {
            gait: Gait::Sprint, ..
        } = case.target
        {
            arena.start_sprint();
        }
        let _ = arena.alice_disconnected();
        let to_bob = arena.attack(id);
        let hurt = decoded::<ClientboundDamageEvent>(&to_bob).len();

        assert_eq!(before - arena.health(target), case.lost, "{}", case.name);
        assert_eq!(hurt, usize::from(case.lost > 0.0), "{}", case.name);
        assert_eq!(
            arena.alice_disconnected(),
            matches!(case.target, Target::Herself),
            "{}",
            case.name
        );
        if let Target::Bob { pvp: true, gait } = case.target {
            let pushed = to_bob
                .iter()
                .filter_map(|payload| match payload {
                    PacketPayload::OwnEntityState(bytes) => {
                        let mut bytes = bytes.as_ref();
                        let id = VarInt::decode(&mut bytes).unwrap().0;
                        (id == ClientboundSetEntityMotion::ID)
                            .then(|| ClientboundSetEntityMotion::decode(&mut bytes).unwrap())
                    }
                    _ => None,
                })
                .next()
                .expect("bob receives his knockback")
                .velocity
                .0
                .z;
            match gait {
                Gait::Walk => {
                    assert!(pushed > 0.0, "{}: {pushed} <= 0", case.name);
                    walk_push = Some(pushed);
                }
                Gait::Sprint => {
                    let walk = walk_push.expect("the walk row comes first");
                    assert!(pushed > walk, "{}: {pushed} <= {walk}", case.name);
                    let sprinting = arena
                        .server
                        .world()
                        .get::<PlayerSprinting>(arena.alice_player)
                        .unwrap()
                        .0;
                    assert!(!sprinting, "{}: the sprint ends", case.name);
                }
            }
        }
    }
}
