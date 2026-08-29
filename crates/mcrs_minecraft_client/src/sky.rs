use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy::transform::TransformSystems;
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::registry::snapshot::rl_from_asset_path;

use crate::sky_state::{SkyField, SkyFrame, SkyKey, SkyLayout, SkyStatic, SkyValue};
use mcrs_minecraft_world::dimension::dimension_type::DimensionType;
use mcrs_minecraft_world::environment::{
    DimensionEnvironments, EnvironmentAttributes, EnvironmentContext, SpatialAttributeInterpolator,
    Weather,
};
use mcrs_minecraft_world::world_clock::WorldClocks;

use crate::player::PlayerCamera;

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

/// The dimension type whose sky is drawn, as read from the save.
#[derive(Resource)]
pub struct PlayerDimension(pub String);

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyFrame>()
            .add_plugins(crate::sky_render::SkyRenderPlugin)
            .add_systems(Startup, request_sources)
            .add_systems(Update, assemble_arrays)
            .add_systems(OnEnter(AppState::Playing), build_sky_environment)
            .add_systems(
                PostUpdate,
                evaluate_sky
                    .after(TransformSystems::Propagate)
                    .run_if(resource_exists::<SkyEnvironment>),
            );
    }
}

const OVERWORLD: &str = "minecraft:overworld";

const HORIZON: f32 = 63.0;

const CLOUD_BLOCKS_PER_TICK: f64 = 0.030000001;

/// No attribute carries it: the reference client multiplies the block-light
/// term by this after the lightmap curve.
const BLOCK_LIGHT_FACTOR: f32 = 1.4;

/// The widest cloud field the march can wrap is 4096 cells of 12 blocks, so
/// folding the drift into that span keeps the shader's own wrap intact while
/// the value stays small enough for `f32` to resolve single blocks.
const CLOUD_DRIFT_SPAN: f64 = 4096.0 * 12.0;

/// Everything the sky needs that holds for as long as the player stays in one
/// dimension. Rebuilt whole, never patched.
#[derive(Resource)]
pub struct SkyEnvironment {
    attributes: EnvironmentAttributes,
    layout: SkyLayout,
    statics: SkyStatic,
    clock: Option<ResourceLocation<Arc<str>>>,
}

impl SkyEnvironment {
    pub fn key(&self) -> SkyKey {
        self.statics.key
    }

    pub fn drift(&self, clocks: &WorldClocks) -> f32 {
        cloud_drift(
            self.clock
                .as_ref()
                .and_then(|clock| clocks.get(clock.as_str()))
                .map_or(0.0, |state| {
                    state.total_ticks as f64 + f64::from(state.partial_tick)
                }),
        )
    }

    fn value(&self, frame: &SkyFrame, field: SkyField) -> SkyValue {
        frame
            .get(&self.layout, field)
            .or_else(|| self.statics.get(field))
            .unwrap_or(SkyValue::Scalar(0.0))
    }

    fn scalar(&self, frame: &SkyFrame, field: SkyField) -> f32 {
        match self.value(frame, field) {
            SkyValue::Scalar(scalar) => scalar,
            SkyValue::Color(packed) => packed as f32,
        }
    }

    pub fn uniform(&self, frame: &SkyFrame, drift: f32) -> SkyUniform {
        let color = |field| self.value(frame, field).color();
        let sunrise = color(SkyField::SunriseSunsetColor);
        let cloud = color(SkyField::CloudColor);
        SkyUniform {
            disc: rgba(
                color(SkyField::SkyColor),
                f32::from(frame.camera[1] < HORIZON),
            ),
            sunrise: rgba(sunrise, alpha(sunrise)),
            angles: [
                self.scalar(frame, SkyField::SunAngle).to_radians(),
                self.scalar(frame, SkyField::MoonAngle).to_radians(),
                self.scalar(frame, SkyField::StarAngle).to_radians(),
                linear(self.scalar(frame, SkyField::StarBrightness)),
            ],
            // Layer 0 of the celestial array is the sun; the phases follow it.
            moon: [
                1.0 + self.scalar(frame, SkyField::MoonPhase),
                1.0 - frame.rain,
                0.0,
                0.0,
            ],
            fog: rgba(
                color(SkyField::FogColor),
                self.scalar(frame, SkyField::SkyFogEndDistance),
            ),
            cloud_color: rgba(cloud, alpha(cloud)),
            cloud: [
                self.scalar(frame, SkyField::CloudHeight),
                drift,
                self.scalar(frame, SkyField::CloudFogEndDistance),
                0.0,
            ],
            sky_light: rgba(
                color(SkyField::SkyLightColor),
                self.scalar(frame, SkyField::SkyLightFactor),
            ),
            block_light: rgba(color(SkyField::BlockLightTint), BLOCK_LIGHT_FACTOR),
            ambient: rgba(color(SkyField::AmbientLightColor), 0.0),
        }
    }

    /// The reference clears the frame to the fog colour before the sky disc
    /// goes down, which is all that shows below the horizon until terrain does.
    fn fog_color(&self, frame: &SkyFrame) -> Color {
        let [red, green, blue, _] = rgba(self.value(frame, SkyField::FogColor).color(), 0.0);
        Color::LinearRgba(LinearRgba::rgb(red, green, blue))
    }
}

fn build_sky_environment(
    mut commands: Commands,
    dimension: Res<PlayerDimension>,
    environments: Res<DimensionEnvironments>,
    dimension_types: Res<Assets<DimensionType>>,
    asset_server: Res<AssetServer>,
    clocks: Res<WorldClocks>,
    weather: Res<Weather>,
) {
    let found = environments
        .get(&dimension.0)
        .map(|attributes| (dimension.0.as_str(), attributes));
    let fallback = || {
        warn!(
            dimension = dimension.0,
            "no environment for this dimension; drawing the overworld"
        );
        environments
            .get(OVERWORLD)
            .map(|attributes| (OVERWORLD, attributes))
    };
    let Some((id, attributes)) = found.or_else(fallback) else {
        error!(dimension = dimension.0, "no environment to draw a sky from");
        return;
    };
    let attributes = attributes.clone();
    let layout = SkyLayout::derive(&attributes);

    let mut ticks = Vec::new();
    attributes.clock_ticks(&clocks, &mut ticks);
    let biomes = SpatialAttributeInterpolator::default();
    let statics = layout.constants(&attributes, &context(Vec3::ZERO, &ticks, &biomes, *weather));

    let clock = dimension_types
        .iter()
        .find(|(asset_id, _)| {
            asset_server
                .get_path(*asset_id)
                .and_then(|path| rl_from_asset_path(path.path()))
                .is_some_and(|found| found.as_str() == id)
        })
        .and_then(|(_, dimension_type)| dimension_type.default_clock.as_deref())
        .and_then(|clock| ResourceLocation::<Arc<str>>::parse(clock).ok());

    info!(
        dimension = id,
        skybox = ?statics.key.skybox,
        effects = ?statics.key.effects,
        draws = statics.key.draws(),
        clock = ?clock.as_ref().map(|clock| clock.to_string()),
        "built the sky environment"
    );
    commands.insert_resource(SkyEnvironment {
        attributes,
        layout,
        statics,
        clock,
    });
}

/// No chunk is loaded, so no biome speaks and every attribute falls through to
/// what the dimension itself declares.
fn context<'a>(
    camera: Vec3,
    ticks: &'a [f64],
    biomes: &'a SpatialAttributeInterpolator,
    weather: Weather,
) -> EnvironmentContext<'a> {
    EnvironmentContext {
        position: camera.as_dvec3(),
        ticks,
        biomes,
        weather,
    }
}

fn evaluate_sky(
    environment: Res<SkyEnvironment>,
    clocks: Res<WorldClocks>,
    weather: Res<Weather>,
    camera: Single<&GlobalTransform, With<PlayerCamera>>,
    mut frame: ResMut<SkyFrame>,
    mut clear: ResMut<ClearColor>,
    mut ticks: Local<Vec<f64>>,
) {
    environment.attributes.clock_ticks(&clocks, &mut ticks);
    let biomes = SpatialAttributeInterpolator::default();
    let ctx = context(camera.translation(), &ticks, &biomes, *weather);
    environment
        .layout
        .evaluate(&environment.attributes, &ctx, &mut frame);
    clear.0 = environment.fog_color(&frame);
}

/// The per-frame GPU block, in the linear space the render target expects.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyUniform {
    pub disc: [f32; 4],
    pub sunrise: [f32; 4],
    pub angles: [f32; 4],
    pub moon: [f32; 4],
    pub fog: [f32; 4],
    pub cloud_color: [f32; 4],
    pub cloud: [f32; 4],
    pub sky_light: [f32; 4],
    pub block_light: [f32; 4],
    pub ambient: [f32; 4],
}

fn cloud_drift(ticks: f64) -> f32 {
    (ticks * CLOUD_BLOCKS_PER_TICK).rem_euclid(CLOUD_DRIFT_SPAN) as f32
}

fn rgba(packed: u32, w: f32) -> [f32; 4] {
    let channel = |shift: u32| linear(((packed >> shift) & 0xff) as f32 / 255.0);
    [channel(16), channel(8), channel(0), w]
}

fn alpha(packed: u32) -> f32 {
    ((packed >> 24) & 0xff) as f32 / 255.0
}

fn linear(srgb: f32) -> f32 {
    LinearRgba::from(Srgba::new(srgb, srgb, srgb, 1.0)).red
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
        let image = images
            .get(handle)
            .ok_or_else(|| format!("{path} is not loaded"))?;
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

/// The tracks and constants read out of `timeline/day.json` by hand, kept as
/// the reference the attribute system has to reproduce: a drift between the two
/// means the attribute path stopped agreeing with the asset.
#[cfg(test)]
mod reference {
    use bevy::prelude::*;

    pub const DAY: f32 = 24000.0;

    pub const BASE_SKY: Vec3 = rgb(0x78a7ff);
    pub const BASE_FOG: Vec3 = rgb(0xc0d8ff);
    pub const CLOUD_COLOR: Vec4 = argb(0xccffffff);
    pub const CLOUD_HEIGHT: f32 = 192.33;
    pub const CLOUD_FADE: f32 = 2048.0;
    pub const SKY_FOG_END: f32 = 512.0;
    pub const RAIN_BRIGHTNESS: f32 = 1.0;
    pub const AMBIENT: Vec3 = rgb(0x0a0a0a);
    pub const BLOCK_LIGHT_TINT: Vec3 = rgb(0xffd88c);
    pub const MOON_PHASE_COUNT: f32 = 8.0;

    pub const SKY_LIGHT_FACTOR: [(f32, f32); 4] = [
        (730.0, 1.0),
        (11270.0, 1.0),
        (13140.0, 0.24),
        (22860.0, 0.24),
    ];

    pub const SKY_LIGHT_COLOR: [(f32, Vec3); 4] = [
        (730.0, Vec3::ONE),
        (11270.0, Vec3::ONE),
        (13140.0, rgb(0x7a7aff)),
        (22860.0, rgb(0x7a7aff)),
    ];

    pub const SKY_COLOR: [(f32, f32); 4] =
        [(133.0, 1.0), (11867.0, 1.0), (13670.0, 0.0), (22330.0, 0.0)];

    pub const FOG_COLOR: [(f32, Vec3); 4] = [
        (133.0, Vec3::ONE),
        (11867.0, Vec3::ONE),
        (13670.0, rgb(0x0c0c16)),
        (22330.0, rgb(0x161616)),
    ];

    pub const CLOUD_TINT: [(f32, Vec3); 4] = [
        (133.0, Vec3::ONE),
        (11867.0, Vec3::ONE),
        (13670.0, rgb(0x191926)),
        (22330.0, rgb(0x191926)),
    ];

    pub const STAR_BRIGHTNESS: [(f32, f32); 12] = [
        (92.0, 0.037),
        (627.0, 0.0),
        (11373.0, 0.0),
        (11732.0, 0.016),
        (11959.0, 0.044),
        (12399.0, 0.143),
        (12729.0, 0.258),
        (13228.0, 0.5),
        (22772.0, 0.5),
        (23032.0, 0.364),
        (23356.0, 0.225),
        (23758.0, 0.101),
    ];

    pub const SUNRISE: [(f32, Vec4); 32] = [
        (71.0, argb(0x5fefa333)),
        (310.0, argb(0x29f5ba33)),
        (565.0, argb(0x06fbd433)),
        (730.0, argb(0x00ffe533)),
        (11270.0, argb(0x00ffe533)),
        (11397.0, argb(0x04fcd833)),
        (11522.0, argb(0x0ff9cb33)),
        (11690.0, argb(0x29f5ba33)),
        (11929.0, argb(0x5fefa333)),
        (12243.0, argb(0xb1e78733)),
        (12358.0, argb(0xcce47e33)),
        (12512.0, argb(0xe9e07233)),
        (12613.0, argb(0xf6dd6b33)),
        (12732.0, argb(0xfeda6333)),
        (12841.0, argb(0xfed75c33)),
        (13035.0, argb(0xecd25133)),
        (13252.0, argb(0xc1cc4733)),
        (13775.0, argb(0x36be3733)),
        (13888.0, argb(0x1fbb3533)),
        (14039.0, argb(0x09b73333)),
        (14192.0, argb(0x00b33333)),
        (21807.0, argb(0x00b23333)),
        (21961.0, argb(0x09b73333)),
        (22112.0, argb(0x1fbb3533)),
        (22225.0, argb(0x36be3733)),
        (22748.0, argb(0xc1cc4733)),
        (22965.0, argb(0xecd25133)),
        (23159.0, argb(0xfed75c33)),
        (23272.0, argb(0xfeda6333)),
        (23488.0, argb(0xe9e07233)),
        (23642.0, argb(0xcce47e33)),
        (23757.0, argb(0xb1e78733)),
    ];

    pub const fn rgb(hex: u32) -> Vec3 {
        Vec3::new(
            ((hex >> 16) & 0xff) as f32 / 255.0,
            ((hex >> 8) & 0xff) as f32 / 255.0,
            (hex & 0xff) as f32 / 255.0,
        )
    }

    pub const fn argb(hex: u32) -> Vec4 {
        Vec4::new(
            ((hex >> 16) & 0xff) as f32 / 255.0,
            ((hex >> 8) & 0xff) as f32 / 255.0,
            (hex & 0xff) as f32 / 255.0,
            ((hex >> 24) & 0xff) as f32 / 255.0,
        )
    }

    pub fn track<T>(keys: &[(f32, T)], ticks: f32) -> T
    where
        T: Copy + std::ops::Add<T, Output = T> + std::ops::Mul<f32, Output = T>,
    {
        let mut at = ticks.rem_euclid(DAY);
        if at < keys[0].0 {
            at += DAY;
        }
        for (index, &(start, value)) in keys.iter().enumerate() {
            let (mut end, next) = keys[(index + 1) % keys.len()];
            if index + 1 == keys.len() {
                end += DAY;
            }
            if at <= end {
                let fraction = (at - start) / (end - start);
                return value * (1.0 - fraction) + next * fraction;
            }
        }
        keys[0].1
    }

    pub fn sun_angle(ticks: f32) -> f32 {
        use std::f32::consts::{PI, TAU};
        let day = (ticks / DAY - 0.25).rem_euclid(1.0);
        let eased = 0.5 - (day * PI).cos() * 0.5;
        (day * 2.0 + eased) / 3.0 * TAU
    }
}

#[cfg(test)]
mod sky_regression {
    use bevy::prelude::*;
    use mcrs_minecraft_world::attribute::EnvironmentAttributeMap;
    use mcrs_minecraft_world::dimension::dimension_type::Skybox;
    use mcrs_minecraft_world::environment::{DimensionEnvironment, EnvironmentAttributes};
    use mcrs_minecraft_world::timeline::Timeline;
    use mcrs_minecraft_world::world_clock::ClockState;

    use super::reference::*;
    use super::*;

    #[derive(serde::Deserialize)]
    struct Dimension {
        has_skylight: bool,
        has_ceiling: bool,
        #[serde(default)]
        skybox: Skybox,
        #[serde(default)]
        attributes: EnvironmentAttributeMap,
    }

    fn read<T: serde::de::DeserializeOwned>(relative: &str) -> T {
        let path = crate::asset_corpus().join("minecraft").join(relative);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn tagged_timelines(tag: &str, out: &mut Vec<Timeline>) {
        let file: serde_json::Value = read(&format!(
            "tags/timeline/{}.json",
            tag.trim_start_matches("minecraft:")
        ));
        for value in file["values"].as_array().unwrap() {
            let entry = value.as_str().unwrap();
            match entry.strip_prefix('#') {
                Some(nested) => tagged_timelines(nested, out),
                None => out.push(read(&format!(
                    "timeline/{}.json",
                    entry.trim_start_matches("minecraft:")
                ))),
            }
        }
    }

    fn overworld() -> SkyEnvironment {
        let dimension: Dimension = read("dimension_type/overworld.json");
        let mut timelines = Vec::new();
        tagged_timelines("in_overworld", &mut timelines);
        let attributes = EnvironmentAttributes::build(
            &DimensionEnvironment {
                id: "minecraft:overworld",
                attributes: &dimension.attributes,
                skybox: dimension.skybox,
                has_skylight: dimension.has_skylight,
                has_ceiling: dimension.has_ceiling,
            },
            &timelines.iter().collect::<Vec<_>>(),
        )
        .unwrap();
        let layout = SkyLayout::derive(&attributes);
        let biomes = SpatialAttributeInterpolator::default();
        let statics = layout.constants(
            &attributes,
            &context(Vec3::ZERO, &[], &biomes, Weather::default()),
        );
        SkyEnvironment {
            attributes,
            layout,
            statics,
            clock: None,
        }
    }

    /// The uniform the attribute system produces at `ticks`, from a camera
    /// above the horizon in clear weather.
    fn ticks_at(attributes: &EnvironmentAttributes, ticks: i64) -> Vec<f64> {
        let mut clocks = WorldClocks::default();
        for clock in attributes.clocks() {
            clocks.insert(
                clock.clone(),
                ClockState {
                    total_ticks: ticks,
                    ..default()
                },
            );
        }
        let mut resolved = Vec::new();
        attributes.clock_ticks(&clocks, &mut resolved);
        resolved
    }

    fn packed(ticks: i64, camera: Vec3) -> SkyUniform {
        let environment = overworld();
        let resolved = ticks_at(&environment.attributes, ticks);
        let biomes = SpatialAttributeInterpolator::default();
        let mut frame = SkyFrame::default();
        environment.layout.evaluate(
            &environment.attributes,
            &context(camera, &resolved, &biomes, Weather::default()),
            &mut frame,
        );
        environment.uniform(&frame, cloud_drift(ticks as f64))
    }

    #[track_caller]
    fn close(label: &str, got: [f32; 4], want: [f32; 4], epsilon: f32) {
        let apart = (0..4)
            .map(|i| (got[i] - want[i]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            apart <= epsilon,
            "{label}: {got:?} is {apart} away from {want:?}"
        );
    }

    fn expected(ticks: i64, camera: Vec3) -> SkyUniform {
        let at = ticks as f32;
        let sunrise = track(&SUNRISE, at);
        let sun = sun_angle(at);
        let linear3 = |color: Vec3, w: f32| [linear(color.x), linear(color.y), linear(color.z), w];
        SkyUniform {
            disc: linear3(
                BASE_SKY * track(&SKY_COLOR, at),
                f32::from(camera.y < HORIZON),
            ),
            sunrise: linear3(sunrise.truncate(), sunrise.w),
            angles: [
                sun,
                sun + std::f32::consts::PI,
                sun,
                linear(track(&STAR_BRIGHTNESS, at)),
            ],
            moon: [
                1.0 + (at / DAY).floor().rem_euclid(MOON_PHASE_COUNT),
                RAIN_BRIGHTNESS,
                0.0,
                0.0,
            ],
            fog: linear3(BASE_FOG * track(&FOG_COLOR, at), SKY_FOG_END),
            cloud_color: linear3(
                CLOUD_COLOR.truncate() * track(&CLOUD_TINT, at),
                CLOUD_COLOR.w,
            ),
            cloud: [CLOUD_HEIGHT, cloud_drift(ticks as f64), CLOUD_FADE, 0.0],
            sky_light: linear3(track(&SKY_LIGHT_COLOR, at), track(&SKY_LIGHT_FACTOR, at)),
            block_light: linear3(BLOCK_LIGHT_TINT, BLOCK_LIGHT_FACTOR),
            ambient: linear3(AMBIENT, 0.0),
        }
    }

    /// The reference multiplies floats where the attribute system multiplies
    /// packed channels, so the two differ by 8-bit rounding. Measured in linear
    /// space this bound is one sRGB code at the bright end and tighter below.
    const CHANNEL: f32 = 2.5 / 255.0;

    #[test]
    fn the_attribute_system_reproduces_the_hardcoded_tracks_across_the_day() {
        let camera = Vec3::new(0.0, 80.0, 0.0);
        for ticks in [0, 6000, 12000, 12800, 13000, 18000, 23000] {
            let got = packed(ticks, camera);
            let want = expected(ticks, camera);
            let label = |field: &str| format!("{field} at {ticks}");
            close(&label("disc"), got.disc, want.disc, CHANNEL);
            close(&label("sunrise"), got.sunrise, want.sunrise, CHANNEL);
            close(&label("fog"), got.fog, want.fog, CHANNEL);
            close(
                &label("cloud_color"),
                got.cloud_color,
                want.cloud_color,
                CHANNEL,
            );
            close(&label("cloud"), got.cloud, want.cloud, 0.0);
            close(&label("sky_light"), got.sky_light, want.sky_light, CHANNEL);
            close(
                &label("block_light"),
                got.block_light,
                want.block_light,
                CHANNEL,
            );
            close(&label("ambient"), got.ambient, want.ambient, CHANNEL);
            close(&label("moon"), got.moon, want.moon, 0.0);
            // the keyframe easing is a cubic bezier where the reference solved
            // the same curve in closed form
            close(&label("angles"), got.angles, want.angles, 0.002);
        }
    }

    #[test]
    fn the_dark_disc_is_flagged_only_from_under_the_horizon() {
        assert_eq!(
            packed(6000, Vec3::new(0.0, HORIZON + 1.0, 0.0)).disc[3],
            0.0
        );
        assert_eq!(
            packed(6000, Vec3::new(0.0, HORIZON - 1.0, 0.0)).disc[3],
            1.0
        );
    }

    #[test]
    fn the_moon_walks_its_phases_and_returns_to_the_first() {
        let layer = |day: i64| packed(day * 24000 + 18000, Vec3::ZERO).moon[0];
        assert_eq!(
            layer(0),
            1.0,
            "the first night is the full moon, celestial layer 1"
        );
        assert_eq!(layer(3), 4.0);
        assert_eq!(layer(8), 1.0);
    }

    /// The reference hands `star_brightness` to the shader as a plain colour,
    /// so it gets the one sRGB-to-linear conversion every other colour gets and
    /// no further curve.
    #[test]
    fn star_brightness_carries_no_curve_beyond_the_colour_conversion() {
        let brightest = packed(18000, Vec3::ZERO).angles[3];
        assert_eq!(brightest, linear(0.5));
        assert_eq!(packed(6000, Vec3::ZERO).angles[3], 0.0);
    }

    #[test]
    fn the_clouds_drift_with_the_clock_and_wrap_inside_the_widest_field() {
        assert_eq!(cloud_drift(0.0), 0.0);
        assert!(
            (cloud_drift(6000.0) - 180.0).abs() < 1e-3,
            "{}",
            cloud_drift(6000.0)
        );
        let span = 4096.0 * 12.0;
        let a_lot = 1_000_000_000.0;
        assert!(cloud_drift(a_lot) < span as f32);
        assert_eq!(
            cloud_drift(a_lot),
            cloud_drift(a_lot + span / CLOUD_BLOCKS_PER_TICK)
        );
    }

    #[test]
    fn the_lighting_attributes_still_match_the_reference_tracks() {
        for ticks in [0, 6000, 13000, 18000, 23000] {
            let at = ticks as f32;
            let attributes = overworld().attributes;
            let resolved = ticks_at(&attributes, ticks);
            let biomes = SpatialAttributeInterpolator::default();
            let ctx = context(Vec3::ZERO, &resolved, &biomes, Weather::default());

            let float = |id: &str| match attributes.value(id, &ctx).unwrap() {
                mcrs_minecraft_world::attribute::AttributeValue::Float(value) => value,
                other => panic!("{id} is not a float: {other:?}"),
            };
            let color = |id: &str| match attributes.value(id, &ctx).unwrap() {
                mcrs_minecraft_world::attribute::AttributeValue::Color(packed) => Vec3::new(
                    ((packed >> 16) & 0xff) as f32 / 255.0,
                    ((packed >> 8) & 0xff) as f32 / 255.0,
                    (packed & 0xff) as f32 / 255.0,
                ),
                other => panic!("{id} is not a colour: {other:?}"),
            };

            let factor = float("minecraft:visual/sky_light_factor");
            assert!(
                (factor - track(&SKY_LIGHT_FACTOR, at)).abs() <= 1e-3,
                "sky_light_factor at {ticks}: {factor}"
            );
            for (id, want) in [
                (
                    "minecraft:visual/sky_light_color",
                    track(&SKY_LIGHT_COLOR, at),
                ),
                ("minecraft:visual/ambient_light_color", AMBIENT),
                ("minecraft:visual/block_light_tint", BLOCK_LIGHT_TINT),
            ] {
                let got = color(id);
                assert!(
                    got.abs_diff_eq(want, CHANNEL),
                    "{id} at {ticks}: {got:?} is not {want:?}"
                );
            }
        }
    }
}
