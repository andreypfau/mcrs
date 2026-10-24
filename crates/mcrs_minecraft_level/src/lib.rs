#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
pub mod aoi;
pub mod block_update;
pub mod entity;
pub mod palette;
pub mod server_loop;
pub mod session;
pub mod tick;
pub mod voxel_update;
pub mod world;
