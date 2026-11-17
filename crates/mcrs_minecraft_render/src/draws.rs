use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;

use mcrs_minecraft_mesh::{Draw, DrawKind, QuadFormat};

use super::DrawMask;
use super::layer::LayerGroup;
use super::shared_types::{DrawArgRow, PARAMS_MODEL, PARAMS_QUAD_CULL, PARAMS_SIZE, Params};

pub(super) const PARAMS_STRIDE: u32 = 256;
const _: () = assert!(size_of::<Params>() as u64 == PARAMS_SIZE);

pub(super) struct DrawList {
    pub draws: Vec<Draw>,
    pub visible_entries: usize,
    params: Vec<Params>,
    dirty: bool,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            draws: Vec::new(),
            visible_entries: 0,
            params: Vec::with_capacity(DrawKind::COUNT),
            dirty: false,
        }
    }

    pub fn set_draws(&mut self, draws: Vec<Draw>) -> Result<(), DrawKind> {
        unique_draw_kinds(&draws)?;
        self.draws = draws;
        Ok(())
    }

    pub fn rebuild(&mut self) -> Result<(), DrawKind> {
        unique_draw_kinds(&self.draws)?;
        self.params.clear();
        let mut visible_base = 0u32;
        for draw in &self.draws {
            let mut flags = 0u32;
            if draw.kind.format == QuadFormat::Model {
                flags |= PARAMS_MODEL;
            }
            if draw.kind.writes_depth() {
                flags |= PARAMS_QUAD_CULL;
            }
            self.params.push(Params {
                group_base: draw.first_group,
                group_count: draw.group_count,
                visible_base,
                args_index: draw.kind.index() as u32,
                counter: DrawArgRow::HiddenQuads.slot(draw.kind) as u32,
                flags,
                _pad_24: [0; 2],
            });
            visible_base += draw.quad_count;
        }
        self.visible_entries = visible_base as usize;
        self.dirty = true;
        Ok(())
    }

    pub fn flush(&mut self, params: &Buffer, queue: &RenderQueue) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let Some(last) = self.params.len().checked_sub(1) else {
            return;
        };
        let mut bytes = vec![0u8; last * PARAMS_STRIDE as usize + PARAMS_SIZE as usize];
        for (chunk, entry) in bytes.chunks_mut(PARAMS_STRIDE as usize).zip(&self.params) {
            if let Some(at) = chunk.get_mut(..PARAMS_SIZE as usize) {
                at.copy_from_slice(bytemuck::bytes_of(entry));
            }
        }
        queue.write_buffer(params, 0, &bytes);
    }

    pub fn drawn<'a>(
        &'a self,
        group: LayerGroup,
        mask: &'a DrawMask,
    ) -> impl Iterator<Item = (usize, &'a Draw)> {
        self.draws.iter().enumerate().filter(move |(_, draw)| {
            draw.quad_count != 0 && group.holds(draw.kind) && mask.drawn(draw.kind)
        })
    }
}

fn unique_draw_kinds(draws: &[Draw]) -> Result<(), DrawKind> {
    let mut seen = [false; DrawKind::COUNT];
    for draw in draws {
        if let Some(slot) = seen.get_mut(draw.kind.index()) {
            if *slot {
                return Err(draw.kind);
            }
            *slot = true;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_mesh::block::Pass;

    #[test]
    fn duplicate_draw_kind_is_rejected_without_corrupting_params() {
        let kind = DrawKind::new(Pass::Solid, QuadFormat::Greedy);
        let mut list = DrawList::new();
        assert_eq!(
            list.set_draws(vec![Draw {
                kind,
                first_group: 0,
                group_count: 1,
                quad_count: 4,
            }]),
            Ok(())
        );
        assert_eq!(list.rebuild(), Ok(()));
        assert_eq!(list.visible_entries, 4);

        let dup = vec![
            Draw {
                kind,
                first_group: 0,
                group_count: 1,
                quad_count: 4,
            },
            Draw {
                kind,
                first_group: 1,
                group_count: 1,
                quad_count: 8,
            },
        ];
        assert_eq!(list.set_draws(dup.clone()), Err(kind));
        assert_eq!(list.draws.len(), 1);

        list.draws = dup;
        assert_eq!(list.rebuild(), Err(kind));
        assert_eq!(list.visible_entries, 4);
    }
}
