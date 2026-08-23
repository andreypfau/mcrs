use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};

const CELESTIAL_LAYERS: [&str; 9] = [
    "minecraft/textures/environment/celestial/sun.png",
    "minecraft/textures/environment/celestial/moon/full_moon.png",
    "minecraft/textures/environment/celestial/moon/waning_gibbous.png",
    "minecraft/textures/environment/celestial/moon/third_quarter.png",
    "minecraft/textures/environment/celestial/moon/waning_crescent.png",
    "minecraft/textures/environment/celestial/moon/new_moon.png",
    "minecraft/textures/environment/celestial/moon/waxing_crescent.png",
    "minecraft/textures/environment/celestial/moon/first_quarter.png",
    "minecraft/textures/environment/celestial/moon/waxing_gibbous.png",
];

const CLOUD_LAYERS: [&str; 1] = ["minecraft/textures/environment/clouds.png"];

#[derive(Resource)]
struct SkySources {
    celestials: Vec<Handle<Image>>,
    clouds: Vec<Handle<Image>>,
}

impl SkySources {
    fn handles(&self) -> impl Iterator<Item = &Handle<Image>> {
        self.celestials.iter().chain(&self.clouds)
    }
}

/// Exists only once both arrays are assembled.
#[derive(Resource)]
pub struct SkyTextures {
    pub celestials: Handle<Image>,
    pub clouds: Handle<Image>,
}

pub struct SkyTexturePlugin;

impl Plugin for SkyTexturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, request_sources)
            .add_systems(Update, assemble_arrays);
    }
}

fn request_sources(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |path: &&str| {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
            })
            .load((*path).to_owned())
    };
    commands.insert_resource(SkySources {
        celestials: CELESTIAL_LAYERS.iter().map(load).collect(),
        clouds: CLOUD_LAYERS.iter().map(load).collect(),
    });
}

fn assemble_arrays(
    mut events: MessageReader<AssetEvent<Image>>,
    sources: Res<SkySources>,
    mut images: ResMut<Assets<Image>>,
    assembled: Option<Res<SkyTextures>>,
    // `insert_resource` below is deferred, so `assembled` still reads `None` on
    // the frame after the first build; without this the leftover `Added` events
    // would build a second pair of arrays and orphan the first.
    mut created: Local<bool>,
    mut commands: Commands,
) {
    let mut touched = false;
    for event in events.read() {
        match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                touched |= sources.handles().any(|handle| handle.id() == *id);
            }
            _ => {}
        }
    }
    if !touched || sources.handles().any(|handle| images.get(handle).is_none()) {
        return;
    }
    if *created && assembled.is_none() {
        return;
    }

    let (celestials, clouds) = match assemble(&images, &sources) {
        Ok(arrays) => arrays,
        Err(err) => {
            error!("{err}");
            return;
        }
    };

    log_array("celestials", &celestials);
    log_array("clouds", &clouds);

    match assembled.map(|existing| (existing.celestials.clone(), existing.clouds.clone())) {
        Some(existing) => {
            for (handle, rebuilt) in [(&existing.0, celestials), (&existing.1, clouds)] {
                match images.get_mut(handle) {
                    Some(mut slot) => *slot = rebuilt,
                    None => error!("the assembled sky array vanished from Assets<Image>"),
                }
            }
        }
        None => {
            commands.insert_resource(SkyTextures {
                celestials: images.add(celestials),
                clouds: images.add(clouds),
            });
            *created = true;
        }
    }
}

fn assemble(images: &Assets<Image>, sources: &SkySources) -> Result<(Image, Image), String> {
    let mut format = None;
    let celestials = array(images, &sources.celestials, &mut format)?;
    let clouds = array(images, &sources.clouds, &mut format)?;
    let side = clouds.texture_descriptor.size.width;
    if !side.is_power_of_two() {
        return Err(format!(
            "the cloud field is {side}x{side}, and the march can only wrap a power of two"
        ));
    }
    Ok((celestials, clouds))
}

fn array(
    images: &Assets<Image>,
    handles: &[Handle<Image>],
    format: &mut Option<TextureFormat>,
) -> Result<Image, String> {
    let mut side = 0;
    let mut pixels = Vec::new();
    for handle in handles {
        let path = handle.path().map(ToString::to_string).unwrap_or_default();
        let image = images.get(handle).ok_or_else(|| format!("{path} is not loaded"))?;
        let (width, height) = (image.width(), image.height());
        if width != height {
            return Err(format!("{path} is {width}x{height}, not square"));
        }
        if side != 0 && width != side {
            return Err(format!(
                "{path} is {width}x{width} where the layers before it were {side}x{side}"
            ));
        }
        side = width;
        let layer_format = image.texture_descriptor.format;
        if *format.get_or_insert(layer_format) != layer_format {
            return Err(format!(
                "{path} is {layer_format:?} where the layers before it were {:?}",
                format.unwrap()
            ));
        }
        let data = image
            .data
            .as_deref()
            .ok_or_else(|| format!("{path} kept no pixel data in the main world"))?;
        pixels.extend_from_slice(data);
    }

    let mut array = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: handles.len() as u32,
        },
        TextureDimension::D2,
        pixels,
        format.expect("a layer list is never empty"),
        RenderAssetUsages::RENDER_WORLD,
    );
    array.sampler = ImageSampler::nearest();
    array.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    Ok(array)
}

fn log_array(name: &str, image: &Image) {
    let descriptor = &image.texture_descriptor;
    info!(
        name,
        layers = descriptor.size.depth_or_array_layers,
        width = descriptor.size.width,
        height = descriptor.size.height,
        format = ?descriptor.format,
        "assembled sky array"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stem(path: &str) -> &str {
        path.rsplit('/').next().unwrap().trim_end_matches(".png")
    }

    #[test]
    fn the_celestial_layers_are_the_sun_then_the_moon_phases_in_index_order() {
        let stems: Vec<&str> = CELESTIAL_LAYERS.iter().copied().map(stem).collect();
        assert_eq!(
            stems,
            [
                "sun",
                "full_moon",
                "waning_gibbous",
                "third_quarter",
                "waning_crescent",
                "new_moon",
                "waxing_crescent",
                "first_quarter",
                "waxing_gibbous",
            ]
        );
    }

    #[test]
    fn every_layer_path_names_a_file_in_the_corpus() {
        let corpus = crate::asset_corpus();
        for path in CELESTIAL_LAYERS.iter().chain(&CLOUD_LAYERS) {
            let file = corpus.join(path);
            assert!(file.is_file(), "{} is missing", file.display());
        }
    }
}
