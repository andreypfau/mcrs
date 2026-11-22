use bevy_ecs::entity::Entity;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_entity_key::WriterKey;

use crate::CrowdCount;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DamageRequest {
    pub target: Entity,
    pub hit: Hit,
    pub writer: WriterKey,
    pub tick: u64,
    pub sequence: u32,
    pub crowd: Option<CrowdCount>,
}

impl DamageRequest {
    #[must_use]
    pub const fn new(
        target: Entity,
        hit: Hit,
        writer: WriterKey,
        tick: u64,
        sequence: u32,
    ) -> Self {
        Self {
            target,
            hit,
            writer,
            tick,
            sequence,
            crowd: None,
        }
    }

    #[must_use]
    pub const fn in_crowd(self, crowd: CrowdCount) -> Self {
        Self {
            crowd: Some(crowd),
            ..self
        }
    }
}
