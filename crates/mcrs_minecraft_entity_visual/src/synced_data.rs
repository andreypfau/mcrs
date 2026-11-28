use crate::NameText;
use bevy_ecs::component::Component;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::variant::{FrogVariant, PigVariant};
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::BlockPos;

const ON_FIRE: u8 = 1 << 0;
const CROUCHING: u8 = 1 << 1;
const SPRINTING: u8 = 1 << 3;
const SWIMMING: u8 = 1 << 4;
const INVISIBLE: u8 = 1 << 5;
const GLOWING: u8 = 1 << 6;
const FALL_FLYING: u8 = 1 << 7;
const USING_ITEM: u8 = 1 << 0;
const SPIN_ATTACK: u8 = 1 << 2;

pub const UPSIDE_DOWN_NAMES: [&str; 2] = ["Dinnerbone", "Grumm"];

pub const FULLY_FROZEN_TICKS: i32 = 140;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantId {
    Pig(Id<PigVariant>),
    Frog(Id<FrogVariant>),
}

impl VariantId {
    #[must_use]
    pub fn number(self) -> u16 {
        match self {
            Self::Pig(id) => id.number(),
            Self::Frog(id) => id.number(),
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct SyncedData {
    pub shared_flags: u8,
    pub upside_down: bool,
    pub custom_name: Option<NameText>,
    pub name_visible: bool,
    pub pose: Pose,
    pub ticks_frozen: i32,
    pub living_flags: u8,
    pub health: f32,
    pub sleeping_position: Option<BlockPos>,
    pub mob_flags: u8,
    pub baby: bool,
    pub age_locked: bool,
    pub variant: Option<VariantId>,
    pub scale: f64,
}

impl Default for SyncedData {
    fn default() -> Self {
        Self {
            shared_flags: 0,
            upside_down: false,
            custom_name: None,
            name_visible: false,
            pose: Pose::Standing,
            ticks_frozen: 0,
            living_flags: 0,
            health: 1.0,
            sleeping_position: None,
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
    pub fn is_fall_flying(&self) -> bool {
        self.shared_flags & FALL_FLYING != 0
    }

    #[must_use]
    pub fn is_using_item(&self) -> bool {
        self.living_flags & USING_ITEM != 0
    }

    #[must_use]
    pub fn is_spin_attacking(&self) -> bool {
        self.living_flags & SPIN_ATTACK != 0
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
