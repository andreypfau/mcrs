#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]

mod block_entity_parity;
mod block_face_parity;
mod block_model_parity;
mod client_pack;
