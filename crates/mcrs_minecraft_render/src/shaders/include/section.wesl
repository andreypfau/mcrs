import mcrs_minecraft_render::shaders::include::fields::SECTION_SIZE;
import mcrs_minecraft_render::shaders::include::frame::camera;
import mcrs_minecraft_render::shaders::include::section_layout::SectionDesc;

fn section_origin(desc: SectionDesc) -> vec3<f32> {
    let delta = vec3<i32>(desc.x, desc.y, desc.z) - camera.section;
    return vec3<f32>(delta * i32(SECTION_SIZE));
}
