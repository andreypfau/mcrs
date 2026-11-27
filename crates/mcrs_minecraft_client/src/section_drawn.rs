use bevy::prelude::*;
use mcrs_minecraft_mesh::Connectivity;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionDrawn {
    Laid {
        section: [i32; 3],
        slot: u32,
        connectivity: Connectivity,
    },
    Forgotten {
        section: [i32; 3],
    },
}
