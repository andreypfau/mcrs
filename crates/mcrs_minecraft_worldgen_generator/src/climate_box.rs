use crate::multi_noise_biomes::{BiomeGrid, MultiNoiseBiomeTable};
use glam::IVec3;
use mcrs_minecraft_biome::climate::TargetPoint;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_voxel::ColumnPos;

const QUARTS_PER_COLUMN: i32 = 4;
const WINDOW: i32 = QUARTS_PER_COLUMN + 2;

pub struct ClimateBox {
    volume: SampleGrid,
    values: Vec<f32>,
}

impl ClimateBox {
    pub fn sample(
        noise_router: &NoiseRouter,
        columns: impl IntoIterator<Item = ColumnPos>,
        y_sections: &[i32],
    ) -> Option<Self> {
        let (&first, &last) = (y_sections.first()?, y_sections.last()?);
        let mut columns = columns.into_iter();
        let start = columns.next()?;
        let (min, max) = columns.fold((start, start), |(min, max), col| {
            (
                ColumnPos::new(min.x.min(col.x), min.z.min(col.z)),
                ColumnPos::new(max.x.max(col.x), max.z.max(col.z)),
            )
        });
        let volume = SampleGrid::new(
            IVec3::new(
                (max.x - min.x + 1) * QUARTS_PER_COLUMN + 2,
                (last - first + 1) * QUARTS_PER_COLUMN,
                (max.z - min.z + 1) * QUARTS_PER_COLUMN + 2,
            ),
            IVec3::new(min.x * 16 - 4, first * 16, min.z * 16 - 4),
            IVec3::splat(4),
        );
        let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
        let mut values = vec![0.0f32; roots.len() * volume.len()];
        noise_router.fill_roots(&mut Workspace::new(), &volume, &roots, &mut values);
        Some(Self { volume, values })
    }

    pub fn grid(&self, table: &MultiNoiseBiomeTable, col: ColumnPos) -> Option<BiomeGrid> {
        let size = self.volume.size();
        let origin = self.volume.min_block();
        let offset_x = (col.x * 16 - 4 - origin.x) / 4;
        let offset_z = (col.z * 16 - 4 - origin.z) / 4;
        let inside = |offset: i32, span: i32| (0..=span - WINDOW).contains(&offset);
        if !inside(offset_x, size.x) || !inside(offset_z, size.z) {
            return None;
        }
        let window = SampleGrid::new(
            IVec3::new(WINDOW, size.y, WINDOW),
            IVec3::new(col.x * 16 - 4, origin.y, col.z * 16 - 4),
            IVec3::splat(4),
        );
        let points = self.volume.len();
        let mut rows = self.values.chunks_exact(points);
        let (Some(temperature), Some(vegetation), Some(continents), Some(erosion)) =
            (rows.next(), rows.next(), rows.next(), rows.next())
        else {
            return None;
        };
        let (Some(depth), Some(ridges)) = (rows.next(), rows.next()) else {
            return None;
        };

        let mut cursor = table.cursor();
        let mut ids = Vec::with_capacity(window.len());
        for z in 0..WINDOW {
            for x in 0..WINDOW {
                for y in 0..size.y {
                    let at = self.volume.index_unchecked(offset_x + x, y, offset_z + z);
                    let target = TargetPoint::new(
                        *temperature.get(at)?,
                        *vegetation.get(at)?,
                        *continents.get(at)?,
                        *erosion.get(at)?,
                        *depth.get(at)?,
                        *ridges.get(at)?,
                    );
                    ids.push(*cursor.find_value(target)?);
                }
            }
        }
        Some(BiomeGrid {
            volume: window,
            ids,
        })
    }
}
