use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureViewDescriptor, TextureViewDimension,
};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_render::{CUBE_FACES, PanoramaImages, PanoramaPlugin};

use crate::stream::BlockCatalog;

const FACES: [&str; 6] = [
    "minecraft/textures/gui/title/background/panorama_0.png",
    "minecraft/textures/gui/title/background/panorama_1.png",
    "minecraft/textures/gui/title/background/panorama_2.png",
    "minecraft/textures/gui/title/background/panorama_3.png",
    "minecraft/textures/gui/title/background/panorama_4.png",
    "minecraft/textures/gui/title/background/panorama_5.png",
];
const OVERLAY: &str = "minecraft/textures/gui/title/background/panorama_overlay.png";

pub struct TitlePanoramaPlugin;

impl Plugin for TitlePanoramaPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PanoramaPlugin)
            .init_resource::<PanoramaLoad>()
            .add_systems(
                Update,
                load_panorama.run_if(resource_exists::<PanoramaLoad>),
            );
    }
}

#[derive(Resource, Default)]
enum PanoramaLoad {
    #[default]
    Waiting,
    Decoding(Task<Result<Decoded, String>>),
}

struct Encoded {
    faces: [Vec<u8>; 6],
    overlay: Vec<u8>,
}

struct Decoded {
    cube: Image,
    overlay: Image,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "one-off load that ends when the resource is removed; it copies the pack bytes of the panorama once for the decoding task"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn load_panorama(
    mut load: ResMut<PanoramaLoad>,
    catalog: Res<BlockCatalog>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let decoded = match &mut *load {
        PanoramaLoad::Waiting => {
            let Some(pack) = catalog.pack() else {
                return;
            };
            match encoded(pack) {
                Ok(encoded) => {
                    let task = AsyncComputeTaskPool::get().spawn(async move { decode(&encoded) });
                    *load = PanoramaLoad::Decoding(task);
                    return;
                }
                Err(reason) => Err(reason),
            }
        }
        PanoramaLoad::Decoding(task) => match check_ready(task) {
            Some(decoded) => decoded,
            None => return,
        },
    };
    commands.remove_resource::<PanoramaLoad>();
    match decoded {
        Ok(Decoded { cube, overlay }) => commands.insert_resource(PanoramaImages {
            cube: images.add(cube),
            overlay: images.add(overlay),
        }),
        Err(reason) => warn!("no title panorama: {reason}"),
    }
}

fn encoded(pack: &ResourceSet) -> Result<Encoded, String> {
    let read = |path: &str| {
        pack.get(path)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| format!("{path} is not in the pack"))
    };
    let mut faces: [Vec<u8>; 6] = Default::default();
    for (face, path) in faces.iter_mut().zip(FACES) {
        *face = read(path)?;
    }
    Ok(Encoded {
        faces,
        overlay: read(OVERLAY)?,
    })
}

fn decode(encoded: &Encoded) -> Result<Decoded, String> {
    let decode = |bytes: &[u8], path: &str| {
        Image::from_buffer(
            bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::linear(),
            RenderAssetUsages::RENDER_WORLD,
        )
        .map_err(|reason| format!("{path}: {reason}"))
    };
    let mut faces: [Image; 6] = Default::default();
    for ((face, bytes), path) in faces.iter_mut().zip(&encoded.faces).zip(FACES) {
        *face = decode(bytes, path)?;
    }
    Ok(Decoded {
        cube: cube(&faces)?,
        overlay: decode(&encoded.overlay, OVERLAY)?,
    })
}

fn cube(faces: &[Image; 6]) -> Result<Image, String> {
    let [first, ..] = faces;
    let size = first.texture_descriptor.size;
    let format = first.texture_descriptor.format;
    if faces.iter().any(|face| {
        face.texture_descriptor.size != size || face.texture_descriptor.format != format
    }) {
        return Err("the panorama faces differ in size or format".to_owned());
    }
    if size.width != size.height {
        return Err(format!(
            "the panorama faces are {}x{}, not square",
            size.width, size.height
        ));
    }
    let mut data = Vec::new();
    for face in CUBE_FACES {
        let Some(bytes) = faces.get(face).and_then(|face| face.data.as_deref()) else {
            return Err(format!("panorama face {face} has no pixels"));
        };
        data.extend_from_slice(bytes);
    }
    let mut cube = Image::new(
        Extent3d {
            depth_or_array_layers: 6,
            ..size
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    cube.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    cube.sampler = ImageSampler::linear();
    Ok(cube)
}

#[cfg(test)]
mod tests {
    use bevy::render::render_resource::TextureFormat;

    use super::*;

    #[test]
    fn cube_layers_hold_the_faces_in_cube_order() {
        let faces: [Image; 6] = std::array::from_fn(|face| {
            Image::new_fill(
                Extent3d::default(),
                TextureDimension::D2,
                &[face as u8, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            )
        });

        let cube = cube(&faces).unwrap();

        let layers: Vec<u8> = cube.data.unwrap().chunks(4).map(|pixel| pixel[0]).collect();
        assert_eq!(layers, [1, 3, 4, 5, 0, 2]);
        assert_eq!(
            cube.texture_view_descriptor.unwrap().dimension,
            Some(TextureViewDimension::Cube)
        );
    }

    #[test]
    fn matching_faces_that_are_not_square_are_rejected() {
        let faces: [Image; 6] = std::array::from_fn(|_| {
            Image::new_fill(
                Extent3d {
                    width: 2,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            )
        });

        assert!(cube(&faces).is_err());
    }
}
