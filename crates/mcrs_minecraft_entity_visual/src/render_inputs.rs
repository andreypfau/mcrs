use glam::DVec3;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_numeric as numeric;
use mcrs_minecraft_pose::Pose;

use crate::angle;
use crate::client_entity::ClientEntity;
use crate::equipment::Equipment;
use crate::living::Living;
use crate::motion::{Age, Motion};
use crate::player_pose_ticks::PlayerPoseTicks;
use crate::sleeping_bed::{SleepingBed, bed_y_rotation};
use crate::synced_data::{SyncedData, VariantId};
use crate::tick_velocity::TickVelocity;
use crate::walk::WalkFrame;

pub const TICKS_PER_SECOND: f32 = 20.0;
pub const MOVING_DISTANCE: f64 = 1e-5;

pub type RenderInputsQuery = (
    &'static ClientEntity,
    &'static Motion,
    &'static Age,
    Option<&'static Living>,
    &'static SyncedData,
    Option<&'static Equipment>,
    Option<&'static PlayerPoseTicks>,
    Option<&'static TickVelocity>,
    Option<&'static SleepingBed>,
);

#[derive(Clone, Copy, Debug)]
pub struct RenderInputs<'a> {
    pub entity: &'a ClientEntity,
    pub motion: &'a Motion,
    pub age: &'a Age,
    pub living: Option<&'a Living>,
    pub data: &'a SyncedData,
    pub equipment: Option<&'a Equipment>,
    pub pose_ticks: Option<&'a PlayerPoseTicks>,
    pub velocity: Option<&'a TickVelocity>,
    pub bed: Option<&'a SleepingBed>,
}

impl<'a>
    From<(
        &'a ClientEntity,
        &'a Motion,
        &'a Age,
        Option<&'a Living>,
        &'a SyncedData,
        Option<&'a Equipment>,
        Option<&'a PlayerPoseTicks>,
        Option<&'a TickVelocity>,
        Option<&'a SleepingBed>,
    )> for RenderInputs<'a>
{
    fn from(
        (entity, motion, age, living, data, equipment, pose_ticks, velocity, bed): (
            &'a ClientEntity,
            &'a Motion,
            &'a Age,
            Option<&'a Living>,
            &'a SyncedData,
            Option<&'a Equipment>,
            Option<&'a PlayerPoseTicks>,
            Option<&'a TickVelocity>,
            Option<&'a SleepingBed>,
        ),
    ) -> Self {
        Self {
            entity,
            motion,
            age,
            living,
            data,
            equipment,
            pose_ticks,
            velocity,
            bed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameValues {
    pub partial_tick: f32,
    pub position: DVec3,
    pub eye_position: DVec3,
    pub clock: f32,
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub relative_head_yaw: f32,
    pub pitch: f32,
    pub walk: WalkFrame,
    pub death_ticks: f32,
    pub hurt_time: u32,
    pub ground_speed: f64,
    pub vertical_speed: f64,
    pub moving: bool,
    pub on_ground: bool,
    pub in_water: bool,
    pub upside_down: bool,
    pub riding: bool,
    pub has_rider: bool,
    pub alive: bool,
    pub baby: bool,
    pub model_scale: f64,
    pub saddled: bool,
    pub pose: Pose,
    pub on_fire: bool,
    pub sneaking: bool,
    pub sprinting: bool,
    pub swimming: bool,
    pub invisible: bool,
    pub fall_flying: bool,
    pub spin_attacking: bool,
    pub using_item: bool,
    pub swim_amount: f32,
    pub fall_flying_ticks: u32,
    pub velocity: DVec3,
    pub tick_velocity_length_squared: f64,
    pub bed_y_rotation: f32,
    pub has_bed: bool,
    pub variant: Option<VariantId>,
}

impl FrameValues {
    #[must_use]
    pub fn life_time(&self) -> f32 {
        self.clock / TICKS_PER_SECOND
    }

    #[must_use]
    pub fn delta_time(&self, previous: Option<&FrameValues>) -> f32 {
        previous.map_or(0.0, |previous| {
            (self.clock - previous.clock) / TICKS_PER_SECOND
        })
    }
}

#[must_use]
pub fn eye_height(entity: &ClientEntity, data: &SyncedData) -> f64 {
    if entity.kind == EntityType::Player {
        let dimensions = data.pose.player_dimensions().scaled(data.scale as f32);
        return f64::from(dimensions.eye_height);
    }
    let Some(type_data) = entity.type_data() else {
        return 0.0;
    };
    let size = if data.baby {
        type_data.baby_scale.unwrap_or(1.0)
    } else {
        1.0
    };
    f64::from(type_data.eye_height * size) * data.scale
}

#[must_use]
pub fn box_height(entity: &ClientEntity, data: &SyncedData) -> f64 {
    if entity.kind == EntityType::Player {
        let dimensions = data.pose.player_dimensions().scaled(data.scale as f32);
        return f64::from(dimensions.height);
    }
    let Some(type_data) = entity.type_data() else {
        return 0.0;
    };
    let size = if data.baby {
        type_data.baby_scale.unwrap_or(1.0)
    } else {
        1.0
    };
    f64::from(type_data.height * size) * data.scale
}

impl RenderInputs<'_> {
    #[must_use]
    pub fn frame(&self, partial_tick: f32) -> FrameValues {
        let type_data = self.entity.type_data();
        let motion = self.motion;
        let data = self.data;
        let position = motion.frame_position(partial_tick);
        let alive = !data.is_dying();
        let pitch = numeric::lerp(partial_tick, motion.previous.pitch, motion.current.pitch);
        let (body_yaw, head_yaw, walk, death_ticks, hurt_time) = match self.living {
            Some(living) => (
                numeric::wrap_degrees(angle::alerp(
                    partial_tick,
                    living.previous_body_yaw,
                    living.body_yaw,
                )),
                numeric::wrap_degrees(angle::alerp(
                    partial_tick,
                    living.previous_head_yaw,
                    living.head_yaw,
                )),
                if alive {
                    living.walk.frame(partial_tick)
                } else {
                    WalkFrame::default()
                },
                if living.death_time > 0 {
                    living.death_time as f32 + partial_tick
                } else {
                    0.0
                },
                living.hurt_time,
            ),
            None => {
                let yaw = numeric::wrap_degrees(angle::alerp(
                    partial_tick,
                    motion.previous.yaw,
                    motion.current.yaw,
                ));
                (yaw, yaw, WalkFrame::default(), 0.0, 0)
            }
        };
        let eye_height = eye_height(self.entity, data);
        let movement = motion.movement();
        let upside_down_sign = if data.upside_down { -1.0 } else { 1.0 };
        let bed = self
            .bed
            .and_then(|bed| bed.0)
            .filter(|_| data.pose == Pose::Sleeping);
        FrameValues {
            partial_tick,
            position,
            eye_position: position + DVec3::new(0.0, eye_height, 0.0),
            clock: self.age.0 as f32 + partial_tick,
            body_yaw,
            head_yaw,
            relative_head_yaw: upside_down_sign * numeric::wrap_degrees(head_yaw - body_yaw),
            pitch: upside_down_sign * pitch,
            walk,
            death_ticks,
            hurt_time,
            ground_speed: movement.x.hypot(movement.z) * f64::from(TICKS_PER_SECOND),
            vertical_speed: movement.y * f64::from(TICKS_PER_SECOND),
            moving: movement.abs().max_element() > MOVING_DISTANCE,
            on_ground: motion.on_ground,
            in_water: motion.in_water,
            upside_down: data.upside_down,
            riding: false,
            has_rider: false,
            alive,
            baby: data.baby,
            model_scale: data.scale,
            saddled: self.equipment.is_some_and(Equipment::is_saddled),
            pose: data.pose,
            on_fire: data.is_on_fire(type_data),
            sneaking: data.is_sneaking(),
            sprinting: data.is_sprinting(),
            swimming: data.is_swimming(),
            invisible: data.is_invisible(),
            fall_flying: data.is_fall_flying(),
            spin_attacking: data.is_spin_attacking(),
            using_item: data.is_using_item(),
            swim_amount: self
                .pose_ticks
                .map_or(0.0, |ticks| ticks.frame_swim_amount(partial_tick)),
            fall_flying_ticks: self.pose_ticks.map_or(0, |ticks| ticks.fall_flying_ticks),
            velocity: self
                .velocity
                .map_or(DVec3::ZERO, |velocity| velocity.frame(partial_tick)),
            tick_velocity_length_squared: self
                .velocity
                .map_or(0.0, |velocity| velocity.current.length_squared()),
            bed_y_rotation: bed.and_then(bed_y_rotation).unwrap_or(body_yaw),
            has_bed: bed.is_some(),
            variant: data.variant,
        }
    }
}
