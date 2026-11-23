use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FoodLevel {
    pub food: i32,
    pub saturation: f32,
}
