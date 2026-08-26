#define_import_path anvil_region_viewer::region

#import anvil_region_viewer::fields::{
    LOCAL_X_SHIFT, LOCAL_X_BITS,
    LOCAL_Y_SHIFT, LOCAL_Y_BITS,
    LOCAL_Z_SHIFT, LOCAL_Z_BITS,
    SECTION_SIZE,
}

const CULLED: u32 = 0xffffffffu;

/// A section number carries its own place inside the render region, so the world position of
/// a section is the region corner plus that local step.
fn section_origin(section: u32, region: vec3<f32>) -> vec3<f32> {
    let local = vec3<f32>(
        f32(extractBits(section, LOCAL_X_SHIFT, LOCAL_X_BITS)),
        f32(extractBits(section, LOCAL_Y_SHIFT, LOCAL_Y_BITS)),
        f32(extractBits(section, LOCAL_Z_SHIFT, LOCAL_Z_BITS)),
    );
    return region + local * SECTION_SIZE;
}

fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
