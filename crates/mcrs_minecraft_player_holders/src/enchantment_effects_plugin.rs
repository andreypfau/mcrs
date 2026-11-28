use bevy_app::App;
use bevy_app::Plugin;

use crate::enchantment_effects::EnchantmentEffects;
use crate::enchantment_effects::derive_enchantment_effects;
use crate::enchantment_effects::forget_enchantment_effects;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnchantmentEffectsPlugin;

impl Plugin for EnchantmentEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnchantmentEffects>()
            .add_observer(derive_enchantment_effects)
            .add_observer(forget_enchantment_effects);
    }
}
