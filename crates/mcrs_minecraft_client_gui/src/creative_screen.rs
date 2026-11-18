use bevy::prelude::Resource;

use crate::creative_screen_state::CreativeScreenState;

#[derive(Resource, Clone, Debug, Default)]
pub struct CreativeScreen(pub CreativeScreenState);
