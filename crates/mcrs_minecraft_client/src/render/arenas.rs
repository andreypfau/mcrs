use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use crate::mesh::Group;
use crate::pack::QUAD_WORDS;

use super::upload::Pending;
use super::{Budget, SECTION_BYTES, VISIBLE_BYTES};

pub(super) struct Arenas {
    pub quads: Buffer,
    pub vertices: Buffer,
    pub faces: Buffer,
    pub groups: Buffer,
    pub sections: Buffer,
    pub visible: Buffer,
    pub pending: Option<Pending>,
}

#[derive(Copy, Clone)]
pub(super) enum Arena {
    Quads,
    Vertices,
    Faces,
    Groups,
    Sections,
}

impl Arenas {
    pub fn new(budget: &Budget, device: &RenderDevice) -> Self {
        let arena = |label, bytes: u64| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: bytes.max(size_of::<[u32; QUAD_WORDS]>() as u64),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            quads: arena("terrain quads", (budget.quads * QUAD_WORDS * 4) as u64),
            vertices: arena(
                "terrain vertices",
                (budget.models * super::MODEL_BYTES) as u64,
            ),
            faces: arena("terrain faces", (budget.faces * 4) as u64),
            groups: arena(
                "terrain groups",
                (budget.groups * size_of::<Group>()) as u64,
            ),
            sections: arena("terrain sections", (budget.sections * SECTION_BYTES) as u64),
            visible: arena(
                "terrain visible list",
                (budget.visible * VISIBLE_BYTES) as u64,
            ),
            pending: None,
        }
    }

    pub fn buffer(&self, arena: Arena) -> &Buffer {
        match arena {
            Arena::Quads => &self.quads,
            Arena::Vertices => &self.vertices,
            Arena::Faces => &self.faces,
            Arena::Groups => &self.groups,
            Arena::Sections => &self.sections,
        }
    }
}
