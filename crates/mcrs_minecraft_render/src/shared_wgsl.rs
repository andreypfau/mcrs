/// The gbuffer targets as a fragment output struct `GBuffer`, and `octahedral_encode` of a normal
/// as the lighting pass decodes it, for a shader that a caller assembles from WGSL text.
pub const GBUFFER_ENCODE_WGSL: &str = include_str!("shaders/include/gbuffer_encode.wesl");

/// The frustum plane tests `box_outside` and `sphere_outside`, and the depth pyramid test
/// `occluded` of a `Footprint` that `cover` grows, for a shader that a caller assembles from WGSL
/// text.
pub const VISIBILITY_WGSL: &str = include_str!("shaders/include/visibility.wesl");
