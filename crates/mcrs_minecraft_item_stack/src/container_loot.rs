use mcrs_minecraft_resource::ResourceLocationBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ContainerLoot {
    pub loot_table: ResourceLocationBuf,
    pub seed: i64,
}
