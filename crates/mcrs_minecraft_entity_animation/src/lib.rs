//! Entity animation on the CPU: the clips of animation files, the play list and the scripts of
//! a client entity, and the bone pose of each frame.

// std: mcrs_minecraft_pack_file

mod channel;
mod clip;
mod clip_library;
mod entity_animation;
mod entity_query;
mod instance;
mod pose;
mod pose_binding;
mod query_source;
mod rig;
mod rig_query;

pub use channel::{Channel, Keyframe, LerpMode, Segment};
pub use clip::{Clip, ClipBone, LoopMode, TimeUpdate};
pub use clip_library::ClipLibrary;
pub use entity_animation::EntityAnimation;
pub use entity_query::EntityQuery;
pub use instance::{Instance, Phase};
pub use pose::{Pose, PoseBone};
pub use pose_binding::PoseBinding;
pub use query_source::QuerySource;
pub use rig::{AnimateEntry, AnimateTarget, Rig};
