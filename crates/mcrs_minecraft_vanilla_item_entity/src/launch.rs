use bevy_ecs::component::Component;
use glam::DVec3;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_item_entity::block_form;
use mcrs_minecraft_item_entity::default_form;
use mcrs_minecraft_item_entity::hand_form;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub source: BirthSource,
    pub form: Form,
    pub values: ItemEntity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Form {
    Hand {
        feet: DVec3,
        eye_height: f32,
        velocity: HandVelocity,
    },
    Default(DVec3),
    Block(DVec3),
    Fixed(Start),
}

impl Form {
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

    #[must_use]
    pub fn position(self) -> DVec3 {
        self.start(&KeyedStream::new(&EntityUuid::from_u128(0), 0))
            .position
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct Launching {
    pub(crate) source: BirthSource,
    pub(crate) form: Form,
}
