// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames.
#![recursion_limit = "256"]
//! The GPU side of the entity rendering: the slot records of each mob, the static tables of the
//! models, the pose programs, and the entity cull, pose, expand and draw passes.

// std: bevy

mod cpu_inputs;
mod entity_assets;
mod entity_gpu;
mod entity_library;
mod entity_pack;
mod frame_uniform;
mod gpu_buffers;
mod gpu_layouts;
mod gpu_pipelines;
mod main_world;
mod plugin;
mod pose_program;
mod records;
mod render_world;
mod slot_upload;

pub use cpu_inputs::input_record;
pub use entity_assets::{
    EntityAssets, MISSING_TEXTURE, VariantInfo, VariantKey, texture_identifier,
};
pub use entity_gpu::{Capacities, ComputePass, DrawTargets, EntityGpu, PassQueries};
pub use entity_library::{
    DefinitionSource, DefinitionTables, EntityLibrary, MAX_ARRAYS, MAX_DEFINITIONS, NO_PALETTE,
    TextureArray, TextureImage, VariantRecord,
};
pub use entity_pack::{DEATH_FLIP, EntityPack, PackEntity};
pub use frame_uniform::{FRAME_NETHER_LIGHT, FULL_RIG_HEIGHT, FrameUniform, FrameView};
pub use main_world::{CpuAnimation, EntityClock, EntitySlot, RecordedClip, RecordedClips};
pub use plugin::{EntityRenderPlugin, EntitySlotSystems};
pub use pose_program::{EntryInputs, InputLayout, PoseProgram, PoseProgramError};
pub use records::*;
pub use render_world::FULL_RIG_CAP;
pub use slot_upload::{PAGE_SLOTS, RecordRange, SCATTER_FRESH, SlotArena, SlotUpload};
