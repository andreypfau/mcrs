use glam::DVec3;
use mcrs_minecraft_protocol_game::entity::Pose;

use crate::angle;
use crate::client_entity::ClientEntity;
use crate::equipment::Equipment;
use crate::living::Living;
use crate::motion::{Age, Motion};
use crate::synced_data::{SyncedData, VariantId};
use crate::walk::WalkFrame;

/// The ticks in one second of the entity clock.
pub const TICKS_PER_SECOND: f32 = 20.0;
/// A change of the position above this, on one axis, makes an entity move.
pub const MOVING_DISTANCE: f64 = 1e-5;

/// The components that the values of a frame read.
pub type RenderInputsQuery = (
    &'static ClientEntity,
    &'static Motion,
    &'static Age,
    Option<&'static Living>,
    &'static SyncedData,
    Option<&'static Equipment>,
);

/// The state of one client entity that a renderer and the queries of its expressions read.
#[derive(Clone, Copy, Debug)]
pub struct RenderInputs<'a> {
    pub entity: &'a ClientEntity,
    pub motion: &'a Motion,
    pub age: &'a Age,
    pub living: Option<&'a Living>,
    pub data: &'a SyncedData,
    pub equipment: Option<&'a Equipment>,
}

impl<'a>
    From<(
        &'a ClientEntity,
        &'a Motion,
        &'a Age,
        Option<&'a Living>,
        &'a SyncedData,
        Option<&'a Equipment>,
    )> for RenderInputs<'a>
{
    fn from(
        (entity, motion, age, living, data, equipment): (
            &'a ClientEntity,
            &'a Motion,
            &'a Age,
            Option<&'a Living>,
            &'a SyncedData,
            Option<&'a Equipment>,
        ),
    ) -> Self {
        Self {
            entity,
            motion,
            age,
            living,
            data,
            equipment,
        }
    }
}

/// The values of one entity in one frame. Each query of a frame reads these values, so two
/// reads in one frame agree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameValues {
    pub partial_tick: f32,
    pub position: DVec3,
    pub eye_position: DVec3,
    /// The entity clock `age + partial tick`, in ticks.
    pub clock: f32,
    pub body_yaw: f32,
    pub head_yaw: f32,
    /// The head yaw relative to the body yaw.
    pub relative_head_yaw: f32,
    pub pitch: f32,
    pub walk: WalkFrame,
    pub death_ticks: f32,
    pub hurt_time: u32,
    /// Blocks per second in the latest tick.
    pub ground_speed: f64,
    pub vertical_speed: f64,
    pub moving: bool,
    pub on_ground: bool,
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
    /// `None` for a type with no variant, or an entity that keeps the default variant.
    pub variant: Option<VariantId>,
}

impl FrameValues {
    /// `query.life_time`, in seconds.
    #[must_use]
    pub fn life_time(&self) -> f32 {
        self.clock / TICKS_PER_SECOND
    }

    /// `query.delta_time`, in seconds: the change of the entity clock since `previous`, the
    /// values of the previous frame of the same entity.
    #[must_use]
    pub fn delta_time(&self, previous: Option<&FrameValues>) -> f32 {
        previous.map_or(0.0, |previous| {
            (self.clock - previous.clock) / TICKS_PER_SECOND
        })
    }
}

impl RenderInputs<'_> {
    /// The values at `partial_tick`, in `[0, 1]`.
    #[must_use]
    pub fn frame(&self, partial_tick: f32) -> FrameValues {
        let traits = self.entity.traits();
        let motion = self.motion;
        let data = self.data;
        let position = motion.frame_position(partial_tick);
        let alive = !data.is_dying();
        let pitch = angle::lerp(partial_tick, motion.previous.pitch, motion.current.pitch);
        let (body_yaw, head_yaw, walk, death_ticks, hurt_time) = match self.living {
            Some(living) => (
                angle::wrap(angle::alerp(
                    partial_tick,
                    living.previous_body_yaw,
                    living.body_yaw,
                )),
                angle::wrap(angle::alerp(
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
                let yaw = angle::wrap(angle::alerp(
                    partial_tick,
                    motion.previous.yaw,
                    motion.current.yaw,
                ));
                (yaw, yaw, WalkFrame::default(), 0.0, 0)
            }
        };
        let size = if data.baby { traits.baby_scale } else { 1.0 };
        let eye_height = f64::from(traits.eye_height * size) * data.scale;
        let movement = motion.movement();
        FrameValues {
            partial_tick,
            position,
            eye_position: position + DVec3::new(0.0, eye_height, 0.0),
            clock: self.age.0 as f32 + partial_tick,
            body_yaw,
            head_yaw,
            relative_head_yaw: angle::wrap(head_yaw - body_yaw),
            pitch,
            walk,
            death_ticks,
            hurt_time,
            ground_speed: movement.x.hypot(movement.z) * f64::from(TICKS_PER_SECOND),
            vertical_speed: movement.y * f64::from(TICKS_PER_SECOND),
            moving: movement.abs().max_element() > MOVING_DISTANCE,
            on_ground: motion.on_ground,
            alive,
            baby: data.baby,
            model_scale: data.scale,
            saddled: self.equipment.is_some_and(Equipment::is_saddled),
            pose: data.pose,
            on_fire: data.is_on_fire(&traits),
            sneaking: data.is_sneaking(),
            sprinting: data.is_sprinting(),
            swimming: data.is_swimming(),
            invisible: data.is_invisible(),
            variant: data.variant,
        }
    }
}
