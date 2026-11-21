use mcrs_minecraft_brain::ControlFlags;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::DamageRecord;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_mob_control::BodyData;
use mcrs_minecraft_mob_control::BodyFlags;
use mcrs_minecraft_mob_control::ControlInput;
use mcrs_minecraft_mob_control::PathEnd;
use mcrs_minecraft_mob_control::Rotations;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MoveInput;
use mcrs_minecraft_physics::Steering;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockPos;

use crate::BlockGoalWorld;
use crate::BrainState;

pub(crate) struct Observed {
    pub(crate) view: MobView,
    pub(crate) struck: bool,
    pub(crate) idle_reset: bool,
    pub(crate) resumed: bool,
}

pub(crate) trait BrainStateExt {
    fn observe(
        &mut self,
        panic_causes: &PanicCauses,
        players: &PlayerTable,
        tick: u64,
        id: EntityId,
        state: &EntityState,
    ) -> Observed;

    fn apply(&mut self, intents: &Intents, state: &EntityState);

    fn steer<V: BlockVolume>(
        &mut self,
        world: &BlockGoalWorld<'_, V>,
        tick: u64,
        state: &EntityState,
    ) -> Option<(MobIntent, Option<PathEnd>)>;
}

impl BrainStateExt for BrainState {
    fn observe(
        &mut self,
        panic_causes: &PanicCauses,
        players: &PlayerTable,
        tick: u64,
        id: EntityId,
        state: &EntityState,
    ) -> Observed {
        let living = state.living.unwrap_or_default();
        let position = state.body.position;
        self.turned = self.travelled.take().map(|input| {
            self.control
                .after_travel(&input, position, rotations(&living))
        });
        let hit = state.hits.last();
        let strikes = state.hits.strikes();
        let hurt = hit.is_some() && hit != self.last_hit;
        self.last_hit = hit;
        let struck_now = strikes != self.strikes;
        self.strikes = strikes;
        if hurt {
            self.hit_tick = Some(tick.saturating_sub(1));
        }
        let damage = self.hit_tick.zip(hit).map(|(tick, hit)| DamageRecord {
            tick,
            cause: if panic_causes.contains(u32::from(hit.damage_type)) {
                DamageCause::Panic
            } else {
                DamageCause::Other
            },
        });
        let reach = self.rules.idle_reset_range;
        let near = players.rows().iter().any(|player| {
            player.is_qualified() && player.position.distance_squared(position) < reach * reach
        });
        let idle_time = if near || struck_now {
            0
        } else {
            self.idle_time.saturating_add(1)
        };
        let idle_reset = idle_time == 0 && self.idle_time >= self.rules.idle_limit;
        self.idle_time = idle_time;
        let stalled = self.age == Some(state.age);
        let resumed = self.stalled && !stalled;
        self.stalled = stalled;
        self.age = Some(state.age);
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
                mob: id,
                key: state.key,
                position,
                eye_height: self.kind.eye_height,
                flags,
                idle_time: self.idle_time,
                damage,
                disabled: ControlFlags::NONE,
                path: self.control.applied(),
            },
            struck: struck_now,
            idle_reset,
            resumed,
        }
    }

    fn apply(&mut self, intents: &Intents, state: &EntityState) {
        self.control.apply(intents, state.body.position);
        if let Some(panic) = intents.panic {
            self.panic = panic;
        }
    }

    fn steer<V: BlockVolume>(
        &mut self,
        world: &BlockGoalWorld<'_, V>,
        tick: u64,
        state: &EntityState,
    ) -> Option<(MobIntent, Option<PathEnd>)> {
        let living = state.living.filter(Living::is_alive)?;
        let head_yaw_limit = self.kind.mob()?.head_yaw_limit;
        let position = state.body.position;
        let turned = self.turned.take();
        let on_ground = state.body.flags.on_ground;
        let input = ControlInput {
            tick,
            position,
            rotations: turned.unwrap_or_else(|| rotations(&living)),
            body: BodyData {
                width: self.kind.width,
                eye_height: self.kind.eye_height,
                step_height: living.attributes.step_height as f32,
                head_yaw_limit,
            },
            flags: if on_ground {
                BodyFlags::ON_GROUND
            } else {
                BodyFlags::NONE
            },
            sideways: living.input.sideways,
            speed: living.speed,
            movement_speed: living.attributes.movement_speed,
            feet_block_top: world.feet_top(BlockPos::from(position)),
        };
        let out = self.control.before_travel(&input);
        self.travelled = Some(input);
        let intent = MobIntent {
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

fn rotations(living: &Living) -> Rotations {
    Rotations {
        yaw: living.orientation.yaw,
        pitch: living.orientation.pitch,
        head_yaw: living.orientation.head_yaw,
        body_yaw: living.orientation.body_yaw,
    }
}
