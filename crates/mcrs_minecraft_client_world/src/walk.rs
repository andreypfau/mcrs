use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::{Transform as PhysicsTransform, Velocity};
use mcrs_minecraft_entity_visual::SyncedData;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_physics::{
    BlockAction, Keys, NoBodies, Orientation, PlayerFacts, PlayerInput, Scratch, Side, View,
    tick_player,
};

use crate::collision_tables::ClientPhysicsTables;
use crate::collision_view::ClientBlocks;
use crate::columns::ColumnStore;
use crate::game_mode::LocalGameMode;
use crate::local_motion::LocalMotion;
use crate::local_player::Sprint;
use crate::movement_input::MovementInput;
use crate::player::Player;

#[derive(Default)]
pub(crate) struct WalkBuffers {
    scratch: Scratch,
    actions: Vec<BlockAction>,
}

pub(crate) fn walk(
    input: Res<MovementInput>,
    columns: Res<ColumnStore>,
    tables: Res<ClientPhysicsTables>,
    mut buffers: Local<WalkBuffers>,
    player: Single<
        (
            &mut LocalMotion,
            &mut PhysicsTransform,
            &mut Velocity,
            &mut Sprint,
            Option<&LocalGameMode>,
            &SyncedData,
        ),
        With<Player>,
    >,
) {
    let (mut motion, mut transform, mut velocity, mut sprint, game_mode, synced) =
        player.into_inner();
    let tables = tables.tables();
    let world = ClientBlocks::new(&*columns, tables);
    let player_input = PlayerInput {
        keys: keys(*input),
        rotation: Orientation::facing(transform.rotation.yaw(), transform.rotation.pitch()),
        facts: PlayerFacts {
            spectator: game_mode.is_some_and(|mode| mode.current == GameMode::Spectator),
            sleeping: synced.sleeping_position.is_some(),
            fall_flying: synced.is_fall_flying(),
            spin_attack: synced.is_spin_attacking(),
            ..PlayerFacts::default()
        },
        push: DVec3::ZERO,
    };
    let start = motion.0;
    let mut next = start;
    let WalkBuffers { scratch, actions } = &mut *buffers;
    tick_player(
        View::new(&world, &NoBodies, tables),
        Side::OwnClient,
        &start,
        &player_input,
        &mut next,
        actions,
        scratch,
    );
    actions.clear();
    motion.set_if_neq(LocalMotion(next));
    if transform.translation != next.body.position {
        transform.translation = next.body.position;
    }
    if velocity.0 != next.body.velocity {
        velocity.0 = next.body.velocity;
    }
    sprint.set_if_neq(Sprint {
        active: next.sprint0,
    });
}

fn keys(input: MovementInput) -> Keys {
    Keys {
        forward: input.forward,
        back: input.backward,
        left: input.left,
        right: input.right,
        jump: input.jump,
        shift: input.shift,
        sprint: input.sprint,
        auto_jump: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_session::{FLOOR_TOP, OWN_PLAYER, TestSession, login, receive};
    use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
    use mcrs_minecraft_protocol::VarInt;
    use mcrs_minecraft_protocol_game::clientbound::{
        AttributeSnapshot, ClientboundPlayerAbilities, ClientboundUpdateAttributes,
    };

    fn logged_in(position: DVec3, game_mode: GameMode) -> TestSession {
        let mut session = TestSession::new(position);
        session.receive(login(game_mode));
        session
    }

    const FORWARD: MovementInput = MovementInput {
        forward: true,
        ..MovementInput::EMPTY
    };

    fn walked(input: MovementInput, base: f64) -> f64 {
        let mut session = logged_in(DVec3::new(8.5, FLOOR_TOP, 2.5), GameMode::Survival);
        session.receive(ClientboundUpdateAttributes {
            entity_id: VarInt(OWN_PLAYER),
            attributes: vec![AttributeSnapshot {
                attribute: Attribute::MovementSpeed.into(),
                base,
                modifiers: Vec::new(),
            }],
        });
        session.ticks(MovementInput::EMPTY, 3);
        assert!(session.motion().body.flags.on_ground, "the player stands");
        let start = session.position();
        session.ticks(input, 20);
        let end = session.position();
        assert_eq!(end.y, FLOOR_TOP, "the player stays on the floor");
        end.z - start.z
    }

    #[test]
    fn the_speed_entry_and_the_sprint_key_lengthen_a_walk() {
        let base = f64::from(0.1_f32);
        let walking = walked(FORWARD, base);
        let faster = walked(FORWARD, 0.2);
        let sprinting = walked(
            MovementInput {
                sprint: true,
                ..FORWARD
            },
            base,
        );
        assert!(walking > 0.0, "{walking}");
        assert!(faster > walking, "{faster} > {walking}");
        assert!(sprinting > walking, "{sprinting} > {walking}");
    }

    #[test]
    fn the_first_step_on_stone_adds_the_input_times_the_movement_speed() {
        let mut session = logged_in(DVec3::new(8.5, FLOOR_TOP, 2.5), GameMode::Survival);
        session.ticks(MovementInput::EMPTY, 3);
        let start = session.position();
        session.ticks(FORWARD, 1);
        let step = session.position().z - start.z;
        let expected = f64::from(0.98_f32) * f64::from(0.1_f32);
        assert_eq!(step.to_bits(), expected.to_bits(), "{step} != {expected}");
    }

    #[test]
    fn a_flying_creative_player_holds_its_height_where_a_survival_player_falls() {
        let above = DVec3::new(8.5, FLOOR_TOP + 4.0, 8.5);
        let mut creative = logged_in(above, GameMode::Creative);
        creative.receive(ClientboundPlayerAbilities {
            invulnerable: true,
            flying: true,
            may_fly: true,
            instant_build: true,
            flying_speed: 0.05_f32,
            walking_speed: 0.1_f32,
        });
        creative.ticks(MovementInput::EMPTY, 10);
        assert_eq!(creative.position(), above);
        assert!(creative.motion().flight0);

        let mut survival = logged_in(above, GameMode::Survival);
        survival.ticks(MovementInput::EMPTY, 10);
        assert!(survival.position().y < above.y, "{}", survival.position());
        assert!(!survival.motion().flight0);
    }

    #[cfg(not(target_family = "wasm"))]
    mod wire {
        use super::*;
        use crate::test_session::{Joined, Server, joined};
        use mcrs_minecraft_core::Look;
        use mcrs_minecraft_pose::Pose;
        use mcrs_minecraft_protocol::Packet;
        use mcrs_minecraft_protocol_game::ColumnPos;
        use mcrs_minecraft_protocol_game::PositionFlags;
        use mcrs_minecraft_protocol_game::clientbound::{
            ClientboundGameEvent, ClientboundPlayerPosition,
        };
        use mcrs_minecraft_protocol_game::game_event::GameEventKind;
        use mcrs_minecraft_protocol_game::serverbound::{
            ServerboundClientTickEnd, ServerboundMovePlayerPos, ServerboundPlayerInput,
            ServerboundPlayerLoaded,
        };

        const LEVEL_CHUNKS_LOAD_START: ClientboundGameEvent = ClientboundGameEvent {
            game_event: GameEventKind::LevelChunksLoadStart,
        };

        #[derive(Debug, PartialEq)]
        struct SentFrame {
            tick_end: u8,
            id: i32,
            body: Vec<u8>,
        }

        fn frames_before_tick_ends(
            server: &mut Server,
            tick_ends: u8,
            ids: &[i32],
        ) -> Vec<SentFrame> {
            let mut sent = Vec::new();
            let mut tick_end = 0;
            while tick_end < tick_ends {
                let frame = server.next_frame();
                if frame.id == ServerboundClientTickEnd::ID {
                    tick_end += 1;
                } else if ids.contains(&frame.id) {
                    sent.push(SentFrame {
                        tick_end,
                        id: frame.id,
                        body: frame.body.to_vec(),
                    });
                }
            }
            sent
        }

        #[test]
        fn a_held_shift_key_goes_to_the_server_once_and_crouches_the_local_player() {
            let session = TestSession::new(DVec3::new(8.5, FLOOR_TOP, 2.5));
            let player = session.player;
            let Joined {
                mut app,
                mut server,
                connection,
            } = joined(session.app);
            receive(&mut app, connection, login(GameMode::Survival));
            receive(&mut app, connection, LEVEL_CHUNKS_LOAD_START);
            app.insert_resource(MovementInput {
                shift: true,
                ..MovementInput::EMPTY
            });
            for _ in 0..3 {
                app.world_mut().run_schedule(FixedUpdate);
            }
            app.update();
            let pose = app.world().get::<LocalMotion>(player).unwrap().0.pose;
            assert_eq!(pose, Pose::Crouching);

            assert_eq!(
                frames_before_tick_ends(&mut server, 3, &[ServerboundPlayerInput::ID]),
                vec![SentFrame {
                    tick_end: 0,
                    id: ServerboundPlayerInput::ID,
                    body: vec![0x20],
                }],
                "only the first tick has a change"
            );
        }

        #[test]
        fn a_shift_key_held_through_the_load_goes_out_after_player_loaded() {
            let session = TestSession::new(DVec3::new(8.5, FLOOR_TOP, 2.5));
            let Joined {
                mut app,
                mut server,
                connection,
            } = joined(session.app);
            let spawn_column = ColumnPos::new(0, 0);
            let column = {
                let mut columns = app.world_mut().resource_mut::<ColumnStore>();
                let column = columns.get(spawn_column).unwrap().as_ref().clone();
                columns.remove(spawn_column);
                column
            };
            receive(&mut app, connection, login(GameMode::Survival));
            app.insert_resource(MovementInput {
                shift: true,
                ..MovementInput::EMPTY
            });
            app.world_mut().run_schedule(FixedUpdate);
            receive(&mut app, connection, LEVEL_CHUNKS_LOAD_START);
            app.world_mut().run_schedule(FixedUpdate);
            app.world_mut()
                .resource_mut::<ColumnStore>()
                .insert(spawn_column, column);
            app.world_mut().run_schedule(FixedUpdate);
            app.update();

            assert_eq!(
                frames_before_tick_ends(
                    &mut server,
                    3,
                    &[ServerboundPlayerLoaded::ID, ServerboundPlayerInput::ID],
                ),
                vec![
                    SentFrame {
                        tick_end: 2,
                        id: ServerboundPlayerLoaded::ID,
                        body: Vec::new(),
                    },
                    SentFrame {
                        tick_end: 2,
                        id: ServerboundPlayerInput::ID,
                        body: vec![0x20],
                    },
                ],
                "the load ends in the tick in which the spawn column arrives"
            );
        }

        #[test]
        fn a_dropped_player_lands_and_its_move_packet_carries_the_ground_flag() {
            let session = TestSession::new(DVec3::ZERO);
            let player = session.player;
            let Joined {
                mut app,
                mut server,
                connection,
            } = joined(session.app);
            receive(&mut app, connection, login(GameMode::Survival));
            receive(
                &mut app,
                connection,
                ClientboundPlayerPosition {
                    teleport_id: VarInt(1),
                    position: DVec3::new(8.5, FLOOR_TOP + 1.0, 8.5),
                    velocity: DVec3::ZERO,
                    look: Look {
                        yaw: 0.0,
                        pitch: 0.0,
                    },
                    flags: PositionFlags::default(),
                },
            );
            let mut ticks = 0;
            while ticks < 40 {
                app.world_mut().run_schedule(FixedUpdate);
                ticks += 1;
                let motion = app.world().get::<LocalMotion>(player).unwrap().0;
                if motion.body.flags.on_ground {
                    break;
                }
            }
            app.update();
            let landed_at = app.world().get::<PhysicsTransform>(player).unwrap();
            assert_eq!(landed_at.translation.y, FLOOR_TOP, "the player lands");

            let mut ground_flags = Vec::new();
            let mut tick_ends = 0;
            while tick_ends < ticks {
                let frame = server.next_frame();
                if frame.id == ServerboundMovePlayerPos::ID {
                    let flags = *frame.body.last().unwrap();
                    ground_flags.push(flags & 0x01 != 0);
                } else if frame.id == ServerboundClientTickEnd::ID {
                    tick_ends += 1;
                }
            }
            let (landed, falling) = ground_flags.split_last().expect("the player moves");
            assert!(*landed, "the landing move has the ground flag");
            assert!(!falling.is_empty() && falling.iter().all(|ground| !ground));
        }
    }
}
