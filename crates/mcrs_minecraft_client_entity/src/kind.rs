use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol_game::entity::Pose;

/// The update interval of a type that the table below does not list.
pub const DEFAULT_UPDATE_INTERVAL: u32 = 3;
/// The default walk target factor of a living entity.
pub const DEFAULT_WALK_TARGET_FACTOR: f32 = 4.0;
/// The default head limit of a mob, in degrees.
pub const DEFAULT_HEAD_LIMIT: f32 = 75.0;

/// How an entity type lives, which picks the rules of its tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Life {
    /// Not a living entity: it has no head, body or walk.
    Inanimate,
    /// A living entity that is not a mob, such as a player.
    Living,
    /// A mob: its body follows its head.
    Mob(MobTraits),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobTraits {
    /// The largest angle between the head and the body, in degrees.
    pub head_limit: f32,
    /// The mob has the baby and age-locked entity data.
    pub ageable: bool,
}

/// The constants of an entity type that the client tick and the renderer read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KindTraits {
    /// Ticks between two position updates of the server.
    pub update_interval: u32,
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
    /// The size of a baby relative to the adult.
    pub baby_scale: f32,
    pub fire_immune: bool,
    pub walk_target_factor: f32,
    pub life: Life,
}

impl KindTraits {
    const INANIMATE: Self = Self {
        update_interval: DEFAULT_UPDATE_INTERVAL,
        width: 0.0,
        height: 0.0,
        eye_height: 0.0,
        baby_scale: 1.0,
        fire_immune: false,
        walk_target_factor: DEFAULT_WALK_TARGET_FACTOR,
        life: Life::Inanimate,
    };

    /// The traits of `kind`. A type that milestone content does not use yet has the traits of
    /// an inanimate entity with the default update interval.
    #[must_use]
    pub fn of(kind: EntityType) -> Self {
        match kind {
            EntityType::Pig => Self {
                update_interval: 3,
                width: 0.9,
                height: 0.9,
                eye_height: 0.765,
                baby_scale: 0.5,
                life: Life::Mob(MobTraits {
                    head_limit: DEFAULT_HEAD_LIMIT,
                    ageable: true,
                }),
                ..Self::INANIMATE
            },
            EntityType::Frog => Self {
                update_interval: 3,
                width: 0.5,
                height: 0.5,
                eye_height: 0.425,
                baby_scale: 0.5,
                walk_target_factor: 25.0,
                life: Life::Mob(MobTraits {
                    head_limit: 5.0,
                    ageable: true,
                }),
                ..Self::INANIMATE
            },
            EntityType::Player => Self {
                update_interval: 2,
                width: 0.6,
                height: 1.8,
                eye_height: 1.62,
                life: Life::Living,
                ..Self::INANIMATE
            },
            _ => Self::INANIMATE,
        }
    }

    #[must_use]
    pub fn is_living(&self) -> bool {
        !matches!(self.life, Life::Inanimate)
    }

    #[must_use]
    pub fn is_ageable(&self) -> bool {
        matches!(self.life, Life::Mob(MobTraits { ageable: true, .. }))
    }

    /// The walk target factor of an entity of this type in `pose`. A frog in a long jump does
    /// not walk.
    #[must_use]
    pub fn walk_target_factor_in(&self, kind: EntityType, pose: Pose) -> f32 {
        if kind == EntityType::Frog && pose == Pose::LongJumping {
            0.0
        } else {
            self.walk_target_factor
        }
    }
}
