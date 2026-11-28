use bevy::prelude::*;
use mcrs_minecraft_mesh::SECTION_SIZE;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct StreamFocus {
    eye: Vec3,
}

impl StreamFocus {
    #[must_use]
    pub fn eye(&self) -> Vec3 {
        self.eye
    }

    #[must_use]
    pub fn section(&self) -> [i32; 3] {
        (self.eye / SECTION_SIZE as f32)
            .floor()
            .as_ivec3()
            .to_array()
    }

    pub(crate) fn look_from(&mut self, eye: Vec3) -> bool {
        let was = self.section();
        self.eye = eye;
        self.section() != was
    }
}
