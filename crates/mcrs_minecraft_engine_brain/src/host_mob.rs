use glam::DVec3;
use mcrs_minecraft_brain::ControlFlags;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::DamageRecord;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_engine_physics::MobIntent;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_goals::STROLL_IDLE_LIMIT;
use mcrs_minecraft_mob_control::BodyData;
use mcrs_minecraft_mob_control::BodyFlags;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::LookDefaults;
use mcrs_minecraft_mob_control::MobControl;
use mcrs_minecraft_mob_control::PathEnd;
use mcrs_minecraft_mob_control::Rotations;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::LastHit;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::Steering;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockPos;

use crate::BlockGoalWorld;
use crate::HotTick;

/// A player nearer than this resets the idle time of a mob (AIT-VAN-020).
const NEAR_PLAYER: f64 = 32.0;

/// One mob of the stage on the side of the hot tick: its controls and the state that its view
/// holds besides its row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HostMob {
    pub(crate) id: EntityId,
    pub(crate) key: EntityKey,
    kind: &'static EntityTypeData,
    head_yaw_limit: f32,
    control: MobControl,
    /// The input of the last step before the travel: the body rotation of that step runs at the
    /// start of the next tick, since the pass has no step after the travel.
    travelled: Option<ControlInput>,
    /// The head yaw and the body yaw of that body rotation, which the next intent writes.
    turned: Option<Rotations>,
    position: DVec3,
    rotations: Rotations,
    idle_time: u32,
    last_hit: Option<LastHit>,
    /// The strike count of the mob at the last observe (GOAL-STR-004).
    strikes: u16,
    damage: Option<DamageRecord>,
    panic: Panic,
    /// The tick in which the life cycle last found the mob.
    pub(crate) seen: u64,
    /// The age of the row at the last observe. The pass adds 1 in each tick in which the row
    /// ticks, so an equal age at the next observe means a tick with no step.
    age: Option<u32>,
    /// The row did not tick in the tick before the last observe.
    stalled: bool,
    /// What the controls of this tick read of the row, from [`HostMob::observe`]; `None` for a
    /// row that is not a living row, or a dying one.
    row: Option<RowFacts>,
}

/// The values of a living row that the controls read before its travel.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RowFacts {
    on_ground: bool,
    sideways: f32,
    speed: f32,
    movement_speed: f64,
    step_height: f32,
}

/// What the start of a tick saw of one mob: its view and its wake events.
pub(crate) struct Observed {
    pub(crate) view: MobView,
    /// A hit passed the tests of DMG-HIT-001 since the last observe: wake event 1.
    pub(crate) struck: bool,
    pub(crate) idle_reset: bool,
    /// The row ticked again after a tick with no step: wake event 7.
    pub(crate) resumed: bool,
}

impl HostMob {
    /// The host of a mob of `kind`, or `None` when `kind` is not a mob type.
    pub(crate) fn new(
        id: EntityId,
        key: EntityKey,
        kind: &'static EntityTypeData,
        tick: u64,
    ) -> Option<Self> {
        let mob = kind.mob()?;
        Some(Self {
            id,
            key,
            kind,
            head_yaw_limit: mob.head_yaw_limit,
            control: MobControl::new(LookDefaults {
                turn_speed: mob.head_turn_speed,
                pitch_limit: mob.pitch_limit,
            }),
            travelled: None,
            turned: None,
            position: DVec3::ZERO,
            rotations: Rotations::default(),
            idle_time: 0,
            last_hit: None,
            strikes: 0,
            damage: None,
            panic: Panic::Calm,
            seen: tick,
            age: None,
            stalled: false,
            row: None,
        })
    }

    /// The end of the last tick: the body rotation after its travel, the damage record, the idle
    /// time, and the view of the snapshot of `tick`. A new hit counts as a hit in the last tick.
    pub(crate) fn observe<V>(
        &mut self,
        hot: &HotTick<'_, V>,
        players: &PlayerTable,
        tick: u64,
        state: &EntityState,
    ) -> Observed {
        let living = state.living.unwrap_or_default();
        self.row = state
            .living
            .filter(Living::is_alive)
            .map(|living| RowFacts {
                on_ground: state.body.flags.on_ground,
                sideways: living.input.sideways,
                speed: living.speed,
                movement_speed: living.attributes.movement_speed,
                step_height: living.attributes.step_height as f32,
            });
        self.position = state.body.position;
        self.rotations = Rotations {
            yaw: living.yaw,
            pitch: living.pitch,
            head_yaw: living.head_yaw,
            body_yaw: living.body_yaw,
        };
        self.turned = self.travelled.take().map(|input| {
            self.control
                .after_travel(&input, self.position, self.rotations)
        });
        if let Some(turned) = self.turned {
            self.rotations = turned;
        }
        let hit = state.hits.last();
        let strikes = state.hits.strikes();
        let hurt = hit.is_some() && hit != self.last_hit;
        self.last_hit = hit;
        let struck_now = strikes != self.strikes;
        self.strikes = strikes;
        if hurt && let Some(hit) = hit {
            self.damage = Some(DamageRecord {
                tick: tick.saturating_sub(1),
                cause: if hot.panic_causes.contains(u32::from(hit.damage_type)) {
                    DamageCause::Panic
                } else {
                    DamageCause::Other
                },
            });
        }
        let near = players.rows().iter().any(|player| {
            player.is_qualified()
                && player.position.distance_squared(self.position) < NEAR_PLAYER * NEAR_PLAYER
        });
        let idle_time = if near || struck_now {
            0
        } else {
            self.idle_time.saturating_add(1)
        };
        let idle_reset = idle_time == 0 && self.idle_time >= STROLL_IDLE_LIMIT;
        self.idle_time = idle_time;
        let stalled = self.age == Some(state.age);
        let resumed = self.stalled && !stalled;
        self.stalled = stalled;
        self.age = Some(state.age);
        // IN_WATER stays clear without the fluid model (GOAL-FLT-003). The row holds no fire
        // ticks, passengers or age, which ON_FIRE, CONTROLLED and BABY need.
        let marks = [
            (living.is_alive(), MobFlags::ALIVE),
            (state.body.flags.on_ground, MobFlags::ON_GROUND),
            (self.panic == Panic::Panicking, MobFlags::IN_PANIC),
        ];
        let flags = marks
            .into_iter()
            .filter(|(set, _)| *set)
            .fold(MobFlags::NONE, |flags, (_, flag)| flags.union(flag));
        Observed {
            view: MobView {
                mob: self.id,
                key: self.key,
                position: self.position,
                eye_height: self.kind.eye_height,
                flags,
                idle_time: self.idle_time,
                damage: self.damage,
                // Only a mob passenger or a boat disables a flag (GOAL-FRM-008), and only the
                // float, which needs the fluid model, holds one by a reflex.
                disabled: ControlFlags::NONE,
                path: self.control.applied(),
            },
            struck: struck_now,
            idle_reset,
            resumed,
        }
    }

    /// Applies the intents of one due decision.
    pub(crate) fn apply(&mut self, intents: &Intents) {
        self.control.apply(intents, self.position);
        if let Some(panic) = intents.panic {
            self.panic = panic;
        }
    }

    /// The controls of this tick before the travel of the row that [`HostMob::observe`] saw: the
    /// intent for the pass, and the end of the path when it ended in this step. A dying mob gets
    /// no intent.
    pub(crate) fn steer<V: BlockVolume>(
        &mut self,
        world: &BlockGoalWorld<'_, V>,
        tick: u64,
    ) -> Option<(MobIntent, Option<PathEnd>)> {
        let row = self.row.take()?;
        let feet = BlockPos::from(self.position);
        let input = ControlInput {
            tick,
            position: self.position,
            rotations: self.rotations,
            body: BodyData {
                width: self.kind.width,
                eye_height: self.kind.eye_height,
                step_height: row.step_height,
                head_yaw_limit: self.head_yaw_limit,
            },
            flags: if row.on_ground {
                BodyFlags::ON_GROUND
            } else {
                BodyFlags::NONE
            },
            sideways: row.sideways,
            speed: row.speed,
            movement_speed: row.movement_speed,
            feet_block_top: world.feet_top(feet),
        };
        let out = self.control.before_travel(&input);
        self.travelled = Some(input);
        let turned = self.turned.take();
        let intent = MobIntent {
            target: self.id,
            steering: out.steering.map(|steering| Steering {
                input: MoveInput {
                    sideways: steering.sideways,
                    vertical: steering.vertical,
                    forward: steering.forward,
                },
                jump: steering.jump,
                speed: steering.speed,
            }),
            yaw: out.yaw,
            pitch: out.pitch,
            head_yaw: out.head_yaw.or(turned.map(|turned| turned.head_yaw)),
            body_yaw: turned.map(|turned| turned.body_yaw),
        };
        Some((intent, out.path_ended))
    }
}
