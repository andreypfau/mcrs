use bevy_ecs::component::Component;
use mcrs_minecraft_nbt::compound::NbtCompound;

/// The keys of `equipment` in the player file that hold no slot of the player inventory. The
/// player keeps them as the file gave them, and each capture writes them back after the slots.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub(crate) struct KeptEquipment(pub(crate) NbtCompound);
