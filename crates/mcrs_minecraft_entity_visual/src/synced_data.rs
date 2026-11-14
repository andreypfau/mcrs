use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::Pose;
use mcrs_minecraft_entity::variant::{FrogVariant, PigVariant};
use mcrs_minecraft_resource::Id;

const ON_FIRE: u8 = 1 << 0;
const CROUCHING: u8 = 1 << 1;
const SPRINTING: u8 = 1 << 3;
const SWIMMING: u8 = 1 << 4;
const INVISIBLE: u8 = 1 << 5;
const GLOWING: u8 = 1 << 6;

/// The plain texts of a custom name that turn an entity upside down.
pub const UPSIDE_DOWN_NAMES: [&str; 2] = ["Dinnerbone", "Grumm"];

/// The frozen ticks from which an entity is fully frozen.
pub const FULLY_FROZEN_TICKS: i32 = 140;

/// The numeric variant id that the entity data of a variant type carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantId {
    Pig(Id<PigVariant>),
    Frog(Id<FrogVariant>),
}

impl VariantId {
    /// The position of the entry in the synced registry of its variant type.
    #[must_use]
    pub fn number(self) -> u16 {
        match self {
            Self::Pig(id) => id.number(),
            Self::Frog(id) => id.number(),
        }
    }
}

/// The entity data that the client keeps for an entity, and its scale attribute.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SyncedData {
    pub shared_flags: u8,
    /// The plain text of the custom name is one of [`UPSIDE_DOWN_NAMES`].
    pub upside_down: bool,
    pub name_visible: bool,
    pub pose: Pose,
    pub ticks_frozen: i32,
    pub living_flags: u8,
    pub health: f32,
    pub mob_flags: u8,
    pub baby: bool,
    pub age_locked: bool,
    /// `None` until the server sends a variant: the type then has its default variant.
    pub variant: Option<VariantId>,
    /// The value of the attribute `minecraft:scale`.
    pub scale: f64,
}

impl Default for SyncedData {
    fn default() -> Self {
        Self {
            shared_flags: 0,
            upside_down: false,
            name_visible: false,
            pose: Pose::Standing,
            ticks_frozen: 0,
            living_flags: 0,
            health: 1.0,
            mob_flags: 0,
            baby: false,
            age_locked: false,
            variant: None,
            scale: 1.0,
        }
    }
}

impl SyncedData {
    #[must_use]
    pub fn is_dying(&self) -> bool {
        self.health <= 0.0
    }

    #[must_use]
    pub fn is_on_fire(&self, data: Option<&EntityTypeData>) -> bool {
        self.shared_flags & ON_FIRE != 0 && !data.is_some_and(|data| data.fire_immune)
    }

    #[must_use]
    pub fn is_sneaking(&self) -> bool {
        self.shared_flags & CROUCHING != 0
    }

    #[must_use]
    pub fn is_sprinting(&self) -> bool {
        self.shared_flags & SPRINTING != 0
    }

    #[must_use]
    pub fn is_swimming(&self) -> bool {
        self.shared_flags & SWIMMING != 0
    }

    #[must_use]
    pub fn is_invisible(&self) -> bool {
        self.shared_flags & INVISIBLE != 0
    }

    #[must_use]
    pub fn is_glowing(&self) -> bool {
        self.shared_flags & GLOWING != 0
    }

    #[must_use]
    pub fn is_fully_frozen(&self) -> bool {
        self.ticks_frozen >= FULLY_FROZEN_TICKS
    }
}

/// The smallest and the largest value of the attribute `minecraft:scale`.
pub const SCALE_RANGE: (f64, f64) = (0.0625, 16.0);
