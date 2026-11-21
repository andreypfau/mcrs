use crate::biome_window::BiomeWindow;
use crate::multi_noise_biomes::MultiNoiseBiomeTable;
use core::ops::Range;
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
const MAX_COLUMNS: i32 = 64;

pub struct ClimateBox {
    volume: SampleGrid,
    min: ColumnPos,
    columns_x: i32,
    members: Vec<Member>,
    ids: Vec<u8>,
}

#[derive(Clone)]
enum Member {
    Absent,
    Shared,
    Alone(Vec<u8>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quart {
    Outside,
    Inside,
    Tied,
}

struct Roots<'v> {
    rows: [&'v [f32]; 6],
}

impl Roots<'_> {
    fn target(&self, at: usize) -> Option<TargetPoint> {
        let [temperature, vegetation, continents, erosion, depth, ridges] = self.rows;
        Some(TargetPoint::new(
            *temperature.get(at)?,
            *vegetation.get(at)?,
            *continents.get(at)?,
            *erosion.get(at)?,
            *depth.get(at)?,
            *ridges.get(at)?,
        ))
    }
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
        let columns_x = max.x.checked_sub(min.x)?.checked_add(1)?;
        let columns_z = max.z.checked_sub(min.z)?.checked_add(1)?;
        if columns_x.checked_mul(columns_z)? > MAX_COLUMNS {
            return None;
        }
        last.checked_add(1)?.checked_mul(16)?;
        let volume = SampleGrid::new(
            IVec3::new(
                columns_x * QUARTS_PER_COLUMN + 2,
                last.checked_sub(first)?
                    .checked_add(1)?
                    .checked_mul(QUARTS_PER_COLUMN)?,
                columns_z * QUARTS_PER_COLUMN + 2,
            ),
            IVec3::new(
                window_origin(min.x, max.x)?,
                first.checked_mul(16)?,
                window_origin(min.z, max.z)?,
            ),
            IVec3::splat(4),
        );
        let size = volume.size();
        let mut members = vec![Member::Absent; usize::try_from(columns_x * columns_z).ok()?];
        let mut quarts = vec![Quart::Outside; usize::try_from(size.x * size.z).ok()?];
        for col in columns.clone() {
            let (column_x, column_z) = (col.x - min.x, col.z - min.z);
            *members.get_mut(usize::try_from(column_z * columns_x + column_x).ok()?)? =
                Member::Shared;
            for row in window_rows(size.x, column_x, column_z) {
                quarts.get_mut(row?)?.fill(Quart::Inside);
            }
        }

        let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
        let points = volume.len();
        let mut values = vec![0.0f32; roots.len() * points];
        noise_router.fill_roots(&mut Workspace::new(), &volume, &roots, &mut values);
        let mut rows = values.chunks_exact(points);
        let roots = Roots {
            rows: [
                rows.next()?,
                rows.next()?,
                rows.next()?,
                rows.next()?,
                rows.next()?,
                rows.next()?,
            ],
        };

        let mut cursor = table.cursor();
        let mut ids = vec![0u8; points];
        let height = usize::try_from(size.y).ok()?;
        let quart_columns = ids.chunks_exact_mut(height).zip(&mut quarts);
        for ((column, quart), at) in quart_columns.zip((0..points).step_by(height)) {
            if *quart == Quart::Outside {
                continue;
            }
            for (y, id) in column.iter_mut().enumerate() {
                let Some(&found) = cursor.find_unique_value(roots.target(at + y)?) else {
                    *quart = Quart::Tied;
                    break;
                };
                *id = found;
            }
        }
        for col in columns {
            let (column_x, column_z) = (col.x - min.x, col.z - min.z);
            let member = members.get_mut(usize::try_from(column_z * columns_x + column_x).ok()?)?;
            if !matches!(member, Member::Shared) {
                continue;
            }
            let mut tied = false;
            for row in window_rows(size.x, column_x, column_z) {
                tied |= quarts.get(row?)?.contains(&Quart::Tied);
            }
            if tied {
                *member = Member::Alone(sample_alone(table, &roots, size, column_x, column_z)?);
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
        let column_x = col.x.checked_sub(self.min.x)?;
        let column_z = col.z.checked_sub(self.min.z)?;
        if !(0..self.columns_x).contains(&column_x) {
            return None;
        }
        let member = column_z
            .checked_mul(self.columns_x)?
            .checked_add(column_x)?;
        let member = usize::try_from(member).ok()?;
        let origin = self.volume.min_block();
        let window = SampleGrid::new(
            IVec3::new(WINDOW, self.volume.size().y, WINDOW),
            IVec3::new(col.x * 16 - 4, origin.y, col.z * 16 - 4),
            IVec3::splat(4),
        );
        match self.members.get(member)? {
            Member::Absent => None,
            Member::Shared => Some(BiomeWindow::new(
                window,
                self.volume,
                column_x * QUARTS_PER_COLUMN,
                column_z * QUARTS_PER_COLUMN,
                &self.ids,
            )),
            Member::Alone(ids) => Some(BiomeWindow::new(window, window, 0, 0, ids)),
        }
    }
}

fn window_origin(min: i32, max: i32) -> Option<i32> {
    max.checked_add(1)?.checked_mul(16)?.checked_add(4)?;
    min.checked_mul(16)?.checked_sub(4)
}

fn window_rows(
    size_x: i32,
    column_x: i32,
    column_z: i32,
) -> impl Iterator<Item = Option<Range<usize>>> {
    let z = column_z * QUARTS_PER_COLUMN;
    (z..z + WINDOW).map(move |z| {
        let from = usize::try_from(z * size_x + column_x * QUARTS_PER_COLUMN).ok()?;
        Some(from..from + WINDOW as usize)
    })
}

fn sample_alone(
    table: &MultiNoiseBiomeTable,
    roots: &Roots<'_>,
    size: IVec3,
    column_x: i32,
    column_z: i32,
) -> Option<Vec<u8>> {
    let height = usize::try_from(size.y).ok()?;
    let mut cursor = table.cursor();
    let mut ids = Vec::with_capacity(height * (WINDOW * WINDOW) as usize);
    for row in window_rows(size.x, column_x, column_z) {
        for quart in row? {
            for y in 0..height {
                ids.push(*cursor.find_value(roots.target(quart * height + y)?)?);
            }
        }
    }
    Some(ids)
}
