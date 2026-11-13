use std::collections::HashSet;

use bytemuck::{Pod, Zeroable};
use mcrs_minecraft_entity_model::{
    BONE_NEVER_RENDER, BoneRecord, CUBOID_EXPLICIT, CUBOID_MIRROR, CuboidRecord, ModelRecord,
};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, BufferUsages,
    CommandEncoder, Device, Queue, RenderPipeline, Texture, TextureView,
};

use crate::entity_library::{EntityLibrary, VariantRecord};
use crate::gpu_pipelines::{BAKE_DEPTH_FORMAT, IMPOSTOR_FORMAT, impostor_constants};
use crate::impostor_frame::{
    IMPOSTOR_ELEVATIONS, IMPOSTOR_TILE, IMPOSTOR_VIEWS, IMPOSTOR_YAWS, ImpostorFrames, ImpostorView,
};
use crate::records::{HIDEABLE_ENTRIES, STATE_HIDDEN_SHIFT, StateRecord, f16_up};

/// The value of a pair with no image set.
const NO_IMPOSTOR: u32 = u32::MAX;
const FIRST_LAYERS: u32 = 8;
const COVER: &str = include_str!("shaders/impostor_cover.wgsl");
/// The most image sets: a layer of the image arrays holds one set.
const MAX_LAYERS: u32 = 2048;
const FACE_EAST: u32 = 0;
const FACE_WEST: u32 = 2;
const FACE_SOUTH: u32 = 3;
const FACE_DOWN: u32 = 5;
const VIEWS: u32 = IMPOSTOR_VIEWS as u32;

/// The instance record of the draw shader, as the bake fills it: the projection of a view.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct BakeInstance {
    rows: [[f32; 4]; 3],
    overlay: [f32; 4],
    light: [f32; 4],
}

/// The buffers and the pipeline that a bake shares with the draw of the full rigs.
pub(crate) struct BakeResources<'a> {
    pub(crate) pipeline: &'a RenderPipeline,
    pub(crate) draw_layout: &'a BindGroupLayout,
    pub(crate) images_layout: &'a BindGroupLayout,
    pub(crate) uniform: &'a Buffer,
    pub(crate) models: &'a Buffer,
    pub(crate) cuboids: &'a Buffer,
    pub(crate) variants: &'a Buffer,
    /// The bind group of each texture array.
    pub(crate) arrays: &'a [BindGroup],
}

/// A model and a texture of a layer, with the bones that the render controllers hide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Pair {
    model: u16,
    variant: u16,
    hidden: u16,
}

/// The impostor image sets: for each pair of a model and a texture that a layer of a slot
/// shows, the views of the rest pose that the bake renders with the draw of the full rigs the
/// first time a state record names the pair. The image set of a pair is one layer of a colour
/// array and of a normal array.
pub(crate) struct ImpostorAtlas {
    models: Vec<ModelRecord>,
    bones: Vec<BoneRecord>,
    cuboids: Vec<u8>,
    variants: Vec<VariantRecord>,
    /// The views of each model.
    frames: Vec<ImpostorFrames>,
    colours: Texture,
    normals: Texture,
    depth: TextureView,
    layers: u32,
    used: u32,
    max_layers: u32,
    /// For each model and variant, the layer of its set and its hidden mask, or [`NO_IMPOSTOR`].
    pub(crate) pairs: Buffer,
    rest_bones: Buffer,
    seen: HashSet<(u16, u16)>,
    noted: Vec<Pair>,
    pub(crate) images: BindGroup,
    /// The [`ImpostorView`] of each model and view, then for each layer and view the centroid of
    /// the covered texels in the image and their share. One buffer holds both, so that the
    /// prepare pass fits the storage buffers of a WebGPU stage.
    pub(crate) covers: Buffer,
    cover: wgpu::ComputePipeline,
}

impl ImpostorAtlas {
    pub(crate) fn new(device: &Device, library: &EntityLibrary, layout: &BindGroupLayout) -> Self {
        let frames = library.impostor_frames.clone();
        let storage = |label: &str, contents: &[u8], usage: BufferUsages| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: if contents.is_empty() {
                    &[0; 16]
                } else {
                    contents
                },
                usage: BufferUsages::STORAGE | usage,
            })
        };
        let pair_count = library.models.len().max(1) * library.variants.len().max(1);
        let pairs = storage(
            "entity impostor pairs",
            bytemuck::cast_slice(&vec![NO_IMPOSTOR; pair_count]),
            BufferUsages::COPY_DST,
        );
        let views: Vec<ImpostorView> = frames.iter().flat_map(|frames| frames.views).collect();
        let first_cover = views.len();
        let rest_bones = storage(
            "entity rest bones",
            bytemuck::cast_slice(&library.rest_bones),
            BufferUsages::empty(),
        );
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("entity impostor bake depth"),
                size: image_extent(1),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: BAKE_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let max_layers = device.limits().max_texture_array_layers.min(MAX_LAYERS);
        let layers = FIRST_LAYERS.min(max_layers);
        let colours = image_array(device, "entity impostor colours", layers);
        let normals = image_array(device, "entity impostor normals", layers);
        let images = images_bind(device, layout, &colours, &normals);
        Self {
            models: library.models.clone(),
            bones: library.bones.clone(),
            cuboids: library.cuboids.clone(),
            variants: library.variants.clone(),
            frames,
            colours,
            normals,
            depth,
            layers,
            used: 0,
            max_layers,
            pairs,
            rest_bones,
            seen: HashSet::new(),
            noted: Vec::new(),
            images,
            covers: storage(
                "entity impostor covers",
                &[
                    bytemuck::cast_slice(&views),
                    vec![0; max_layers as usize * VIEWS as usize * 16].as_slice(),
                ]
                .concat(),
                BufferUsages::empty(),
            ),
            cover: device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("entity impostor cover"),
                layout: None,
                module: &device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("entity impostor cover"),
                    source: wgpu::ShaderSource::Wgsl(
                        impostor_constants(COVER)
                            .replace("#{FIRST_COVER}", &first_cover.to_string())
                            .into(),
                    ),
                }),
                entry_point: Some("cover"),
                compilation_options: Default::default(),
                cache: None,
            }),
        }
    }

    /// Measures the coverage of each view image of the layer `layer`.
    fn encode_cover(&self, device: &Device, encoder: &mut CommandEncoder, layer: u32) {
        let layer_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("entity impostor cover layer"),
            contents: bytemuck::bytes_of(&[layer, 0, 0, 0]),
            usage: BufferUsages::UNIFORM,
        });
        let colours = self.colours.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("entity impostor cover"),
            layout: &self.cover.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&colours),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: self.covers.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: layer_uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("entity impostor cover"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.cover);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(VIEWS, 1, 1);
    }

    /// Notes the pairs of the active layers of `state` that have no image set yet.
    pub(crate) fn note(&mut self, state: &StateRecord) {
        let hidden = (state.flags >> STATE_HIDDEN_SHIFT) as u16;
        for (layer, [model, variant]) in state.layers.iter().enumerate() {
            if state.layer_mask & (1 << layer) != 0 && self.seen.insert((*model, *variant)) {
                self.noted.push(Pair {
                    model: *model,
                    variant: *variant,
                    hidden,
                });
            }
        }
    }

    /// Renders the image set of each noted pair. A pair that does not fit keeps no set, so its
    /// slots draw a full rig.
    pub(crate) fn bake_noted(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        resources: &BakeResources<'_>,
    ) {
        for pair in std::mem::take(&mut self.noted) {
            self.bake(device, queue, encoder, resources, pair);
        }
    }

    fn bake(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        resources: &BakeResources<'_>,
        pair: Pair,
    ) {
        let (Some(model), Some(variant)) = (
            self.models.get(usize::from(pair.model)).copied(),
            self.variants.get(usize::from(pair.variant)),
        ) else {
            return;
        };
        let Some(textures) = resources.arrays.get((variant.array_layer >> 16) as usize) else {
            return;
        };
        if self.frames.get(usize::from(pair.model)).is_none() {
            return;
        }
        if self.used == self.layers && !self.grow(device, encoder, resources.images_layout) {
            return;
        }
        let layer = self.used;
        self.used += 1;

        let faces = self.bake_faces(&model, pair.hidden);
        let bind = self.bake_bind(device, resources, pair, &model, &faces);
        let layer_view = |texture: &Texture| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let colour_view = layer_view(&self.colours);
        let normal_view = layer_view(&self.normals);
        let attachment = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("entity impostor bake"),
            color_attachments: &[attachment(&colour_view), attachment(&normal_view)],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(resources.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_bind_group(1, textures, &[]);
        let per_view = faces.len() as u32 * 6;
        let tile = IMPOSTOR_TILE as f32;
        for view in 0..VIEWS {
            let (column, row) = (view % IMPOSTOR_YAWS, view / IMPOSTOR_YAWS);
            pass.set_viewport(
                column as f32 * tile,
                row as f32 * tile,
                tile,
                tile,
                0.0,
                1.0,
            );
            pass.draw(view * per_view..(view + 1) * per_view, 0..1);
        }
        drop(pass);
        self.encode_cover(device, encoder, layer);
        let at =
            u64::from(pair.model) * self.variants.len().max(1) as u64 + u64::from(pair.variant);
        queue.write_buffer(
            &self.pairs,
            at * 4,
            bytemuck::bytes_of(&(layer | (u32::from(pair.hidden) << 16))),
        );
    }

    /// The bind group of the draw of the full rigs for the bake of `pair`: one visible entry and
    /// one projection for each view, and the face records `faces` for each view.
    fn bake_bind(
        &self,
        device: &Device,
        resources: &BakeResources<'_>,
        pair: Pair,
        model: &ModelRecord,
        faces: &[(u32, u32)],
    ) -> BindGroup {
        let frames = self
            .frames
            .get(usize::from(pair.model))
            .copied()
            .unwrap_or_default();
        let instances: Vec<BakeInstance> = (0..IMPOSTOR_VIEWS)
            .map(|view| BakeInstance {
                rows: frames.bake_rows(view),
                ..BakeInstance::default()
            })
            .collect();
        let mut records: Vec<[u32; 2]> = Vec::with_capacity(faces.len() * VIEWS as usize);
        for view in 0..VIEWS {
            records.extend(
                faces
                    .iter()
                    .map(|&(cuboid, face)| [view, cuboid | (face << 24)]),
            );
        }
        let state = StateRecord {
            definition: 0,
            layer_mask: 1,
            layers: [[pair.model, pair.variant], [0; 2], [0; 2], [0; 2]],
            flags: u32::from(pair.hidden) << STATE_HIDDEN_SHIFT,
            scale: f16_up(1.0),
            ..StateRecord::default()
        };
        let visible = vec![[0, model.first_bone]; VIEWS as usize];
        let buffer = |label: &str, contents: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: if contents.is_empty() {
                    &[0; 16]
                } else {
                    contents
                },
                usage: BufferUsages::STORAGE,
            })
        };
        let states = buffer("entity bake state", bytemuck::bytes_of(&state));
        let visible = buffer("entity bake views", bytemuck::cast_slice(&visible));
        let instances = buffer("entity bake projections", bytemuck::cast_slice(&instances));
        let face_records = buffer("entity bake faces", bytemuck::cast_slice(&records));
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("entity bake"),
            layout: resources.draw_layout,
            entries: &[
                (0, resources.uniform),
                (5, &states),
                (8, resources.models),
                (10, resources.cuboids),
                (11, &visible),
                (12, &self.rest_bones),
                (13, &instances),
                (14, &face_records),
                (19, resources.variants),
            ]
            .map(|(binding, buffer)| BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }),
        })
    }

    /// The faces of the cuboids of `model` that the bake draws, in the order of the expand
    /// pass: with the depth test, the faces that a camera does not see stay hidden.
    fn bake_faces(&self, model: &ModelRecord, hidden: u16) -> Vec<(u32, u32)> {
        let mut faces = Vec::new();
        for index in 0..model.cuboid_count {
            let at = (model.first_cuboid + index) as usize * 32;
            let Some(bytes) = self.cuboids.get(at..at + 32) else {
                continue;
            };
            let cuboid: CuboidRecord = bytemuck::pod_read_unaligned(bytes);
            let Some(bone) = self
                .bones
                .get(model.first_bone as usize + usize::from(cuboid.bone))
            else {
                continue;
            };
            let hidden_bone =
                usize::from(bone.palette) < HIDEABLE_ENTRIES && (hidden >> bone.palette) & 1 != 0;
            if bone.flags & BONE_NEVER_RENDER != 0 || hidden_bone {
                continue;
            }
            let mask = u32::from(cuboid.flags) & 0x3f;
            let in_mask = |face: u32| mask & (1 << face) != 0;
            let zero = cuboid.size.map(|size| size == 0.0);
            let selected = match zero {
                _ if cuboid.flags & CUBOID_EXPLICIT != 0 => None,
                [true, false, false] if cuboid.flags & CUBOID_MIRROR != 0 => Some(FACE_EAST),
                [true, false, false] => Some(FACE_WEST),
                [false, true, false] => Some(FACE_DOWN),
                [false, false, true] => Some(FACE_SOUTH),
                [false, false, false] => None,
                _ => continue,
            };
            match selected {
                Some(face) => {
                    let other = opposite(face);
                    faces.extend(
                        [other, face]
                            .into_iter()
                            .filter(|face| in_mask(*face))
                            .map(|face| (index, face)),
                    );
                }
                None => faces.extend(
                    (0..6)
                        .filter(|face| in_mask(*face))
                        .map(|face| (index, face)),
                ),
            }
        }
        faces
    }

    /// Doubles the layers of the image arrays. `false` when they are at the limit.
    fn grow(
        &mut self,
        device: &Device,
        encoder: &mut CommandEncoder,
        layout: &BindGroupLayout,
    ) -> bool {
        if self.layers >= self.max_layers {
            return false;
        }
        let layers = (self.layers * 2).min(self.max_layers);
        let colours = image_array(device, "entity impostor colours", layers);
        let normals = image_array(device, "entity impostor normals", layers);
        for (from, to) in [(&self.colours, &colours), (&self.normals, &normals)] {
            encoder.copy_texture_to_texture(
                from.as_image_copy(),
                to.as_image_copy(),
                image_extent(self.layers),
            );
        }
        self.images = images_bind(device, layout, &colours, &normals);
        self.colours = colours;
        self.normals = normals;
        self.layers = layers;
        true
    }

    /// The image sets that the atlas holds.
    pub(crate) fn image_sets(&self) -> u32 {
        self.used
    }
}

fn opposite(face: u32) -> u32 {
    match face {
        FACE_EAST => FACE_WEST,
        1 => FACE_SOUTH,
        FACE_WEST => FACE_EAST,
        FACE_SOUTH => 1,
        4 => FACE_DOWN,
        _ => 4,
    }
}

fn image_extent(layers: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: IMPOSTOR_TILE * IMPOSTOR_YAWS,
        height: IMPOSTOR_TILE * IMPOSTOR_ELEVATIONS.len() as u32,
        depth_or_array_layers: layers,
    }
}

fn image_array(device: &Device, label: &str, layers: u32) -> Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: image_extent(layers),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: IMPOSTOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn images_bind(
    device: &Device,
    layout: &BindGroupLayout,
    colours: &Texture,
    normals: &Texture,
) -> BindGroup {
    let view = |texture: &Texture| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
    };
    let (colours, normals) = (view(colours), view(normals));
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("entity impostor images"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&colours),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&normals),
            },
        ],
    })
}
