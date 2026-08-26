
#import anvil_region_viewer::frame::{sky, view}
#import anvil_region_viewer::sky_bindings::clouds

const CLOUD_CELL: f32 = 12.0;
const CLOUD_THICKNESS: f32 = 4.0;
const CLOUD_OPEN: f32 = 10.0 / 255.0;
const CLOUD_STEPS: u32 = 192u;
const CLOUD_TOP: f32 = 1.0;
const CLOUD_BOTTOM: f32 = 0.7;
const CLOUD_SIDE_X: f32 = 0.9;
const CLOUD_SIDE_Z: f32 = 0.8;

struct CloudVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

struct CloudFragment {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@vertex
fn vertex_clouds(@builtin(vertex_index) index: u32) -> CloudVertex {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: CloudVertex;
    out.ndc = corner * 2.0 - 1.0;
    out.clip_position = vec4<f32>(out.ndc, 1.0, 1.0);
    return out;
}

fn cloud_cell(cell: vec2<i32>, size: vec2<i32>) -> vec4<f32> {
    return textureLoad(clouds, cell & (size - vec2<i32>(1)), 0, 0);
}

@fragment
fn fragment_clouds(in: CloudVertex) -> CloudFragment {
    var out: CloudFragment;
    out.color = vec4<f32>(0.0);
    out.depth = 0.0;

    let origin = view.world_position.xyz;
    let near = view.world_from_clip * vec4<f32>(in.ndc, 1.0, 1.0);
    let direction = normalize(near.xyz / near.w - origin);

    let bottom = sky.cloud.x;
    let top = bottom + CLOUD_THICKNESS;
    let fade = sky.cloud.w;

    var enter = 0.0;
    var leave = fade;
    if abs(direction.y) < 1e-6 {
        if origin.y < bottom || origin.y > top {
            discard;
        }
    } else {
        let first = (bottom - origin.y) / direction.y;
        let second = (top - origin.y) / direction.y;
        enter = max(0.0, min(first, second));
        leave = min(fade, max(first, second));
    }
    if enter > leave {
        discard;
    }

    let size = vec2<i32>(textureDimensions(clouds));
    let span = f32(size.x) * CLOUD_CELL;
    let drift = vec2<f32>(sky.cloud.y % span, sky.cloud.z);
    let start = (origin.xz + drift + direction.xz * enter) / CLOUD_CELL;

    let corner = floor(start);
    let step = sign(direction.xz);
    let per_cell = CLOUD_CELL / max(abs(direction.xz), vec2<f32>(1e-6));
    var next = enter
        + select(start - corner, corner + 1.0 - start, step > vec2<f32>(0.0)) * per_cell;
    var cell = vec2<i32>(corner);
    let cell_step = vec2<i32>(step);

    var shade = select(CLOUD_BOTTOM, CLOUD_TOP, direction.y < 0.0);
    var distance = enter;
    var filled = vec4<f32>(0.0);
    for (var taken = 0u; taken < CLOUD_STEPS; taken = taken + 1u) {
        let sample = cloud_cell(cell, size);
        if sample.a >= CLOUD_OPEN {
            filled = sample;
            break;
        }
        if next.x < next.y {
            distance = next.x;
            next.x = next.x + per_cell.x;
            cell.x = cell.x + cell_step.x;
            shade = CLOUD_SIDE_X;
        } else {
            distance = next.y;
            next.y = next.y + per_cell.y;
            cell.y = cell.y + cell_step.y;
            shade = CLOUD_SIDE_Z;
        }
        if distance > leave {
            break;
        }
    }
    if filled.a < CLOUD_OPEN {
        discard;
    }

    let position = origin + direction * distance;
    let clip = view.clip_from_world * vec4<f32>(position, 1.0);
    out.depth = clip.z / clip.w;
    out.color = vec4<f32>(
        filled.rgb * sky.cloud_color.rgb * shade,
        filled.a * sky.cloud_color.a * (1.0 - clamp(distance / fade, 0.0, 1.0)),
    );
    return out;
}
