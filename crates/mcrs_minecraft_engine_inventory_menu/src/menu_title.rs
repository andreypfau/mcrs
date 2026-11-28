use bevy_ecs::component::Component;
use mcrs_minecraft_text::Text;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct MenuTitle(pub Text);
