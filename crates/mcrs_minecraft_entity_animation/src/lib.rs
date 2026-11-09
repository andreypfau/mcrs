//! Entity animation on the CPU: the clips of animation files, the play list and the scripts of
//! a client entity, and the bone pose of each frame.

// std: mcrs_minecraft_entity_model

mod animation_diagnostic;
mod channel;
mod client_entity_definition;
mod clip;
mod clip_library;
mod entity_animation;
mod entity_query;
mod field_compiler;
mod frame_queries;
mod instance;
mod pack_file_error;
mod pose;
mod pose_binding;
mod query_source;
mod rig;
mod rig_query;

pub use animation_diagnostic::{AnimationDiagnostic, DiagnosticKind};
pub use channel::{Channel, Keyframe, LerpMode, Segment};
pub use client_entity_definition::{AnimateItem, AnimationName, ClientEntityDefinition, Scripts};
pub use clip::{Clip, ClipBone, LoopMode, TimeUpdate};
pub use clip_library::ClipLibrary;
pub use entity_animation::EntityAnimation;
pub use entity_query::EntityQuery;
pub use frame_queries::{FrameQueries, VariantEntry, pose_from_name, pose_name};
pub use instance::{Instance, Phase};
pub use pack_file_error::PackFileError;
pub use pose::{Pose, PoseBone};
pub use pose_binding::PoseBinding;
pub use query_source::QuerySource;
pub use rig::{AnimateEntry, AnimateTarget, Rig};
