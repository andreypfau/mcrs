import mcrs_minecraft_render::shaders::include::fields::Group;
import mcrs_minecraft_render::shaders::include::section_layout::SectionDesc;

struct DrawArgs {
    vertex_count: atomic<u32>,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

const DRAW_KINDS: u32 = 6u;
const ROW_FIRST_PASS_DRAWS: u32 = 0u;
const ROW_SECOND_PASS_DRAWS: u32 = 1u;
const ROW_HIDDEN_QUADS: u32 = 2u;
const ROW_FIRST_PASS_SURVIVORS: u32 = 3u;
const ROW_SECOND_PASS_SURVIVORS: u32 = 4u;
const ROW_CANDIDATES: u32 = 5u;
const ROW_FIRST_PASS_QUAD_DISPATCH: u32 = 6u;
const ROW_SECOND_PASS_QUAD_DISPATCH: u32 = 7u;
const ROW_CANDIDATE_DISPATCH: u32 = 8u;

@group(1) @binding(0) var<storage, read> groups: array<Group>;
@group(1) @binding(1) var<storage, read_write> visible: array<vec2<u32>>;
@group(1) @binding(2) var<storage, read_write> args: array<DrawArgs>;
@group(1) @binding(3) var<storage, read_write> cave_visible: array<u32>;
@group(1) @binding(4) var<storage, read> sections: array<SectionDesc>;
@group(1) @binding(5) var<storage, read_write> batches: array<u32>;
@group(1) @binding(6) var hiz: texture_2d<f32>;
@group(1) @binding(7) var<storage, read_write> candidates: array<u32>;
@group(1) @binding(8) var<storage, read> quad_words: array<u32>;
@group(1) @binding(9) var<storage, read> model_words: array<u32>;
