use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};

#[derive(Resource, Debug, Default)]
pub struct MobChanges {
    pub(crate) spawned: Vec<Spawned>,
    pub(crate) strikes: Vec<Struck>,
    pub(crate) swings: Vec<Swung>,
}

impl MobChanges {
    #[must_use]
    pub fn spawned(&self) -> &[Spawned] {
        &self.spawned
    }

    #[must_use]
    pub fn strikes(&self) -> &[Struck] {
        &self.strikes
    }

    #[must_use]
    pub fn swings(&self) -> &[Swung] {
        &self.swings
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Spawned {
    pub entity: Entity,
    pub id: EntityId,
    pub start: EntityState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Struck {
    pub entity: Entity,
    pub uuid: EntityUuid,
    pub tile: TileKey,
    pub damage_type: u32,
    pub outcome: HitOutcome,
    pub died: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Swung {
    pub entity: Entity,
    pub hand: Hand,
    pub animation: SwingAnimationKind,
    pub duration: i32,
}

pub(crate) fn clear_mob_changes(mut changes: ResMut<MobChanges>) {
    changes.spawned.clear();
    changes.strikes.clear();
    changes.swings.clear();
}
