use mcrs_minecraft_core::BlockStateId;

mod access;
mod sections;
mod stores;
mod views;

pub(crate) fn states(count: u16) -> Vec<BlockStateId> {
    (0..count)
        .filter_map(|n| BlockStateId::checked(n, usize::from(count)))
        .collect()
}
