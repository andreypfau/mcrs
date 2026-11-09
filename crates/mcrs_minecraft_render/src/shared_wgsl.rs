const GBUFFER_ENCODE: &str = include_str!("shaders/include/gbuffer_encode.wgsl");
const VISIBILITY: &str = include_str!("shaders/include/visibility.wgsl");

/// The gbuffer targets as a fragment output struct `GBuffer`, and `octahedral_encode` of a normal
/// as the lighting pass decodes it, for a shader that a caller assembles from WGSL text.
#[must_use]
pub fn gbuffer_encode_wgsl() -> &'static str {
    without_import_path(GBUFFER_ENCODE)
}

/// The frustum plane tests `box_outside` and `sphere_outside`, and the depth pyramid test
/// `occluded` of a `Footprint` that `cover` grows, for a shader that a caller assembles from WGSL
/// text.
#[must_use]
pub fn visibility_wgsl() -> &'static str {
    without_import_path(VISIBILITY)
}

fn without_import_path(source: &'static str) -> &'static str {
    source
        .strip_prefix("#define_import_path")
        .and_then(|rest| rest.split_once('\n'))
        .map_or(source, |(_, body)| body)
}
