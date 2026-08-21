use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;

use super::sky;
use super::terrain::Terrain;

pub(super) const TERRAIN_PIPELINES: usize = 6;
pub(super) const UNTESTED_PIPELINE: usize = 4;

const TERRAIN_ENTRIES: [(&str, &str, bool); TERRAIN_PIPELINES] = [
    ("vertex_simple", "fragment_greedy_opaque", false),
    ("vertex_complex", "fragment_model_opaque", false),
    ("vertex_simple", "fragment_greedy_blend", true),
    ("vertex_complex", "fragment_model_blend", true),
    ("vertex_simple", "fragment_greedy_solid", false),
    ("vertex_complex", "fragment_model_solid", false),
];

pub(super) fn prepare_pipelines(
    mut terrain: Option<ResMut<Terrain>>,
    views: Query<&ExtractedView>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    if terrain.pipelines.is_some() {
        return;
    }
    let Some(view) = views.iter().next() else {
        return;
    };

    let pipelines = TERRAIN_ENTRIES.map(|(vertex, fragment, blend)| {
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("terrain".into()),
            layout: vec![terrain.view_layout.clone(), terrain.draw_layout.clone()],
            vertex: VertexState {
                shader: terrain.terrain_shader.clone(),
                entry_point: Some(vertex.into()),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: terrain.terrain_shader.clone(),
                entry_point: Some(fragment.into()),
                targets: vec![Some(ColorTargetState {
                    format: view.target_format,
                    blend: blend.then_some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: (!blend).then_some(Face::Back),
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(!blend),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: if vertex == "vertex_complex" && !blend {
                    DepthBiasState {
                        constant: 2,
                        slope_scale: 1.0,
                        clamp: 0.0,
                    }
                } else {
                    default()
                },
            }),
            multisample: MultisampleState {
                count: 1,
                ..default()
            },
            ..default()
        })
    });
    let sky_pipelines = sky::pipelines(terrain, view, &pipeline_cache);

    terrain.pipelines = Some(pipelines);
    terrain.sky_pipelines = Some(sky_pipelines);
}
