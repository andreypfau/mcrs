use mcrs_voxel::{BlockPos, LocalPos, SectionPos};

pub(crate) type SectionOrder = (i32, i32, i32);

pub(crate) fn section_order(section: SectionPos) -> SectionOrder {
    (section.x, section.z, section.y)
}

pub(crate) fn section_at(order: SectionOrder) -> SectionPos {
    let (x, z, y) = order;
    SectionPos::new(x, y, z)
}

pub(crate) fn block_pos(section: SectionPos, local: LocalPos) -> BlockPos {
    let axis = |section: i32, offset: u8| section.wrapping_shl(4) | i32::from(offset);
    BlockPos::new(
        axis(section.x, local.x()),
        axis(section.y, local.y()),
        axis(section.z, local.z()),
    )
}

pub(crate) fn bytes<E>(capacity: usize) -> usize {
    capacity.saturating_mul(size_of::<E>())
}

pub(crate) fn table_bytes<E>(capacity: usize) -> usize {
    capacity.saturating_mul(size_of::<E>().saturating_add(1))
}
