use crate::biome_window::BiomeWindow;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
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
    min: ColumnPos,
    columns_x: i32,
    members: Vec<bool>,
    ids: Vec<u8>,
}

impl ClimateBox {
    pub fn sample<I>(
        noise_router: &NoiseRouter,
        table: &MultiNoiseBiomeTable,
        columns: I,
        y_sections: &[i32],
    ) -> Option<Self>
    where
        I: IntoIterator<Item = ColumnPos>,
        I::IntoIter: Clone,
    {
        let (&first, &last) = (y_sections.first()?, y_sections.last()?);
        let columns = columns.into_iter();
        let start = columns.clone().next()?;
        let (min, max) = columns.clone().fold((start, start), |(min, max), col| {
            (
                ColumnPos::new(min.x.min(col.x), min.z.min(col.z)),
                ColumnPos::new(max.x.max(col.x), max.z.max(col.z)),
            )
        });
        let columns_x = max.x - min.x + 1;
        let columns_z = max.z - min.z + 1;
        let volume = SampleGrid::new(
            IVec3::new(
                columns_x * QUARTS_PER_COLUMN + 2,
                (last - first + 1) * QUARTS_PER_COLUMN,
                columns_z * QUARTS_PER_COLUMN + 2,
            ),
            IVec3::new(min.x * 16 - 4, first * 16, min.z * 16 - 4),
            IVec3::splat(4),
        );
        let size = volume.size();
        let mut members = vec![false; usize::try_from(columns_x * columns_z).ok()?];
        let mut covered = vec![false; usize::try_from(size.x * size.z).ok()?];
        for col in columns {
            let (column_x, column_z) = (col.x - min.x, col.z - min.z);
            *members.get_mut(usize::try_from(column_z * columns_x + column_x).ok()?)? = true;
            for z in column_z * QUARTS_PER_COLUMN..column_z * QUARTS_PER_COLUMN + WINDOW {
                let row = usize::try_from(z * size.x).ok()?;
                let from = row + usize::try_from(column_x * QUARTS_PER_COLUMN).ok()?;
                covered.get_mut(from..from + WINDOW as usize)?.fill(true);
            }
        }

        let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
        let points = volume.len();
        let mut values = vec![0.0f32; roots.len() * points];
        noise_router.fill_roots(&mut Workspace::new(), &volume, &roots, &mut values);
        let mut rows = values.chunks_exact(points);
        let (Some(temperature), Some(vegetation), Some(continents), Some(erosion)) =
            (rows.next(), rows.next(), rows.next(), rows.next())
        else {
            return None;
        };
        let (Some(depth), Some(ridges)) = (rows.next(), rows.next()) else {
            return None;
        };

        let mut cursor = table.cursor();
        let mut ids = vec![0u8; points];
        let height = usize::try_from(size.y).ok()?;
        let quart_columns = ids.chunks_exact_mut(height).zip(&covered);
        for (quart_column, at) in quart_columns.zip((0..points).step_by(height)) {
            let (column, &true) = quart_column else {
                continue;
            };
            for (y, id) in column.iter_mut().enumerate() {
                let at = at + y;
                let target = TargetPoint::new(
                    *temperature.get(at)?,
                    *vegetation.get(at)?,
                    *continents.get(at)?,
                    *erosion.get(at)?,
                    *depth.get(at)?,
                    *ridges.get(at)?,
                );
                *id = *cursor.find_value(target)?;
            }
        }
        Some(Self {
            volume,
            min,
            columns_x,
            members,
            ids,
        })
    }

    #[must_use]
    pub fn window(&self, col: ColumnPos) -> Option<BiomeWindow<'_>> {
        let (column_x, column_z) = (col.x - self.min.x, col.z - self.min.z);
        if !(0..self.columns_x).contains(&column_x) {
            return None;
        }
        let member = usize::try_from(column_z * self.columns_x + column_x).ok()?;
        if !*self.members.get(member)? {
            return None;
        }
        let origin = self.volume.min_block();
        let window = SampleGrid::new(
            IVec3::new(WINDOW, self.volume.size().y, WINDOW),
            IVec3::new(col.x * 16 - 4, origin.y, col.z * 16 - 4),
            IVec3::splat(4),
        );
        Some(BiomeWindow::new(
            window,
            self.volume,
            column_x * QUARTS_PER_COLUMN,
            column_z * QUARTS_PER_COLUMN,
            &self.ids,
        ))
    }
}
