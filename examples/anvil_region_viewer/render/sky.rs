use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;
use bevy::render::view::ExtractedView;

use super::binds::Bindings;
use super::pipeline;
use super::shaders::Shaders;
use super::terrain::Terrain;
use super::Sky;

const STAR_COUNT: u32 = 1500;

const ADDITIVE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOperation::Add,
    },
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum SkyPart {
    Dome,
    Clouds,
}

pub(super) struct SkyDraw {
    label: &'static str,
    part: SkyPart,
    vertex: &'static str,
    fragment: &'static str,
    blend: Option<BlendState>,
    vertices: u32,
}

pub(super) const SKY_DRAWS: [SkyDraw; 5] = [
    SkyDraw {
        label: "sky disc",
        part: SkyPart::Dome,
        vertex: "vertex_disc",
        fragment: "fragment_disc",
        blend: None,
        vertices: 48,
    },
    SkyDraw {
        label: "sky twilight",
        part: SkyPart::Dome,
        vertex: "vertex_sunrise",
        fragment: "fragment_flat",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 48,
    },
    SkyDraw {
        label: "sky celestial",
        part: SkyPart::Dome,
        vertex: "vertex_celestial",
        fragment: "fragment_celestial",
        blend: Some(ADDITIVE),
        vertices: 12,
    },
    SkyDraw {
        label: "sky stars",
        part: SkyPart::Dome,
        vertex: "vertex_stars",
        fragment: "fragment_flat",
        blend: Some(ADDITIVE),
        vertices: STAR_COUNT * 6,
    },
    SkyDraw {
        label: "sky clouds",
        part: SkyPart::Clouds,
        vertex: "vertex_clouds",
        fragment: "fragment_clouds",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 3,
    },
];

pub(super) fn pipelines(
    shaders: &Shaders,
    binds: &Bindings,
    view: &ExtractedView,
    pipeline_cache: &PipelineCache,
) -> [CachedRenderPipelineId; SKY_DRAWS.len()] {
    SKY_DRAWS.map(|draw| {
        let writes_depth = draw.part == SkyPart::Clouds;
        let shader = match draw.part {
            SkyPart::Dome => &shaders.sky,
            SkyPart::Clouds => &shaders.clouds,
        };
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(writes_depth),
                depth_compare: Some(if writes_depth {
                    CompareFunction::GreaterEqual
                } else {
                    CompareFunction::Always
                }),
                stencil: default(),
                bias: default(),
            }),
            ..pipeline::common(
                draw.label.to_string(),
                vec![binds.view_layout.clone(), binds.sky_layout.clone()],
                shader,
                draw.vertex.to_string(),
                draw.fragment.to_string(),
                view,
                draw.blend,
            )
        })
    })
}

pub(super) fn draw<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    view_bind_group: &'pass BindGroup,
    view_offset: u32,
    pipeline_cache: &'pass PipelineCache,
    part: SkyPart,
) {
    pass.push_debug_group(match part {
        SkyPart::Dome => "sky",
        SkyPart::Clouds => "clouds",
    });
    pass.set_bind_group(1, &terrain.binds.sky, &[]);
    for (index, draw) in SKY_DRAWS.iter().enumerate() {
        if draw.part != part {
            continue;
        }
        let Some(pipeline) = terrain.pipelines.sky(index, pipeline_cache) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, view_bind_group, &[view_offset, 0]);
        pass.draw(0..draw.vertices, 0..1);
    }
    pass.pop_debug_group();
}

pub(super) fn prepare(sky: Res<Sky>, terrain: Option<Res<Terrain>>, queue: Res<RenderQueue>) {
    let Some(terrain) = terrain else {
        return;
    };
    queue.write_buffer(&terrain.frame.sky, 0, bytemuck::bytes_of(&*sky));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clouds_are_the_only_draw_that_writes_depth() {
        let clouds: Vec<&str> = SKY_DRAWS
            .iter()
            .filter(|draw| draw.part == SkyPart::Clouds)
            .map(|draw| draw.label)
            .collect();
        assert_eq!(clouds, ["sky clouds"]);
    }
}
