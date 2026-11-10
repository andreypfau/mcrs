use crate::base_attributes::BaseAttributes;
use crate::entity_type_keys::EntityType;
use crate::life::Life;
use crate::mob_data::MobData;
use crate::variant_indices::VariantIndices;

/// The type data of an entity type (`entity-tracking.md`, TRK-TYPE-001), with the look and body
/// values of a mob type and the values that a client reads to tick and draw the type.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct EntityTypeData {
    pub entity_type: EntityType,
    /// The width of the box of the standing adult.
    pub width: f32,
    /// The height of the box of the standing adult.
    pub height: f32,
    pub eye_height: f32,
    /// The client tracking range, in columns.
    pub tracking_range: u32,
    /// The tracker ticks between two periodic position updates.
    pub update_interval: u32,
    pub fire_immune: bool,
    /// The size of a baby relative to the adult, which the renderer applies; `None` for a type
    /// with no baby form or with no uniform baby scale in its mob document.
    pub baby_scale: Option<f32>,
    /// The factor from the horizontal movement of a tick to the walk target speed.
    pub walk_target_factor: f32,
    pub life: Life,
    /// `None` for a type that the server does not make.
    pub base_attributes: Option<BaseAttributes>,
    /// Where the variant entries of the type are in its entity data.
    pub variant_indices: VariantIndices,
}

const MOB_DEFAULTS: EntityTypeData = EntityTypeData {
    entity_type: EntityType::Pig,
    width: 0.0,
    height: 0.0,
    eye_height: 0.0,
    tracking_range: 10,
    update_interval: EntityTypeData::DEFAULT_UPDATE_INTERVAL,
    fire_immune: false,
    baby_scale: None,
    // client-entities.md, CENTS-WALK-001, item 3.
    walk_target_factor: 4.0,
    life: Life::Mob(MobData::DEFAULT),
    base_attributes: None,
    variant_indices: VariantIndices::NONE,
};

const ANIMAL: Life = Life::Mob(MobData {
    ageable: true,
    ..MobData::DEFAULT
});

impl EntityTypeData {
    /// The update interval of a type that the table does not list.
    pub const DEFAULT_UPDATE_INTERVAL: u32 = 3;

    pub const PIG: Self = Self {
        entity_type: EntityType::Pig,
        width: 0.9,
        height: 0.9,
        eye_height: 0.765,
        // mob-pig.md: the size of a baby is the adult values times 0.5.
        baby_scale: Some(0.5),
        life: ANIMAL,
        base_attributes: Some(BaseAttributes {
            max_health: 10.0,
            movement_speed: 0.25,
        }),
        variant_indices: VariantIndices::PIG,
        ..MOB_DEFAULTS
    };

    pub const COW: Self = Self {
        entity_type: EntityType::Cow,
        width: 0.9,
        height: 1.4,
        eye_height: 1.3,
        // mob-animals.md, ANIM-TYPE-001: the baby box is half the adult box on each axis.
        baby_scale: Some(0.5),
        life: ANIMAL,
        base_attributes: Some(BaseAttributes {
            max_health: 10.0,
            movement_speed: 0.2_f32 as f64,
        }),
        variant_indices: VariantIndices::COW,
        ..MOB_DEFAULTS
    };

    pub const SHEEP: Self = Self {
        entity_type: EntityType::Sheep,
        width: 0.9,
        height: 1.3,
        eye_height: 1.235,
        // mob-animals.md, ANIM-TYPE-001: the baby box is half the adult box on each axis.
        baby_scale: Some(0.5),
        life: ANIMAL,
        base_attributes: Some(BaseAttributes {
            max_health: 8.0,
            movement_speed: 0.23_f32 as f64,
        }),
        ..MOB_DEFAULTS
    };

    pub const CHICKEN: Self = Self {
        entity_type: EntityType::Chicken,
        width: 0.4,
        height: 0.7,
        eye_height: 0.644,
        life: ANIMAL,
        base_attributes: Some(BaseAttributes {
            max_health: 4.0,
            movement_speed: 0.25,
        }),
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
        base_attributes: Some(BaseAttributes {
            max_health: 20.0,
            movement_speed: 0.23_f32 as f64,
        }),
        ..MOB_DEFAULTS
    };

    pub const FROG: Self = Self {
        entity_type: EntityType::Frog,
        width: 0.5,
        height: 0.5,
        eye_height: 0.425,
        // mob-frog.md: the walk target factor.
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
        life: Life::Living,
        ..MOB_DEFAULTS
    };

    /// Every row of the table.
    pub const ALL: [&'static Self; 7] = [
        &Self::PIG,
        &Self::COW,
        &Self::SHEEP,
        &Self::CHICKEN,
        &Self::ZOMBIFIED_PIGLIN,
        &Self::FROG,
        &Self::PLAYER,
    ];

    /// The type data of `entity_type`, or `None` for a type that the table does not list yet.
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
            _ => None,
        }
    }

    /// The type data of `entity_type` when the server makes mobs of it (TRK-TYPE-002).
    #[must_use]
    pub fn server_mob(entity_type: EntityType) -> Option<&'static Self> {
        Self::of(entity_type).filter(|data| data.mob().is_some() && data.base_attributes.is_some())
    }

    #[must_use]
    pub const fn mob(&self) -> Option<&MobData> {
        match &self.life {
            Life::Mob(mob) => Some(mob),
            Life::Living => None,
        }
    }

    #[must_use]
    pub const fn is_living(&self) -> bool {
        matches!(self.life, Life::Living | Life::Mob(_))
    }

    /// The entity data chain of the type holds the ageable mob part (TRK-DATA-006).
    #[must_use]
    pub const fn is_ageable(&self) -> bool {
        matches!(self.life, Life::Mob(MobData { ageable: true, .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PLAYER_EYE_HEIGHT, PLAYER_HEIGHT, PLAYER_WIDTH};

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

    #[test]
    fn the_f64_player_constants_are_the_player_row() {
        let player = EntityTypeData::PLAYER;
        for (constant, row) in [
            (PLAYER_WIDTH, player.width),
            (PLAYER_HEIGHT, player.height),
            (PLAYER_EYE_HEIGHT, player.eye_height),
        ] {
            assert!(
                (constant - f64::from(row)).abs() < f64::from(f32::EPSILON) * constant,
                "{constant} != {row}"
            );
        }
    }
}
