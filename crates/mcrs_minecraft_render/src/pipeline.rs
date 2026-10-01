use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use bevy::shader::ShaderDefVal;

use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_mesh::{STREAMS, stream_pass};

use super::binds::Bindings;
use super::layer::{Shape, blend};
use super::shaders::Shaders;
use super::terrain::Terrain;

pub const TERRAIN_PIPELINES: usize = Pass::COUNT * Shape::ALL.len() * 2;

pub const fn terrain_slot(layer: Pass, shape: Shape, wireframe: bool) -> usize {
    (layer as usize * Shape::ALL.len() + shape as usize) * 2 + wireframe as usize
}

pub fn stream_slot(stream: u32, wireframe: bool) -> usize {
    terrain_slot(stream_pass(stream), Shape::of_stream(stream), wireframe)
}

pub fn common(
    label: String,
    layout: Vec<BindGroupLayoutDescriptor>,
    shader: &Handle<Shader>,
    vertex: String,
    fragment: String,
    view: &ExtractedView,
    blend: Option<BlendState>,
) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: Some(label.into()),
        layout,
        vertex: VertexState {
            shader: shader.clone(),
            entry_point: Some(vertex.into()),
            ..default()
        },
        fragment: Some(FragmentState {
            shader: shader.clone(),
            entry_point: Some(fragment.into()),
            targets: vec![Some(ColorTargetState {
                format: view.target_format,
                blend,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        multisample: MultisampleState {
            count: 1,
            ..default()
        },
        ..default()
    }
}

pub(super) struct Pipelines {
    pub shaders: Shaders,
    pub cull_sections: CachedComputePipelineId,
    pub cull_groups: CachedComputePipelineId,
    pub cull_quads: CachedComputePipelineId,
    pub cull_count: CachedComputePipelineId,
    pub cull_scan: CachedComputePipelineId,
    pub cull_scatter: CachedComputePipelineId,
    pub cull_groups_second: CachedComputePipelineId,
    pub cull_quads_second: CachedComputePipelineId,
    terrain: Option<[CachedRenderPipelineId; TERRAIN_PIPELINES]>,
}

impl Pipelines {
    pub fn new(
        shaders: Shaders,
        binds: &Bindings,
        section_slots: usize,
        max_workgroups: u32,
        subgroups: bool,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let layout = vec![binds.view_layout.clone(), binds.cull_layout.clone()];
        let quad_layout = vec![binds.view_layout.clone(), binds.quad_cull_layout.clone()];
        let mut shader_defs = vec![
            ShaderDefVal::UInt("CULL_THREADS".into(), super::pass::CULL_THREADS),
            ShaderDefVal::UInt("GROUP_THREADS".into(), super::pass::GROUP_THREADS),
            ShaderDefVal::UInt("GROUPS_PER_STEP".into(), super::pass::GROUPS_PER_STEP),
            ShaderDefVal::UInt("STREAMS".into(), STREAMS as u32),
            ShaderDefVal::UInt("SECTION_SLOTS".into(), section_slots as u32),
            ShaderDefVal::UInt("CAVE_WORDS".into(), section_slots.div_ceil(32) as u32),
            ShaderDefVal::UInt("MAX_WORKGROUPS".into(), max_workgroups),
            ShaderDefVal::Int("MODEL_BIAS_CONSTANT".into(), MODEL_DEPTH_BIAS.constant),
            ShaderDefVal::UInt(
                "MODEL_BIAS_SLOPE_BITS".into(),
                MODEL_DEPTH_BIAS.slope_scale.to_bits(),
            ),
        ];
        if subgroups {
            shader_defs.push("SUBGROUPS".into());
        }
        let compute = |label: &str, layout: &Vec<BindGroupLayoutDescriptor>, entry: &str| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(label.to_owned().into()),
                layout: layout.clone(),
                shader: shaders.cull.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some(entry.to_owned().into()),
                ..default()
            })
        };
        Self {
            cull_sections: compute("terrain cull sections", &quad_layout, "cull_sections"),
            cull_groups: compute("terrain cull groups", &quad_layout, "cull_groups"),
            cull_quads: compute("terrain cull quads", &quad_layout, "cull_quads"),
            cull_count: compute("terrain cull count", &layout, "count_ordered"),
            cull_scan: compute("terrain cull scan", &layout, "scan_ordered"),
            cull_scatter: compute("terrain cull scatter", &layout, "scatter_ordered"),
            cull_groups_second: compute(
                "terrain cull groups second",
                &quad_layout,
                "cull_groups_second",
            ),
            cull_quads_second: compute(
                "terrain cull quads second",
                &quad_layout,
                "cull_quads_second",
            ),
            shaders,
            terrain: None,
        }
    }

    pub fn ready(&self) -> bool {
        self.terrain.is_some()
    }

    pub fn queue_render(
        &mut self,
        binds: &Bindings,
        view: &ExtractedView,
        pipeline_cache: &PipelineCache,
    ) {
        let mut terrain = [CachedRenderPipelineId::INVALID; TERRAIN_PIPELINES];
        for layer in Pass::ALL {
            for shape in Shape::ALL {
                for wireframe in [false, true] {
                    terrain[terrain_slot(layer, shape, wireframe)] = pipeline_cache
                        .queue_render_pipeline(terrain_descriptor(
                            binds,
                            &self.shaders,
                            layer,
                            shape,
                            wireframe,
                            view,
                        ));
                }
            }
        }
        self.terrain = Some(terrain);
    }

    pub fn terrain<'cache>(
        &self,
        stream: u32,
        wireframe: bool,
        pipeline_cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        pipeline_cache.get_render_pipeline(self.terrain?[stream_slot(stream, wireframe)])
    }
}

pub(crate) fn terrain_descriptor(
    binds: &Bindings,
    shaders: &Shaders,
    layer: Pass,
    shape: Shape,
    wireframe: bool,
    view: &ExtractedView,
) -> RenderPipelineDescriptor {
    let mut descriptor = RenderPipelineDescriptor {
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            front_face: FrontFace::Ccw,
            cull_mode: layer.writes_depth().then_some(Face::Back),
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: CORE_3D_DEPTH_FORMAT,
            depth_write_enabled: Some(layer.writes_depth()),
            depth_compare: Some(super::DEPTH_COMPARE),
            stencil: default(),
            bias: model_depth_bias(layer, shape),
        }),
        ..common(
            format!(
                "terrain {} {}{}",
                layer.label(),
                shape.label(),
                if wireframe { " wireframe" } else { "" }
            ),
            vec![binds.view_layout.clone(), binds.draw_layout.clone()],
            shaders.shape(shape),
            format!("vertex_{}", shape.label()),
            format!("fragment_{}_{}", shape.label(), layer.label()),
            view,
            blend(layer),
        )
    };
    if wireframe {
        let fragment = descriptor
            .fragment
            .as_mut()
            .expect("common sets a fragment");
        fragment.shader_defs.push("WIREFRAME".into());
    }
    descriptor
}

// Model quads sit flush against the greedy faces behind them, so without a nudge the two
// fight for the same depth. The cull allows for the nudge when it tests a model quad.
const MODEL_DEPTH_BIAS: DepthBiasState = DepthBiasState {
    constant: 2,
    slope_scale: 1.0,
    clamp: 0.0,
};

fn model_depth_bias(layer: Pass, shape: Shape) -> DepthBiasState {
    if shape == Shape::Model && layer.writes_depth() {
        MODEL_DEPTH_BIAS
    } else {
        default()
    }
}

pub(super) fn prepare_pipelines(
    mut terrain: Option<ResMut<Terrain>>,
    views: Query<&ExtractedView, With<Camera3d>>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(terrain) = terrain.as_mut() else {
        return;
    };
    if terrain.pipelines.ready() {
        return;
    }
    let Some(view) = views.iter().next() else {
        return;
    };
    let terrain = terrain.as_mut();
    terrain
        .pipelines
        .queue_render(&terrain.binds, view, &pipeline_cache);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_one_pipeline_per_layer_shape_and_wireframe() {
        let mut slots: Vec<usize> = Pass::ALL
            .iter()
            .flat_map(|&layer| {
                Shape::ALL.iter().flat_map(move |&shape| {
                    [false, true].map(|wireframe| terrain_slot(layer, shape, wireframe))
                })
            })
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
    }
}
