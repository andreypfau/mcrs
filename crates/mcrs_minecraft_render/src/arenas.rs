use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;

use mcrs_minecraft_mesh::Group;
use mcrs_minecraft_mesh::pack::QUAD_WORDS;

use super::Budget;
use super::render_path::RenderPath;
use super::shared_types::SECTION_BYTES;
use super::upload::{Pending, Placement};
use super::words::Words;

pub(super) const HEADROOM: usize = 4;

const GEOMETRY_STEP: u64 = 16 << 20;

pub(super) struct Geometry {
    pub words: Words,
    label: &'static str,
    limit: u64,
}

impl Geometry {
    pub fn words(path: RenderPath, label: &'static str, limit: u64, device: &RenderDevice) -> Self {
        Self {
            words: Words::new(path, label, GEOMETRY_STEP.min(limit), device),
            label,
            limit,
        }
    }

    pub fn records(
        label: &'static str,
        limit: u64,
        usage: BufferUsages,
        device: &RenderDevice,
    ) -> Self {
        Self {
            words: Words::records(label, GEOMETRY_STEP.min(limit), usage, device),
            label,
            limit,
        }
    }

    pub fn fit(&mut self, end: u64, device: &RenderDevice, encoder: &mut CommandEncoder) -> bool {
        if end <= self.words.bytes() {
            return false;
        }
        let size = (end + end / HEADROOM as u64)
            .next_multiple_of(GEOMETRY_STEP)
            .min(self.limit.max(end));
        bevy::log::info!(
            label = self.label,
            mb = size >> 20,
            "growing a terrain arena"
        );
        self.words = self.words.grown(self.label, size, device, encoder);
        true
    }
}

pub(super) struct Grown {
    pub geometry: bool,
    pub groups: bool,
}

pub(super) struct Arenas {
    pub quads: Geometry,
    pub vertices: Geometry,
    pub faces: Geometry,
    pub groups: Geometry,
    pub sections: Words,
    pub pending: Option<Pending>,
}

impl Arenas {
    pub fn new(path: RenderPath, budget: &Budget, device: &RenderDevice) -> Self {
        let words = |label, limit: usize| Geometry::words(path, label, limit as u64, device);
        let records = match path {
            RenderPath::Full => BufferUsages::STORAGE,
            RenderPath::Minimal => BufferUsages::VERTEX,
        };
        Self {
            quads: words("terrain quads", budget.quads * QUAD_WORDS * 4),
            vertices: words("terrain vertices", budget.models * super::MODEL_BYTES),
            faces: words("terrain faces", budget.faces * super::FACE_BYTES),
            groups: Geometry::records(
                "terrain groups",
                (budget.groups * size_of::<Group>()) as u64,
                records,
                device,
            ),
            sections: Words::new(
                path,
                "terrain sections",
                budget.sections as u64 * SECTION_BYTES,
                device,
            ),
            pending: None,
        }
    }

    pub fn group_slots(&self) -> usize {
        self.groups.words.bytes() as usize / size_of::<Group>()
    }

    pub fn fit(
        &mut self,
        placement: &Placement,
        device: &RenderDevice,
        encoder: &mut CommandEncoder,
    ) -> Grown {
        fn end<T>((offset, data): &(u64, Vec<T>)) -> u64 {
            offset + size_of_val(&data[..]) as u64
        }
        let quads = self.quads.fit(end(&placement.quads), device, encoder);
        let vertices = self.vertices.fit(end(&placement.vertices), device, encoder);
        let faces = self.faces.fit(end(&placement.faces), device, encoder);
        let groups_end = placement
            .groups
            .iter()
            .map(|(offset, records)| offset + size_of_val(&records[..]) as u64)
            .max()
            .unwrap_or(0);
        Grown {
            geometry: quads | vertices | faces,
            groups: self.groups.fit(groups_end, device, encoder),
        }
    }
}
