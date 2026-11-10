//! Entity geometry files of the Bedrock format: reading the legacy and the current shape,
//! the conversion to internal space, the bake to a static model, the bone transform and the
//! GPU records of a model.

// std: mcrs_minecraft_pack_file

mod bake;
mod baked_geometry;
mod bone_pose;
mod cube;
mod cuboid_records;
mod diagnostic;
mod explicit_face_record;
mod face;
mod format_version;
mod geometry;
mod geometry_error;
mod geometry_file;
mod geometry_file_bone;
mod gpu_record;
mod load_error;
mod resolve_parents;
pub mod space;
mod uv_rect;

pub use bake::{MAX_BONES, MAX_CUBES, MAX_DEPTH};
pub use baked_geometry::{
    BakedBone, BakedCube, BakedFace, BakedGeometry, BakedLocator, BoxUv, CuboidShape, VisibleBounds,
};
pub use bone_pose::BonePose;
pub use cube::{Cube, CubeUv, FaceUv, PerFaceUv, UvRotation};
pub use cuboid_records::CuboidRecords;
pub use diagnostic::Diagnostic;
pub use explicit_face_record::{EXPLICIT_FACE_MIRROR, ExplicitFaceRecord};
pub use face::{FACE_TRIANGLES, Face, UnknownFace};
pub use format_version::{FormatVersion, InvalidFormatVersion, Shape};
pub use geometry::{Bone, Geometry, Locator};
pub use geometry_error::GeometryError;
pub use geometry_file::GeometryFile;
pub use gpu_record::{
    BONE_NEVER_RENDER, BoneRecord, CUBOID_EXPLICIT, CUBOID_MIRROR, CuboidRecord, INFLATE_STEP,
    ModelRecord, NO_PARENT, RecordError,
};
pub use load_error::LoadError;
pub use resolve_parents::resolve_parents;
pub use uv_rect::{UvRect, rotate_corners};
