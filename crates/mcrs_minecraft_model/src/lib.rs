//! Block and item models baked from a resource pack, with no world, network or client: the
//! pack, the model files, the sprite atlas, the block catalog and the item model trees.
// std: bevy
#![cfg_attr(
    test,
    expect(clippy::disallowed_types, reason = "test input is written as JSON")
)]

pub mod anim;
pub mod atlas;
pub mod bake;
pub mod bake_error;
mod block_build;
pub mod block_catalog;
pub mod block_entity_error;
pub mod block_entity_model;
pub mod catalog_failure;
pub mod colormap;
pub mod item_model_asset;
pub mod item_model_bake;
pub mod item_model_eval;
pub mod item_model_generator;
pub mod item_model_transform;
pub mod resource_set;
pub mod sprite_error;
pub mod vanilla_source;
