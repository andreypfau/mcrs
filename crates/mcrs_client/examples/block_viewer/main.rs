//! Renders a single vanilla Minecraft blockstate from the bundled client assets, shaded the way
//! the vanilla client shades it: baked block models, directional face shade, and the real smooth
//! lighting (ambient occlusion) code path.
//!
//! Pass the blockstate to render as the first argument, in the same form the game writes it:
//!
//! ```text
//! cargo run --example block_viewer -- "minecraft:oak_log[axis=x]"
//! cargo run --example block_viewer -- glass
//! ```
//!
//! Drag with the left mouse button to orbit, scroll to zoom, press `F` to drop the block onto a
//! floor of neighbours — ambient occlusion is invisible on a block floating in open air, because
//! every neighbour sample is then unoccluded.

mod bake;
mod model;

use bevy::asset::RenderAssetUsages;
use bevy::color::LinearRgba;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use bake::{Neighborhood, TinyWorld};

const DEFAULT_TARGET: &str = "minecraft:oak_log[axis=y]";

fn main() {
    let target = Target::from_args();
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: target.label(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.13, 0.14, 0.17)))
        .insert_resource(target)
        .init_resource::<ShowFloor>()
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, (toggle_floor, rebuild_scene, orbit))
        .run();
}

/// The blockstate to render, written the way the game writes it: `name[prop=value,...]`.
#[derive(Resource)]
struct Target {
    block: String,
    props: Vec<(String, String)>,
}

impl Target {
    fn from_args() -> Self {
        Self::parse(
            &std::env::args()
                .nth(1)
                .unwrap_or_else(|| DEFAULT_TARGET.to_string()),
        )
    }

    fn parse(arg: &str) -> Self {
        let (block, props) = match arg.split_once('[') {
            Some((block, props)) => (block, props.trim_end().trim_end_matches(']')),
            None => (arg, ""),
        };
        let props = props
            .split(',')
            .filter(|entry| !entry.trim().is_empty())
            .filter_map(|entry| entry.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        Self {
            block: block.trim().to_string(),
            props,
        }
    }

    fn pairs(&self) -> Vec<(&str, &str)> {
        self.props
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    fn label(&self) -> String {
        if self.props.is_empty() {
            return self.block.clone();
        }
        let props: Vec<String> = self.props.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{}[{}]", self.block, props.join(","))
    }
}

#[derive(Resource, Default)]
struct ShowFloor(bool);

#[derive(Component)]
struct BlockMesh;

#[derive(Component)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    radius: f32,
}

fn spawn_camera(mut commands: Commands) {
    let orbit = Orbit {
        yaw: 0.7,
        pitch: 0.5,
        radius: 3.5,
    };
    commands.spawn((
        Camera3d::default(),
        // Bevy tonemaps by default, which filmic-shifts the vanilla shade values.
        Tonemapping::None,
        Transform::from_xyz(2.5, 2.0, 2.5).looking_at(Vec3::splat(0.5), Vec3::Y),
        orbit,
    ));
}

fn toggle_floor(keys: Res<ButtonInput<KeyCode>>, mut show: ResMut<ShowFloor>) {
    if keys.just_pressed(KeyCode::KeyF) {
        show.0 = !show.0;
    }
}

fn rebuild_scene(
    mut commands: Commands,
    target: Res<Target>,
    show: Res<ShowFloor>,
    existing: Query<Entity, With<BlockMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !show.is_changed() {
        return;
    }
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let blocks = scene_blocks(show.0);
    let (mesh, sprites) = match build_mesh(&blocks, &target) {
        Ok(built) => built,
        Err(error) => {
            error!("cannot bake {}: {error}", target.label());
            return;
        }
    };
    let atlas = match build_atlas(&sprites) {
        Ok(atlas) => atlas,
        Err(error) => {
            error!("cannot build the texture atlas: {error}");
            return;
        }
    };

    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(images.add(atlas)),
        unlit: true,
        alpha_mode: AlphaMode::Opaque,
        ..default()
    });
    commands.spawn((
        BlockMesh,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
    ));
}

fn scene_blocks(show_floor: bool) -> Vec<IVec3> {
    let mut blocks = vec![IVec3::ZERO];
    if show_floor {
        for x in -2..=2 {
            for z in -2..=2 {
                blocks.push(IVec3::new(x, -1, z));
            }
        }
    }
    blocks
}

fn build_mesh(blocks: &[IVec3], target: &Target) -> Result<(Mesh, Vec<String>), String> {
    let props = target.pairs();
    let world = TinyWorld::new(blocks.iter().copied());
    let mut sprites: Vec<String> = Vec::new();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut baked_quads = Vec::new();

    for &pos in blocks {
        let baked = bake::bake(&target.block, &props, pos, &world)?;
        for quad in baked.quads {
            // The only piece of vanilla's `shouldRenderFace` a lone block needs: a face touching a
            // full neighbour is never visible.
            if let Some(cull) = quad.cull
                && world.is_collision_shape_full_block(pos + cull.inormal())
            {
                continue;
            }
            let sprite = &baked.sprites[quad.sprite];
            let slot = match sprites.iter().position(|s| s == sprite) {
                Some(slot) => slot,
                None => {
                    sprites.push(sprite.clone());
                    sprites.len() - 1
                }
            };
            baked_quads.push((pos, quad, slot));
        }
    }

    let slots = sprites.len().max(1) as f32;
    for (pos, quad, slot) in baked_quads {
        let base = positions.len() as u32;
        let normal = quad.dir.normal().to_array();
        for i in 0..4 {
            positions.push((quad.positions[i] + pos.as_vec3()).to_array());
            normals.push(normal);
            uvs.push([quad.uvs[i][0], (slot as f32 + quad.uvs[i][1]) / slots]);
            // Vanilla's shade byte is sRGB; Bevy consumes vertex colours as linear.
            let c = quad.color[i];
            colors.push(LinearRgba::from(Color::srgb_u8(c, c, c)).to_f32_array());
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_COLOR,
        VertexAttributeValues::Float32x4(colors),
    )
    .with_inserted_indices(Indices::U32(indices));
    Ok((mesh, sprites))
}

/// One vertical strip of sprites, so the whole block is a single mesh with a single material —
/// which is what a chunk section will need.
fn build_atlas(sprites: &[String]) -> Result<Image, String> {
    let mut cells: Vec<(usize, Vec<u8>)> = Vec::new();
    for id in sprites {
        let path = model::resource_path(id, "textures", "png");
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let image = Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::default(),
        )
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;
        let width = image.width() as usize;
        let data = image
            .data
            .ok_or_else(|| format!("{} decoded without pixel data", path.display()))?;
        // An animated sprite is a vertical strip of square frames; take the first one.
        let frame = width * width * 4;
        if data.len() < frame {
            return Err(format!(
                "{} is smaller than one square frame",
                path.display()
            ));
        }
        cells.push((width, data[..frame].to_vec()));
    }

    let size = cells.first().map(|(w, _)| *w).unwrap_or(16);
    if let Some((other, _)) = cells.iter().find(|(w, _)| *w != size) {
        return Err(format!(
            "mixed sprite sizes ({size} and {other}); the atlas assumes one size"
        ));
    }

    let mut data = Vec::with_capacity(size * size * 4 * cells.len().max(1));
    for (_, cell) in &cells {
        data.extend_from_slice(cell);
    }
    if cells.is_empty() {
        data.resize(size * size * 4, 255);
    }

    let mut atlas = Image::new(
        Extent3d {
            width: size as u32,
            height: (size * cells.len().max(1)) as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    atlas.sampler = ImageSampler::nearest();
    Ok(atlas)
}

fn orbit(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    camera: Single<(&mut Orbit, &mut Transform)>,
) {
    let (mut orbit, mut transform) = camera.into_inner();
    if buttons.pressed(MouseButton::Left) {
        orbit.yaw -= motion.delta.x * 0.005;
        orbit.pitch = (orbit.pitch - motion.delta.y * 0.005).clamp(-1.54, 1.54);
    }
    let zoom = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    };
    orbit.radius = (orbit.radius - zoom * 0.4).clamp(1.2, 20.0);

    let target = Vec3::splat(0.5);
    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
    *transform = Transform::from_translation(target + rotation * (Vec3::Z * orbit.radius))
        .looking_at(target, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::Target;

    #[test]
    fn target_parses_the_game_s_own_blockstate_syntax() {
        let bare = Target::parse("glass");
        assert_eq!(bare.block, "glass");
        assert!(bare.props.is_empty());
        assert_eq!(bare.label(), "glass");

        let full = Target::parse("minecraft:oak_log[axis=x]");
        assert_eq!(full.block, "minecraft:oak_log");
        assert_eq!(full.pairs(), [("axis", "x")]);

        let messy = Target::parse(" minecraft:furnace [ facing = north , lit = true ] ");
        assert_eq!(messy.block, "minecraft:furnace");
        assert_eq!(messy.pairs(), [("facing", "north"), ("lit", "true")]);
        assert_eq!(messy.label(), "minecraft:furnace[facing=north,lit=true]");
    }
}
