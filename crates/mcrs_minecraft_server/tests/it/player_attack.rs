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
const DEFAULT_KNOCKBACK: f64 = 0.4000000059604645;

#[derive(Component)]
struct Cancelled;

fn cancel_hits(mut pending: ResMut<PendingHits>, cancelled: Query<(), With<Cancelled>>) {
    pending.retain(|request| !cancelled.contains(request.target));
}

fn cancel_plugin(app: &mut App) {
    app.add_systems(FixedUpdate, cancel_hits.in_set(DamageSet::PacketFilter));
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

    fn summon_pig(&mut self, at: DVec3, summoned: Summoned) -> (Entity, i32) {
        let data = match summoned {
            Summoned::NoAiOnGround => "NoAI:1b,OnGround:1b",
            Summoned::NoAi => "NoAI:1b",
            Summoned::Ai => "",
        };
        let command = format!("summon minecraft:pig {} {} {} {{{data}}}", at.x, at.y, at.z);
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

    fn target(&mut self, target: Target, near: DVec3) -> (Entity, i32) {
        match target {
            Target::Bob { pvp, .. } => {
                self.server
                    .world()
                    .resource_mut::<GameRules>()
                    .set(GameRule::Pvp, i32::from(pvp));
                let session = self.server.session_of(self.bob).unwrap();
                (self.bob_player, player_network_id(session))
            }
            Target::Herself => {
                let session = self.server.session_of(self.alice).unwrap();
                (self.alice_player, player_network_id(session))
            }
            Target::Pig(summoned) => self.summon_pig(near, summoned),
            Target::DyingPig => {
                let pig = self.summon_pig(near, Summoned::NoAi);
                self.server.world().get_mut::<Vitals>(pig.0).unwrap().health = 1.0;
                pig
            }
            Target::FarPig => self.summon_pig(DVec3::new(14.5, 64.0, 15.5), Summoned::NoAi),
            Target::InvulnerablePig => {
                let pig = self.summon_pig(near, Summoned::NoAi);
                self.server
                    .world()
                    .get_mut::<Defence>(pig.0)
                    .unwrap()
                    .invulnerable = true;
                pig
            }
            Target::CancelledPig => {
                let pig = self.summon_pig(near, Summoned::NoAi);
                self.server.world().entity_mut(pig.0).insert(Cancelled);
                pig
            }
            Target::UnknownId => (self.summon_pig(near, Summoned::NoAi).0, i32::MAX),
        }
    }

    fn health(&mut self, entity: Entity) -> f32 {
        self.server.world().get::<Vitals>(entity).unwrap().health
    }

    fn attack(&mut self, target: Entity, id: i32) -> Attack {
        self.server.queue(
            self.alice,
            &ServerboundAttack {
                entity_id: VarInt(id),
            },
        );
        let session = self.server.session_of(self.bob).unwrap();
        let before = *self.server.world().get::<Vitals>(target).unwrap();
        let mut packets = Vec::new();
        let mut after_hit = None;
        for _ in 0..3 {
            packets.extend(self.server.ticks_all(1));
            let now = *self.server.world().get::<Vitals>(target).unwrap();
            if after_hit.is_none() && now.health != before.health {
                after_hit = Some(now);
            }
        }
        Attack {
            seen: sent_to(session, packets),
            after_hit,
        }
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

struct Attack {
    seen: Vec<PacketPayload>,
    after_hit: Option<Vitals>,
}

fn decoded<P: Packet + for<'a> Decode<'a>>(packets: &[PacketPayload]) -> Vec<P> {
    replicated(packets)
        .into_iter()
        .filter(|(id, _)| *id == P::ID)
        .map(|(_, bytes)| P::decode(&mut bytes.as_slice()).unwrap())
        .collect()
}

fn assert_pushed_away(
    arena: &mut Arena,
    name: &str,
    id: i32,
    at: DVec3,
    summoned: Summoned,
    seen: &[PacketPayload],
) {
    let pushed = decoded::<ClientboundSetEntityMotion>(seen)
        .into_iter()
        .find(|motion| motion.entity_id.0 == id)
        .expect("the viewer sees the knockback of the pig")
        .velocity
        .0;
    let alice = arena
        .server
        .world()
        .get::<Transform>(arena.alice_player)
        .unwrap()
        .translation;
    let away = (at - alice).with_y(0.0).normalize();
    let (horizontal, lift) = match summoned {
        Summoned::NoAiOnGround => (DEFAULT_KNOCKBACK, 0.4),
        Summoned::NoAi => (DEFAULT_KNOCKBACK, 0.0),
        Summoned::Ai => (0.2184000286221508, 0.2751840104494096),
    };
    let expected = away * horizontal + DVec3::Y * lift;
    assert!(
        pushed.abs_diff_eq(expected, 1.0E-4),
        "{name}: ATK-KB-005 sends the velocity directly after the knockback: {pushed} != {expected}"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gait {
    Walk,
    Sprint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Summoned {
    NoAiOnGround,
    NoAi,
    Ai,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Bob { pvp: bool, gait: Gait },
    Herself,
    Pig(Summoned),
    DyingPig,
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
    ends: Ends,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ends {
    Unchecked,
    Hurt,
    Dying,
}

const CASES: [Case; 14] = [
    Case {
        name: "a pig with NoAI and OnGround hit by an empty hand at full strength",
        attacker: GameMode::Survival,
        target: Target::Pig(Summoned::NoAiOnGround),
        lost: 1.0,
        ends: Ends::Hurt,
    },
    Case {
        name: "a pig with AI at rest hit by an empty hand at full strength",
        attacker: GameMode::Survival,
        target: Target::Pig(Summoned::Ai),
        lost: 1.0,
        ends: Ends::Hurt,
    },
    Case {
        name: "a pig with NoAI and no OnGround hit by an empty hand at full strength",
        attacker: GameMode::Survival,
        target: Target::Pig(Summoned::NoAi),
        lost: 1.0,
        ends: Ends::Hurt,
    },
    Case {
        name: "a pig with one health point killed by an empty hand",
        attacker: GameMode::Survival,
        target: Target::DyingPig,
        lost: 1.0,
        ends: Ends::Dying,
    },
    Case {
        name: "a player with pvp off",
        attacker: GameMode::Survival,
        target: Target::Bob {
            pvp: false,
            gait: Gait::Walk,
        },
        lost: 0.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "a player with pvp on",
        attacker: GameMode::Survival,
        target: Target::Bob {
            pvp: true,
            gait: Gait::Walk,
        },
        lost: 1.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "a player with pvp on hit by a sprint attack",
        attacker: GameMode::Survival,
        target: Target::Bob {
            pvp: true,
            gait: Gait::Sprint,
        },
        lost: 1.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "an invulnerable pig hit in survival",
        attacker: GameMode::Survival,
        target: Target::InvulnerablePig,
        lost: 0.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "an invulnerable pig hit in creative",
        attacker: GameMode::Creative,
        target: Target::InvulnerablePig,
        lost: 1.0,
        ends: Ends::Hurt,
    },
    Case {
        name: "a pig hit by a spectator",
        attacker: GameMode::Spectator,
        target: Target::Pig(Summoned::NoAi),
        lost: 0.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "a pig out of reach",
        attacker: GameMode::Survival,
        target: Target::FarPig,
        lost: 0.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "a pig whose hit a filter cancels",
        attacker: GameMode::Survival,
        target: Target::CancelledPig,
        lost: 0.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "the attacker herself",
        attacker: GameMode::Survival,
        target: Target::Herself,
        lost: 0.0,
        ends: Ends::Unchecked,
    },
    Case {
        name: "an id that names no entity",
        attacker: GameMode::Survival,
        target: Target::UnknownId,
        lost: 0.0,
        ends: Ends::Unchecked,
    },
];

fn assert_ends(case: &Case, after_hit: Option<Vitals>) {
    match case.ends {
        Ends::Unchecked => {}
        Ends::Hurt => {
            let vitals = after_hit.expect("the hit changes the health");
            assert_eq!(
                (vitals.hurt_time, vitals.damage_cooldown),
                (9, 19),
                "{}: ATK-ORD-005 counts the hit down in its tick",
                case.name
            );
        }
        Ends::Dying => {
            let vitals = after_hit.expect("the hit changes the health");
            assert_eq!(
                vitals.death_time,
                Some(1),
                "{}: ATK-ORD-005 counts the death in its tick",
                case.name
            );
        }
    }
}

#[test]
fn the_server_accepts_or_refuses_an_attack() {
    let mut arena = Arena::start();
    arena.set_mode(arena.bob_player, GameMode::Survival);
    let mut walk_push = None;
    for (index, case) in CASES.iter().enumerate() {
        arena.set_mode(arena.alice_player, case.attacker);
        let near = DVec3::new(5.5 + index as f64, 64.0, 10.5);
        let (target, id) = arena.target(case.target, near);
        let _ = arena.server.ticks_all(10);
        let before = arena.health(target);

        if let Target::Bob {
            gait: Gait::Sprint, ..
        } = case.target
        {
            arena.start_sprint();
        }
        let _ = arena.alice_disconnected();
        let Attack {
            seen: to_bob,
            after_hit,
        } = arena.attack(target, id);
        let hurt = decoded::<ClientboundDamageEvent>(&to_bob).len();

        assert_eq!(before - arena.health(target), case.lost, "{}", case.name);
        assert_eq!(hurt, usize::from(case.lost > 0.0), "{}", case.name);
        assert_eq!(
            arena.alice_disconnected(),
            matches!(case.target, Target::Herself),
            "{}",
            case.name
        );
        assert_ends(case, after_hit);
        if let Target::Pig(summoned) = case.target
            && case.lost > 0.0
        {
            assert_pushed_away(&mut arena, case.name, id, near, summoned, &to_bob);
        }
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
