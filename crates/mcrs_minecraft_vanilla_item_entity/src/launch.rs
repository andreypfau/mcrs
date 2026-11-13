use bevy_ecs::component::Component;
use glam::DVec3;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_item_entity::block_form;
use mcrs_minecraft_item_entity::default_form;
use mcrs_minecraft_item_entity::hand_form;

/// How a stack enters the world as an item entity (IEN-SPN-001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    /// The source of the UUID of the new item entity, such as the thrower.
    pub source: BirthSource,
    pub form: Form,
    pub values: ItemEntity,
}

/// The start position and velocity of an item entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Form {
    /// The hand form of an entity at `feet` (IEN-SPN-002).
    Hand {
        feet: DVec3,
        eye_height: f32,
        velocity: HandVelocity,
    },
    /// The default form at a point (IEN-SPN-008).
    Default(DVec3),
    /// A drop of the block whose lowest corner is the point (PHY-CMP-025).
    Block(DVec3),
    /// A given start, such as `summon item`.
    Fixed(Start),
}

impl Form {
    /// The start, with the random draws of `stream`.
    #[must_use]
    pub fn start(self, stream: &KeyedStream) -> Start {
        match self {
            Self::Hand {
                feet,
                eye_height,
                velocity,
            } => hand_form(feet, eye_height, velocity, stream),
            Self::Default(point) => default_form(point, stream),
            Self::Block(block) => block_form(block, stream),
            Self::Fixed(start) => start,
        }
    }

    /// The start position, which takes no random draw.
    #[must_use]
    pub fn position(self) -> DVec3 {
        self.start(&KeyedStream::new(&EntityKey(0), 0)).position
    }
}

/// A launch whose row the next run of the plugin spawns.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct Launching {
    pub(crate) source: BirthSource,
    pub(crate) form: Form,
}
