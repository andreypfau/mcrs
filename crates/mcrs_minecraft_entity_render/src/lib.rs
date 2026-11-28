#![recursion_limit = "256"]

mod counts_readback;
mod cpu_inputs;
mod entity_assets;
mod entity_assets_build;
mod entity_gpu;
mod entity_library;
mod entity_pack;
mod entity_views;
mod figure_draw;
mod figure_gpu;
mod figure_targets;
mod figure_view;
mod frame_posed;
mod frame_queries;
mod frame_uniform;
mod gpu_buffers;
mod gpu_layouts;
mod gpu_pipelines;
mod height_threshold;
mod impostor_atlas;
mod impostor_frame;
mod main_world;
mod player_skin;
mod plugin;
mod pose_program;
mod records;
mod render_world;
mod repository_pack;
mod skin_image;
mod slot_upload;
mod variant_source;

pub use counts_readback::EntityFrameCounts;
pub use cpu_inputs::input_record;
pub use entity_assets::{EntityAssets, VariantInfo, VariantKey};
pub use entity_gpu::{Capacities, ComputePass, DrawTargets, EntityGpu, PassQueries};
pub use entity_library::{
    DefinitionSource, DefinitionTables, EntityLibrary, TextureArray, TextureImage, VariantRecord,
};
pub use entity_pack::{EntityPack, PackEntity};
pub use entity_views::EntityViews;
pub use figure_draw::FigureDraw;
pub use figure_targets::FigureTargets;
pub use figure_view::{FigureFrame, FigureView};
pub use frame_posed::FramePosed;
pub use frame_queries::{FrameQueries, VariantEntry, pose_from_name};
pub use frame_uniform::FrameUniform;
pub use height_threshold::HeightThreshold;
pub use impostor_frame::{IMPOSTOR_VIEWS, ImpostorFrames, ImpostorView};
pub use main_world::{CpuAnimation, EntityClock, EntitySlot, RecordedClip, RecordedClips};
pub use player_skin::{PlayerModel, PlayerSkin, SKIN_SLOTS};
pub use plugin::{
    EntityPasses, EntityRenderPlugin, EntitySlotSystems, FigureViewSystems, entity_passes,
};
pub use pose_program::{EntryInputs, InputLayout, PoseProgram, PoseProgramError, QueryInput};
pub use records::{
    CULL_VALID, ClipSlot, CullRecord, InputRecord, STATE_BABY, STATE_HIDDEN_SHIFT, ScatterEntry,
    StateRecord, TickRecord, f16_up,
};
pub use render_world::FULL_RIG_CAP;
pub use skin_image::{SKIN_SIDE, SkinError, SkinImage};
pub use slot_upload::{RecordRange, SlotArena, SlotUpload};
