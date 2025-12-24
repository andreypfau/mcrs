mod attribute;
pub mod enchantments;
pub mod lore;
mod rarity;
mod swing;
pub mod tool;

use crate::sound::{ITEM_BREAK, SoundEvent};
pub use crate::world::item::component::attribute::AttributeModifiers;
pub use crate::world::item::component::enchantments::Enchantments;
use crate::world::item::component::rarity::Rarity;
use crate::world::item::component::swing::SwingAnimation;
pub use crate::world::item::component::tool::Tool;
use crate::world::item::component::tool::{ToolMaterial, ToolRule};
use bevy_asset::Handle;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use mcrs_protocol::item::{CustomData, ItemComponentKind, Lore, MaxStackSize};

#[derive(Default, Clone)]
pub struct ItemComponents {
    pub custom: Option<CustomData>,
    pub max_stack_size: MaxStackSize,
    pub lore: Lore,
    pub enchantments: Enchantments,
    pub repair_cost: RepairCost,
    pub use_effects: UseEffects,
    pub attribute_modifiers: AttributeModifiers,
    pub rarity: Rarity,
    pub break_sound: BreakSound,
    pub tooltip_display: TooltipDisplay,
    pub swing_animation: SwingAnimation,
    pub max_damage: Option<MaxDamage>,
    pub damage: Option<Damage>,
    pub enchantable: Option<Enchantable>,
    pub tool: Option<Tool>,
}

impl ItemComponents {
    pub const fn new() -> Self {
        ItemComponents {
            custom: None,
            max_stack_size: MaxStackSize(64),
            lore: Lore::new(Vec::new()),
            enchantments: Enchantments::empty(),
            repair_cost: RepairCost(0),
            use_effects: UseEffects::DEFAULT,
            attribute_modifiers: AttributeModifiers::new(Vec::new()),
            rarity: Rarity::Common,
            break_sound: BreakSound(ITEM_BREAK),
            tooltip_display: TooltipDisplay::DEFAULT,
            swing_animation: SwingAnimation::DEFAULT,
            max_damage: None,
            damage: None,
            enchantable: None,
            tool: None,
        }
    }

    pub const fn with_durability(mut self, durability: u32) -> Self {
        self.max_stack_size = MaxStackSize(1);
        self.max_damage = Some(MaxDamage(durability));
        self.damage = Some(Damage(0));
        self
    }

    pub const fn with_enchantable(mut self, value: u8) -> Self {
        self.enchantable = Some(Enchantable(value));
        self
    }

    pub const fn with_tool(mut self, tool: Tool) -> Self {
        self.tool = Some(tool);
        self
    }

    pub const fn with_pickaxe(
        self,
        material: &ToolMaterial,
        attack_damage: f32,
        attack_speed: f32,
        rules: &'static [ToolRule],
    ) -> Self {
        material.apply_tool_properties(self, attack_damage, attack_speed, 0.0, rules)
    }
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct RepairCost(pub u32);

#[derive(Clone, Copy, Debug, Component)]
pub struct UseEffects {
    can_sprint: bool,
    interact_vibrations: bool,
    speed_multiplier: f32,
}

#[derive(Clone, Copy, Debug, Component)]
pub struct Enchantable(pub u8);

impl UseEffects {
    pub const DEFAULT: Self = UseEffects {
        can_sprint: false,
        interact_vibrations: true,
        speed_multiplier: 0.2,
    };

    pub const fn new(can_sprint: bool, interact_vibrations: bool, speed_multiplier: f32) -> Self {
        Self {
            can_sprint,
            interact_vibrations,
            speed_multiplier,
        }
    }

    pub fn can_sprint(&self) -> bool {
        self.can_sprint
    }

    pub fn interact_vibrations(&self) -> bool {
        self.interact_vibrations
    }

    pub fn speed_multiplier(&self) -> f32 {
        self.speed_multiplier
    }
}

impl Default for UseEffects {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct MaxDamage(pub u32);

#[derive(Clone, Copy, Debug, Component)]
pub struct Damage(pub u32);

#[derive(Clone, Debug, Component)]
pub struct BreakSound(pub Handle<SoundEvent>);

impl Default for BreakSound {
    fn default() -> Self {
        Self(ITEM_BREAK)
    }
}

#[derive(Clone, Debug, Default, Component)]
pub struct TooltipDisplay {
    pub hide_tooltip: bool,
    pub hidden_components: Vec<ItemComponentKind>,
}

impl TooltipDisplay {
    pub const DEFAULT: Self = TooltipDisplay {
        hide_tooltip: false,
        hidden_components: Vec::new(),
    };

    pub fn new(hide_tooltip: bool, hidden_components: Vec<ItemComponentKind>) -> Self {
        Self {
            hide_tooltip,
            hidden_components,
        }
    }
}
