use bevy_ecs::component::Component;

#[derive(Default, Clone, Debug, Component)]
pub struct AttributeModifiers {
    modifiers: Vec<Entry>,
}

impl AttributeModifiers {
    pub const fn new(modifiers: Vec<Entry>) -> Self {
        Self { modifiers }
    }

    pub fn modifiers(&self) -> &Vec<Entry> {
        &self.modifiers
    }
}

#[derive(Default, Clone, Debug)]
pub struct Entry {}
