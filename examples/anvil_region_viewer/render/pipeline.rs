use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;

use super::binds::Bindings;
use super::layer::{Layer, Shape};
use super::shaders::Shaders;
use super::sky::{self, SKY_DRAWS};
use super::terrain::Terrain;

pub(super) const TERRAIN_PIPELINES: usize = Layer::ALL.len() * Shape::ALL.len();

const fn slot(layer: Layer, shape: Shape) -> usize {
    layer as usize * Shape::ALL.len() + shape as usize
}

pub(super) fn common(
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
    pub cull_stable: CachedComputePipelineId,
    terrain: Option<[CachedRenderPipelineId; TERRAIN_PIPELINES]>,
    sky: Option<[CachedRenderPipelineId; SKY_DRAWS.len()]>,
}

impl Pipelines {
    pub fn new(shaders: Shaders, binds: &Bindings, pipeline_cache: &PipelineCache) -> Self {
        let layout = vec![binds.view_layout.clone(), binds.cull_layout.clone()];
        let cull = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("terrain cull".into()),
            layout: layout.clone(),
            shader: shaders.cull.clone(),
            entry_point: Some("cull".into()),
            ..default()
        });
        let cull_stable = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("terrain cull stable".into()),
            layout,
            shader: shaders.cull.clone(),
            entry_point: Some("cull_stable".into()),
            ..default()
        });
        Self {
            shaders,
            cull,
            cull_stable,
            terrain: None,
            sky: None,
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
        for layer in Layer::ALL {
            for shape in Shape::ALL {
                terrain[slot(layer, shape)] =
                    self.queue_terrain(layer, shape, binds, view, pipeline_cache);
            }
        }
        self.terrain = Some(terrain);
        self.sky = Some(sky::pipelines(&self.shaders, binds, view, pipeline_cache));
    }

    fn queue_terrain(
        &self,
        layer: Layer,
        shape: Shape,
        binds: &Bindings,
        view: &ExtractedView,
        pipeline_cache: &PipelineCache,
    ) -> CachedRenderPipelineId {
        let descriptor = RenderPipelineDescriptor {
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                front_face: FrontFace::Ccw,
                cull_mode: layer.writes_depth().then_some(Face::Back),
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(layer.writes_depth()),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: model_depth_bias(layer, shape),
            }),
            ..common(
                format!("terrain {} {}", layer.label(), shape.label()),
                vec![binds.view_layout.clone(), binds.draw_layout.clone()],
                self.shaders.shape(shape),
                format!("vertex_{}", shape.label()),
                format!("fragment_{}_{}", shape.label(), layer.label()),
                view,
                layer.blend(),
            )
        };
        pipeline_cache.queue_render_pipeline(descriptor)
    }

    pub fn terrain<'cache>(
        &self,
        stream: u32,
        wireframe: bool,
        pipeline_cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        let layer = Layer::of_stream(stream).drawn_as(wireframe);
        let slot = slot(layer, Shape::of_stream(stream));
        pipeline_cache.get_render_pipeline(self.terrain?[slot])
    }

    pub fn sky<'cache>(
        &self,
        index: usize,
        pipeline_cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        pipeline_cache.get_render_pipeline(self.sky?[index])
    }
}

// Model quads sit flush against the greedy faces behind them, so without a nudge the two
// fight for the same depth.
fn model_depth_bias(layer: Layer, shape: Shape) -> DepthBiasState {
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
    fn the_table_holds_one_pipeline_per_layer_and_shape() {
        let mut slots: Vec<usize> = Layer::ALL
            .iter()
            .flat_map(|&layer| Shape::ALL.iter().map(move |&shape| slot(layer, shape)))
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
    }
}
