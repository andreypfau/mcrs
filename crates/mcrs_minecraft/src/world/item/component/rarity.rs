use bevy_ecs::component::Component;

#[derive(Clone, Copy, Debug, Default, Component)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
}
