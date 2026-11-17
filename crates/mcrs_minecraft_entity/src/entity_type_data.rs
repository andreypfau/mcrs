use crate::client_motion::ClientMotion;
use crate::entity_type_keys::EntityType;
use crate::life::Life;
use crate::mob_data::MobData;
use crate::precise_sync::PreciseSync;
use crate::variant_indices::VariantIndices;
use mcrs_minecraft_entity_attribute::AttributeBases;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct EntityTypeData {
    pub entity_type: EntityType,
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
    pub tracking_range: u32,
    pub update_interval: u32,
    pub tracks_velocity: bool,
    pub client_motion: ClientMotion,
    pub fire_immune: bool,
    pub baby_scale: Option<f32>,
    pub walk_target_factor: f32,
    pub life: Life,
    pub variant_indices: VariantIndices,
}

const MOB_DEFAULTS: EntityTypeData = EntityTypeData {
    entity_type: EntityType::Pig,
    width: 0.0,
    height: 0.0,
    eye_height: 0.0,
    tracking_range: 10,
    update_interval: EntityTypeData::DEFAULT_UPDATE_INTERVAL,
    tracks_velocity: true,
    client_motion: ClientMotion::Replayed,
    fire_immune: false,
    baby_scale: None,
    walk_target_factor: 4.0,
    life: Life::Mob(MobData::DEFAULT),
    variant_indices: VariantIndices::NONE,
};

const ANIMAL: Life = Life::Mob(MobData {
    ageable: true,
    ..MobData::DEFAULT
});

impl EntityTypeData {
    pub const DEFAULT_UPDATE_INTERVAL: u32 = 3;

    pub const PIG: Self = Self {
        entity_type: EntityType::Pig,
        width: 0.9,
        height: 0.9,
        eye_height: 0.765,
        baby_scale: Some(0.5),
        life: ANIMAL,
        variant_indices: VariantIndices::PIG,
        ..MOB_DEFAULTS
    };

    pub const COW: Self = Self {
        entity_type: EntityType::Cow,
        width: 0.9,
        height: 1.4,
        eye_height: 1.3,
        baby_scale: Some(0.5),
        life: ANIMAL,
        variant_indices: VariantIndices::COW,
        ..MOB_DEFAULTS
    };

    pub const SHEEP: Self = Self {
        entity_type: EntityType::Sheep,
        width: 0.9,
        height: 1.3,
        eye_height: 1.235,
        baby_scale: Some(0.5),
        life: ANIMAL,
        ..MOB_DEFAULTS
    };

    pub const CHICKEN: Self = Self {
        entity_type: EntityType::Chicken,
        width: 0.4,
        height: 0.7,
        eye_height: 0.644,
        life: ANIMAL,
        variant_indices: VariantIndices::CHICKEN,
        ..MOB_DEFAULTS
    };

    pub const ZOMBIFIED_PIGLIN: Self = Self {
        entity_type: EntityType::ZombifiedPiglin,
        width: 0.6,
        height: 1.95,
        eye_height: 1.79,
        tracking_range: 8,
        fire_immune: true,
        ..MOB_DEFAULTS
    };

    pub const FROG: Self = Self {
        entity_type: EntityType::Frog,
        width: 0.5,
        height: 0.5,
        eye_height: 0.425,
        walk_target_factor: 25.0,
        life: Life::Mob(MobData {
            head_yaw_limit: 5.0,
            ageable: true,
            ..MobData::DEFAULT
        }),
        ..MOB_DEFAULTS
    };

    pub const PLAYER: Self = Self {
        entity_type: EntityType::Player,
        width: 0.6,
        height: 1.8,
        eye_height: 1.62,
        tracking_range: 32,
        update_interval: 2,
        tracks_velocity: false,
        life: Life::Living,
        ..MOB_DEFAULTS
    };

    pub const ITEM: Self = Self {
        entity_type: EntityType::Item,
        width: 0.25,
        height: 0.25,
        eye_height: 0.2125,
        tracking_range: 6,
        update_interval: 20,
        client_motion: ClientMotion::Simulated(PreciseSync::LostPrecision),
        life: Life::Lifeless,
        ..MOB_DEFAULTS
    };

    pub const FALLING_BLOCK: Self = Self {
        entity_type: EntityType::FallingBlock,
        width: 0.98,
        height: 0.98,
        eye_height: 0.833_000_06,
        update_interval: 20,
        client_motion: ClientMotion::Simulated(PreciseSync::AfterCollision),
        life: Life::Lifeless,
        ..MOB_DEFAULTS
    };

    pub const TNT: Self = Self {
        entity_type: EntityType::Tnt,
        width: 0.98,
        height: 0.98,
        eye_height: 0.15,
        update_interval: 10,
        client_motion: ClientMotion::Simulated(PreciseSync::AfterCollision),
        fire_immune: true,
        life: Life::Lifeless,
        ..MOB_DEFAULTS
    };

    pub const EXPERIENCE_ORB: Self = Self {
        entity_type: EntityType::ExperienceOrb,
        width: 0.5,
        height: 0.5,
        eye_height: 0.425,
        tracking_range: 6,
        update_interval: 20,
        client_motion: ClientMotion::Simulated(PreciseSync::Never),
        life: Life::Lifeless,
        ..MOB_DEFAULTS
    };

    pub const ALL: [&'static Self; 11] = [
        &Self::PIG,
        &Self::COW,
        &Self::SHEEP,
        &Self::CHICKEN,
        &Self::ZOMBIFIED_PIGLIN,
        &Self::FROG,
        &Self::PLAYER,
        &Self::ITEM,
        &Self::FALLING_BLOCK,
        &Self::TNT,
        &Self::EXPERIENCE_ORB,
    ];

    #[must_use]
    pub const fn of(entity_type: EntityType) -> Option<&'static Self> {
        match entity_type {
            EntityType::Pig => Some(&Self::PIG),
            EntityType::Cow => Some(&Self::COW),
            EntityType::Sheep => Some(&Self::SHEEP),
            EntityType::Chicken => Some(&Self::CHICKEN),
            EntityType::ZombifiedPiglin => Some(&Self::ZOMBIFIED_PIGLIN),
            EntityType::Frog => Some(&Self::FROG),
            EntityType::Player => Some(&Self::PLAYER),
            EntityType::Item => Some(&Self::ITEM),
            EntityType::FallingBlock => Some(&Self::FALLING_BLOCK),
            EntityType::Tnt => Some(&Self::TNT),
            EntityType::ExperienceOrb => Some(&Self::EXPERIENCE_ORB),
            _ => None,
        }
    }

    #[must_use]
    pub const fn attribute_bases(&self) -> Option<AttributeBases> {
        crate::attribute_bases(self.entity_type)
    }

    #[must_use]
    pub const fn mob(&self) -> Option<&MobData> {
        match &self.life {
            Life::Mob(mob) => Some(mob),
            Life::Living | Life::Lifeless => None,
        }
    }

    #[must_use]
    pub const fn is_living(&self) -> bool {
        matches!(self.life, Life::Living | Life::Mob(_))
    }

    #[must_use]
    pub const fn is_ageable(&self) -> bool {
        matches!(self.life, Life::Mob(MobData { ageable: true, .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_finds_each_row_of_all() {
        for row in EntityTypeData::ALL {
            assert_eq!(
                EntityTypeData::of(row.entity_type),
                Some(row),
                "{:?}",
                row.entity_type
            );
        }
        let listed = EntityType::ALL
            .iter()
            .filter(|entity_type| EntityTypeData::of(**entity_type).is_some())
            .count();
        assert_eq!(
            listed,
            EntityTypeData::ALL.len(),
            "ALL misses a row of `of`"
        );
    }
}
