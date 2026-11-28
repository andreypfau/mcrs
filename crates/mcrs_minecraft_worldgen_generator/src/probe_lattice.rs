use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::NoiseRouter;
use mcrs_minecraft_worldgen_density::router_key::RouterKey;
use std::collections::HashMap;

#[derive(Default)]
pub struct ProbeLattice {
    owner: Option<RouterKey>,
    columns: HashMap<(i32, i32), Box<[f32]>>,
    values: Vec<f32>,
}

impl ProbeLattice {
    pub(crate) fn pin(&mut self, router: &NoiseRouter, ws: &mut Workspace, x: i32, z: i32) {
        let Some(cell) = router.cell_size() else {
            return;
        };
        let (min_y, height) = (router.noise.min_y, router.noise.height as i32);
        if min_y % cell.y != 0 || height % cell.y != 0 {
            return;
        }
        let inputs = router.cell_inputs();
        if inputs.is_empty() {
            return;
        }
        let owner = router.key();
        if self.owner != Some(owner) {
            self.owner = Some(owner);
            self.columns.clear();
        }
        let rows = height / cell.y + 1;
        let base = IVec3::new(
            x.div_euclid(cell.x) * cell.x,
            min_y,
            z.div_euclid(cell.z) * cell.z,
        );
        let lattice = SampleGrid::new(IVec3::new(2, rows, 2), base, cell);
        let column_len = inputs.len() * rows as usize;
        self.values.clear();
        self.values.resize(inputs.len() * lattice.len(), 0.0);
        for node_z in 0..2 {
            for node_x in 0..2 {
                let at = IVec3::new(base.x + node_x * cell.x, min_y, base.z + node_z * cell.z);
                let column = self.columns.entry((at.x, at.z)).or_insert_with(|| {
                    let nodes = SampleGrid::new(IVec3::new(1, rows, 1), at, cell);
                    let mut sampled = vec![0.0f32; column_len];
                    router.fill_nodes(ws, &nodes, inputs, &mut sampled);
                    sampled.into_boxed_slice()
                });
                place(&mut self.values, &lattice, node_x, node_z, column);
            }
        }
        router.pin_cell_lattice(ws, &lattice, &self.values);
    }
}

fn place(values: &mut [f32], lattice: &SampleGrid, node_x: i32, node_z: i32, column: &[f32]) {
    let rows = lattice.size().y as usize;
    for (input, sampled) in column.chunks(rows).enumerate() {
        for (row, value) in sampled.iter().enumerate() {
            let to = input * lattice.len() + lattice.index_unchecked(node_x, row as i32, node_z);
            if let Some(slot) = values.get_mut(to) {
                *slot = *value;
            }
        }
    }
}
