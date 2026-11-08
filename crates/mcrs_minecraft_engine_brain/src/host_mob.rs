use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_brain::ControlFlags as DisabledFlags;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::DamageRecord;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobId;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_engine_physics::MobHit;
use mcrs_minecraft_engine_physics::MobIntent;
use mcrs_minecraft_goals::STROLL_IDLE_LIMIT;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_mob_control::BodyData;
use mcrs_minecraft_mob_control::ControlFlags;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::LookDefaults;
use mcrs_minecraft_mob_control::MobControl;
use mcrs_minecraft_mob_control::PathEnd;
use mcrs_minecraft_mob_control::Rotations;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::Steering;
use mcrs_minecraft_volume::BlockVolume;

use crate::BlockGoalWorld;
use crate::HotTick;

/// A player nearer than this resets the idle time of a mob (AIT-VAN-020).
const NEAR_PLAYER: f64 = 32.0;
/// The largest difference of the head yaw from the body yaw of an animal.
const HEAD_YAW_LIMIT: f32 = 75.0;

/// One mob of the stage on the side of the hot tick: its controls and the state that its view
/// holds besides its row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HostMob {
    pub(crate) id: EntityId,
    pub(crate) key: OrderKey,
    kind: MobType,
    control: MobControl,
    /// The input of the last step before the travel: the body rotation of that step runs at the
    /// start of the next tick, since the pass has no step after the travel.
    travelled: Option<ControlInput>,
    /// The head yaw and the body yaw of that body rotation, which the next intent writes.
    turned: Option<Rotations>,
    position: Point,
    rotations: Rotations,
    idle_time: u32,
    last_hit: Option<MobHit>,
    damage: Option<DamageRecord>,
    panic: Panic,
    /// The tick in which the life cycle last found the mob.
    pub(crate) seen: u64,
}

/// What the start of a tick saw of one mob: its view and its wake events.
pub(crate) struct Observed {
    pub(crate) view: MobView,
    pub(crate) hurt: bool,
    pub(crate) idle_reset: bool,
}

impl HostMob {
    pub(crate) fn new(id: EntityId, key: OrderKey, kind: MobType, tick: u64) -> Self {
        Self {
            id,
            key,
            kind,
            control: MobControl::new(LookDefaults::ANIMAL),
            travelled: None,
            turned: None,
            position: Point::default(),
            rotations: Rotations::default(),
            idle_time: 0,
            last_hit: None,
            damage: None,
            panic: Panic::Calm,
            seen: tick,
        }
    }

    pub(crate) const fn mob(&self) -> MobId {
        MobId(self.id.0)
    }

    /// The end of the last tick: the body rotation after its travel, the damage record, the idle
    /// time, and the view of the snapshot of `tick`. A new hit counts as a hit in the last tick.
    pub(crate) fn observe<V>(
        &mut self,
        hot: &HotTick<'_, V>,
        tick: u64,
        state: &EntityState,
    ) -> Observed {
        let living = state.living.unwrap_or_default();
        self.position = point(state.body.position);
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
        let hit = hot.mobs.0.get(&self.id).and_then(|mob| mob.last_hit);
        let hurt = hit.is_some() && hit != self.last_hit;
        self.last_hit = hit;
        if hurt && let Some(hit) = hit {
            self.damage = Some(DamageRecord {
                tick: tick.saturating_sub(1),
                cause: if hot.panic_causes.contains(hit.damage_type) {
                    DamageCause::Panic
                } else {
                    DamageCause::Other
                },
            });
        }
        let near = hot.players.iter().any(|player| {
            player.is_qualified()
                && player.position.distance_squared(self.position) < NEAR_PLAYER * NEAR_PLAYER
        });
        let idle_time = if near || hurt {
            0
        } else {
            self.idle_time.saturating_add(1)
        };
        let idle_reset = idle_time == 0 && self.idle_time >= STROLL_IDLE_LIMIT;
        self.idle_time = idle_time;
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
                mob: self.mob(),
                key: self.key,
                position: self.position,
                eye_height: self.kind.eye_height(),
                flags,
                idle_time: self.idle_time,
                damage: self.damage,
                disabled: DisabledFlags::NONE,
                path: self.control.applied(),
            },
            hurt,
            idle_reset,
        }
    }

    /// Applies the intents of one due decision.
    pub(crate) fn apply(&mut self, intents: &Intents) {
        self.control.apply(intents, self.position);
        if let Some(panic) = intents.panic {
            self.panic = panic;
        }
    }

    /// The controls of this tick before the travel of the row: the intent for the pass, and the
    /// end of the path when it ended in this step. A dying mob gets no intent.
    pub(crate) fn steer<V: BlockVolume>(
        &mut self,
        world: &BlockGoalWorld<'_, V>,
        tick: u64,
        state: &EntityState,
    ) -> Option<(MobIntent, Option<PathEnd>)> {
        let living: &Living = state.living.as_ref()?;
        if !living.is_alive() {
            return None;
        }
        let feet = self.position.block();
        let [width, _] = self.kind.size();
        let input = ControlInput {
            tick,
            position: self.position,
            rotations: self.rotations,
            body: BodyData {
                width,
                eye_height: self.kind.eye_height(),
                step_height: living.attributes.step_height as f32,
                head_yaw_limit: HEAD_YAW_LIMIT,
            },
            flags: if state.body.flags.on_ground {
                ControlFlags::ON_GROUND
            } else {
                ControlFlags::NONE
            },
            sideways: living.input.sideways,
            speed: living.speed,
            movement_speed: living.attributes.movement_speed,
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

fn point(position: DVec3) -> Point {
    Point::new(position.x, position.y, position.z)
}
