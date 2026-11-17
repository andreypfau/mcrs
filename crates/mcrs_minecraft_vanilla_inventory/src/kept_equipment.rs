use bevy_ecs::component::Component;
use mcrs_minecraft_nbt::compound::NbtCompound;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub(crate) struct KeptEquipment(pub(crate) NbtCompound);
