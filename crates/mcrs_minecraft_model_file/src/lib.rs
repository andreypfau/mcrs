//! Block model files, block state files and the face bake of a resource pack, with no Bevy, so
//! a tool bakes block faces without the asset server, the image decoder or the task pools.
#![cfg_attr(
    test,
    expect(clippy::disallowed_types, reason = "test input is written as JSON")
)]

pub mod block_state_file;
pub mod block_state_file_error;
pub mod content_error;
pub mod face_bake;
pub mod face_bake_error;
pub mod model_file;
pub mod model_file_error;
pub mod model_resolver;
pub mod pack_files;
pub mod property_table;
pub mod resource_id;
