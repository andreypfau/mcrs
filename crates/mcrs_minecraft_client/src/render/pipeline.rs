use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use bevy::shader::ShaderDefVal;

use crate::blocks::Pass;
use crate::mesh::{STREAMS, stream_pass};

use super::binds::Bindings;
use super::layer::Shape;
use super::shaders::Shaders;
use super::terrain::Terrain;

pub(super) const TERRAIN_PIPELINES: usize = Pass::COUNT * Shape::ALL.len() * 2;

const fn slot(layer: Pass, shape: Shape, wireframe: bool) -> usize {
    (layer as usize * Shape::ALL.len() + shape as usize) * 2 + wireframe as usize
}

pub(crate) fn common(
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
    pub cull: CachedComputePipelineId,
    pub cull_count: CachedComputePipelineId,
    pub cull_scan: CachedComputePipelineId,
    pub cull_scatter: CachedComputePipelineId,
    pub cull_second: CachedComputePipelineId,
    terrain: Option<[CachedRenderPipelineId; TERRAIN_PIPELINES]>,
}

impl Pipelines {
    pub fn new(shaders: Shaders, binds: &Bindings, pipeline_cache: &PipelineCache) -> Self {
        let layout = vec![binds.view_layout.clone(), binds.cull_layout.clone()];
        let shader_defs = vec![
            ShaderDefVal::UInt("CULL_THREADS".into(), super::pass::CULL_THREADS),
            ShaderDefVal::UInt("STREAMS".into(), STREAMS as u32),
        ];
        let cull = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("terrain cull".into()),
            layout: layout.clone(),
            shader: shaders.cull.clone(),
            shader_defs: shader_defs.clone(),
            entry_point: Some("cull".into()),
            ..default()
        });
        let ordered = |label: &str, entry: &str| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(label.to_owned().into()),
                layout: layout.clone(),
                shader: shaders.cull.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some(entry.to_owned().into()),
                ..default()
            })
        };
        let cull_count = ordered("terrain cull count", "count_ordered");
        let cull_scan = ordered("terrain cull scan", "scan_ordered");
        let cull_scatter = ordered("terrain cull scatter", "scatter_ordered");
        let cull_second = ordered("terrain cull second", "cull_second");
        Self {
            shaders,
            cull,
            cull_count,
            cull_scan,
            cull_scatter,
            cull_second,
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
                    terrain[slot(layer, shape, wireframe)] =
                        self.queue_terrain(layer, shape, wireframe, binds, view, pipeline_cache);
                }
            }
        }
        self.terrain = Some(terrain);
    }

    fn queue_terrain(
        &self,
        layer: Pass,
        shape: Shape,
        wireframe: bool,
        binds: &Bindings,
        view: &ExtractedView,
        pipeline_cache: &PipelineCache,
    ) -> CachedRenderPipelineId {
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
                self.shaders.shape(shape),
                format!("vertex_{}", shape.label()),
                format!("fragment_{}_{}", shape.label(), layer.label()),
                view,
                layer.blend(),
            )
        };
        if wireframe {
            let fragment = descriptor
                .fragment
                .as_mut()
                .expect("common sets a fragment");
            fragment.shader_defs.push("WIREFRAME".into());
        }
        pipeline_cache.queue_render_pipeline(descriptor)
    }

    pub fn terrain<'cache>(
        &self,
        stream: u32,
        wireframe: bool,
        pipeline_cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        let slot = slot(stream_pass(stream), Shape::of_stream(stream), wireframe);
        pipeline_cache.get_render_pipeline(self.terrain?[slot])
    }
}

// Model quads sit flush against the greedy faces behind them, so without a nudge the two
// fight for the same depth.
fn model_depth_bias(layer: Pass, shape: Shape) -> DepthBiasState {
    if shape == Shape::Model && layer.writes_depth() {
        DepthBiasState {
            constant: 2,
            slope_scale: 1.0,
            clamp: 0.0,
        }
    } else {
        default()
    }
}

pub(super) fn prepare_pipelines(
    mut terrain: Option<ResMut<Terrain>>,
    views: Query<&ExtractedView>,
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
                    [false, true].map(|wireframe| slot(layer, shape, wireframe))
                })
            })
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
    }
}
