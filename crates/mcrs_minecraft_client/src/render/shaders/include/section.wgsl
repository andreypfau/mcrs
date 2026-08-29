#define_import_path mcrs_minecraft_client::section

#import mcrs_minecraft_client::fields::SECTION_SIZE

const CULLED: u32 = 0xffffffffu;

struct SectionDesc {
    x: i32,
    y: i32,
    z: i32,
    scale: u32,
    face_base: u32,
}

fn section_origin(desc: SectionDesc) -> vec3<f32> {
    return vec3<f32>(f32(desc.x), f32(desc.y), f32(desc.z)) * SECTION_SIZE;
}

fn section_span(desc: SectionDesc) -> f32 {
    return SECTION_SIZE * f32(desc.scale);
}

fn degenerate() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
