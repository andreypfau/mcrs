use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use crate::mesh::Group;
use crate::pack::QUAD_WORDS;

use super::Layout;
use super::upload::Pending;

pub(super) struct Arenas {
    pub quads: Buffer,
    pub vertices: Buffer,
    pub faces: Buffer,
    pub groups: Buffer,
    pub visible: Buffer,
    pub pending: Option<Pending>,
}

#[derive(Copy, Clone)]
pub(super) enum Arena {
    Quads,
    Vertices,
    Faces,
    Groups,
}

impl Arenas {
    pub fn new(layout: &Layout, device: &RenderDevice) -> Self {
        let arena = |label, bytes: u64| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: bytes.max(size_of::<[u32; QUAD_WORDS]>() as u64),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            quads: arena(
                "terrain quads",
                (layout.quad_capacity * QUAD_WORDS * 4) as u64,
            ),
            vertices: arena(
                "terrain vertices",
                (layout.model_capacity * crate::MODEL_BYTES) as u64,
            ),
            faces: arena("terrain faces", (layout.face_capacity * 4) as u64),
            groups: arena(
                "terrain groups",
                (layout.group_capacity * size_of::<Group>()) as u64,
            ),
            visible: arena(
                "terrain visible list",
                ((layout.quad_capacity + layout.model_capacity) * 4) as u64,
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
        }
    }
}
