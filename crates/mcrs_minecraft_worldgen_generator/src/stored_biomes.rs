use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::SectionPos;

type Container = VoxelPalette<u8, { SectionPos::SIZE }>;

const EDGE: i32 = SectionPos::SIZE as i32;

pub fn column_cell(first_section_y: i32, section_count: usize, y: i32) -> (usize, usize) {
    if section_count == 0 {
        return (0, 0);
    }
    let lowest = first_section_y * EDGE;
    let highest = (first_section_y + section_count as i32) * EDGE - 1;
    let above_lowest = y.clamp(lowest, highest) - lowest;
    (
        (above_lowest / EDGE) as usize,
        (above_lowest % EDGE) as usize,
    )
}

// chisle: a height outside the column reads the edge block layer, while the reference's horizontal pick at that height can land on another cell; lifted by storing rows beyond the column.
pub fn stored_biome(sections: &[Container], first_section_y: i32, x: i32, y: i32, z: i32) -> u8 {
    if sections.is_empty() {
        return 0;
    }
    let (section, local_y) = column_cell(first_section_y, sections.len(), y);
    sections[section].get_cell(
        x.rem_euclid(EDGE) as usize,
        local_y,
        z.rem_euclid(EDGE) as usize,
    )
}
