use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{EROSION, NoiseRouter};

use crate::multi_noise_biomes::BiomeGrid;
use crate::structures::index::{EndBiomes, is_central_end_chunk};

/// The quart grid of an end column, with the ring of [`crate::multi_noise_grid`]. A quart takes
/// the biome of its chunk, picked by the erosion at the chunk's centre column, so the grid samples
/// the centres of the three by three chunks around the column for every quart row in one pass.
pub(crate) fn end_biome_grid(
    noise_router: &NoiseRouter,
    end: &EndBiomes<u8>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Option<BiomeGrid> {
    let (&first, &last) = (y_sections.first()?, y_sections.last()?);
    let rows = (last - first + 1) * 4;
    let row_count = usize::try_from(rows).ok().filter(|&count| count > 0)?;
    let volume = SampleGrid::new(
        IVec3::new(6, rows, 6),
        IVec3::new(block_x - 4, first * 16, block_z - 4),
        IVec3::splat(4),
    );
    let (chunk_x, chunk_z) = (block_x >> 4, block_z >> 4);
    let mut ring = (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (chunk_x + dx, chunk_z + dz)));
    if ring.all(|(x, z)| is_central_end_chunk(x, z)) {
        let ids = vec![end.end; volume.len()];
        return Some(BiomeGrid { volume, ids });
    }
    let centres = SampleGrid::new(
        IVec3::new(3, rows, 3),
        IVec3::new(chunk_x * 16 - 8, first * 16, chunk_z * 16 - 8),
        IVec3::new(16, 4, 16),
    );
    let mut erosion = vec![0.0f32; centres.len()];
    noise_router.fill_roots(&mut Workspace::new(), &centres, &[EROSION], &mut erosion);

    let mut ids = Vec::with_capacity(volume.len());
    for z in 0..6 {
        for x in 0..6 {
            let quart_chunk_x = volume.block_x(x) >> 4;
            let quart_chunk_z = volume.block_z(z) >> 4;
            let centre = (quart_chunk_x - chunk_x + 1) + (quart_chunk_z - chunk_z + 1) * 3;
            let column = erosion
                .chunks_exact(row_count)
                .nth(usize::try_from(centre).ok()?)?;
            ids.extend(
                column
                    .iter()
                    .map(|&erosion| end.pick(quart_chunk_x, quart_chunk_z, || f64::from(erosion))),
            );
        }
    }
    Some(BiomeGrid { volume, ids })
}
