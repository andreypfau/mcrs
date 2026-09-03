#define_import_path mcrs_minecraft_client::section

#import mcrs_minecraft_client::fields::SECTION_SIZE
#import mcrs_minecraft_client::frame::camera

const CULLED: u32 = 0xffffffffu;

struct SectionDesc {
    x: i32,
    y: i32,
    z: i32,
    scale: u32,
    face_base: u32,
}

/// Against the origin of the camera's own section: the subtraction is exact in i32, and only
/// the small result becomes an f32.
fn section_origin(desc: SectionDesc) -> vec3<f32> {
    let delta = vec3<i32>(desc.x, desc.y, desc.z) - camera.section;
    return vec3<f32>(delta * i32(SECTION_SIZE));
}

fn section_span(desc: SectionDesc) -> f32 {
    return SECTION_SIZE * f32(desc.scale);
}
