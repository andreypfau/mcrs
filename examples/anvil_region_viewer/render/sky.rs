use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::binding_types::{sampler, texture_2d_array};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderQueue;
use bevy::render::view::ExtractedView;

use super::Sky;
use super::terrain::Terrain;

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

pub(super) struct SkyDraw {
    label: &'static str,
    vertex: &'static str,
    fragment: &'static str,
    blend: Option<BlendState>,
    vertices: u32,
    depth: bool,
}

pub(super) const SKY_DRAWS: [SkyDraw; 5] = [
    SkyDraw {
        label: "sky disc",
        vertex: "vertex_disc",
        fragment: "fragment_disc",
        blend: None,
        vertices: 48,
        depth: false,
    },
    SkyDraw {
        label: "sky twilight",
        vertex: "vertex_sunrise",
        fragment: "fragment_flat",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 48,
        depth: false,
    },
    SkyDraw {
        label: "sky celestial",
        vertex: "vertex_celestial",
        fragment: "fragment_celestial",
        blend: Some(ADDITIVE),
        vertices: 12,
        depth: false,
    },
    SkyDraw {
        label: "sky stars",
        vertex: "vertex_stars",
        fragment: "fragment_flat",
        blend: Some(ADDITIVE),
        vertices: STAR_COUNT * 6,
        depth: false,
    },
    SkyDraw {
        label: "sky clouds",
        vertex: "vertex_clouds",
        fragment: "fragment_clouds",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 3,
        depth: true,
    },
];

pub(super) fn bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "sky data",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
            ),
        ),
    )
}

pub(super) fn pipelines(
    terrain: &Terrain,
    view: &ExtractedView,
    pipeline_cache: &PipelineCache,
) -> [CachedRenderPipelineId; SKY_DRAWS.len()] {
    SKY_DRAWS.map(|draw| {
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(draw.label.into()),
            layout: vec![terrain.view_layout.clone(), terrain.sky_layout.clone()],
            vertex: VertexState {
                shader: terrain.sky_shader.clone(),
                entry_point: Some(draw.vertex.into()),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: terrain.sky_shader.clone(),
                entry_point: Some(draw.fragment.into()),
                targets: vec![Some(ColorTargetState {
                    format: view.target_format,
                    blend: draw.blend,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(draw.depth),
                depth_compare: Some(if draw.depth {
                    CompareFunction::GreaterEqual
                } else {
                    CompareFunction::Always
                }),
                stencil: default(),
                bias: default(),
            }),
            multisample: MultisampleState {
                count: 1,
                ..default()
            },
            ..default()
        })
    })
}

pub(super) fn draw<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    terrain: &'pass Terrain,
    view_bind_group: &'pass BindGroup,
    view_offset: u32,
    pipeline_cache: &'pass PipelineCache,
    depth: bool,
) {
    let Some(sky) = terrain.sky_pipelines else {
        return;
    };
    pass.set_bind_group(1, &terrain.sky_bind_group, &[]);
    for (index, draw) in SKY_DRAWS.iter().enumerate() {
        if draw.depth != depth {
            continue;
        }
        let Some(pipeline) = pipeline_cache.get_render_pipeline(sky[index]) else {
            continue;
        };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, view_bind_group, &[view_offset, 0]);
        pass.draw(0..draw.vertices, 0..1);
    }
}

pub(super) fn prepare(sky: Res<Sky>, terrain: Option<Res<Terrain>>, queue: Res<RenderQueue>) {
    let Some(terrain) = terrain else {
        return;
    };
    queue.write_buffer(&terrain.sky, 0, bytemuck::bytes_of(&*sky));
}
