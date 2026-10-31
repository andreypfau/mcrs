use crate::joined_game::CurrentDimension;
use bevy::asset::{AssetPath, RenderAssetUsages};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy::render::{Extract, ExtractSchedule, RenderApp};
use bevy::transform::TransformSystems;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Id;

use crate::sky_state::{SkyField, SkyFrame, SkyKey, SkyLayout, SkyStatic, SkyValue};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::{
    EnvironmentAttributes, EnvironmentContext, Weather,
};
use mcrs_minecraft_environment::attribute::MoonPhase;
use mcrs_minecraft_environment::world_clock::{WorldClock, WorldClocks};

use crate::player::PlayerCamera;
use crate::registries::{SessionSet, entered_configuration};
use crate::vanilla::{self, VanillaAssets};
use mcrs_minecraft_render::sky::{ExtractedSky, SkyDrawsOnly, SkyRenderPlugin, SkyUniform};

const SUN: &str = "minecraft/textures/environment/celestial/sun.png";

/// Layer 0 is the sun; the moon phases follow in `MoonPhase::ALL` order.
fn celestial_paths() -> impl Iterator<Item = String> {
    std::iter::once(SUN.to_owned()).chain(MoonPhase::ALL.iter().map(|phase| {
        format!(
            "minecraft/textures/environment/celestial/moon/{}.png",
            phase.name()
        )
    }))
}

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

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyFrame>()
            .add_plugins(SkyRenderPlugin)
            .add_systems(OnEnter(VanillaAssets::Ready), request_sources)
            .add_systems(
                Update,
                assemble_arrays.run_if(resource_exists::<SkySources>),
            )
            .add_systems(
                Update,
                (
                    clear_sky
                        .in_set(SessionSet::Clear)
                        .run_if(entered_configuration),
                    build_sky_environment
                        .after(SessionSet::Derive)
                        .run_if(resource_exists::<CurrentRegistries>),
                ),
            )
            .add_systems(
                PostUpdate,
                evaluate_sky
                    .after(TransformSystems::Propagate)
                    .run_if(resource_exists::<SkyEnvironment>),
            );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, extract_sky);
        }
    }
}

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
    clock: Option<Id<mcrs_minecraft_environment::world_clock::WorldClock>>,
}

impl SkyEnvironment {
    pub fn key(&self) -> SkyKey {
        self.statics.key
    }

    pub fn drift(&self, clocks: &WorldClocks) -> f32 {
        cloud_drift(
            self.clock
                .and_then(|clock| clocks.get(clock))
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
                Srgba::gamma_function(self.scalar(frame, SkyField::StarBrightness)),
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
            sky_light: raw(
                color(SkyField::SkyLightColor),
                self.scalar(frame, SkyField::SkyLightFactor),
            ),
            block_light: raw(color(SkyField::BlockLightTint), BLOCK_LIGHT_FACTOR),
            ambient: raw(color(SkyField::AmbientLightColor), 0.0),
        }
    }

    /// The reference clears the frame to the fog colour before the sky disc
    /// goes down, which is all that shows below the horizon until terrain does.
    fn fog_color(&self, frame: &SkyFrame) -> Color {
        let [red, green, blue, _] = rgba(self.value(frame, SkyField::FogColor).color(), 0.0);
        Color::LinearRgba(LinearRgba::rgb(red, green, blue))
    }
}

fn clear_sky(mut commands: Commands) {
    commands.remove_resource::<SkyEnvironment>();
}

fn build_sky_environment(
    current: Single<&CurrentDimension, Changed<CurrentDimension>>,
    registries: Res<CurrentRegistries>,
    clocks: Res<WorldClocks>,
    mut commands: Commands,
) {
    let (Some(dimension_types), Some(world_clocks)) = (
        registries.registry::<DimensionTypeFile>(),
        registries.registry::<WorldClock>(),
    ) else {
        error!("the registries hold no dimension types to draw a sky from");
        return;
    };
    let CurrentDimension {
        key: dimension,
        dimension_type,
    } = &**current;
    let attributes =
        match EnvironmentAttributes::of_dimension(&registries, dimension, *dimension_type) {
            Ok(attributes) => attributes,
            Err(error) => {
                error!(%dimension, %error, "no environment to draw a sky from");
                commands.remove_resource::<SkyEnvironment>();
                return;
            }
        };
    let type_id = Id::checked(dimension_type.number(), dimension_types.len());
    let layout = SkyLayout::derive(&attributes);

    let mut ticks = Vec::new();
    attributes.clock_ticks(&clocks, &mut ticks);
    let statics = layout.constants(&attributes, &EnvironmentContext { ticks: &ticks });

    let clock = type_id
        .and_then(|id| dimension_types.get(id))
        .and_then(|file| file.default_clock);

    info!(
        %dimension,
        dimension_type = ?type_id.and_then(|id| dimension_types.name(id)).map(ToString::to_string),
        skybox = ?statics.key.skybox,
        effects = ?statics.key.effects,
        draws = statics.key.draws(),
        clock = ?clock
            .and_then(|clock| Id::checked(clock.number(), world_clocks.len()))
            .and_then(|clock| world_clocks.name(clock))
            .map(ToString::to_string),
        "built the sky environment"
    );
    commands.insert_resource(SkyEnvironment {
        attributes,
        layout,
        statics,
        clock,
    });
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
    environment.layout.evaluate(
        &environment.attributes,
        &EnvironmentContext { ticks: &ticks },
        camera.translation(),
        *weather,
        &mut frame,
    );
    clear.0 = environment.fog_color(&frame);
}

fn extract_sky(
    mut commands: Commands,
    environment: Extract<Option<Res<SkyEnvironment>>>,
    textures: Extract<Option<Res<SkyTextures>>>,
    frame: Extract<Res<SkyFrame>>,
    clocks: Extract<Res<WorldClocks>>,
    only: Extract<Option<Res<SkyDrawsOnly>>>,
) {
    let (Some(environment), Some(textures)) = (environment.as_ref(), textures.as_ref()) else {
        return;
    };
    let mut effects = environment.key().effects;
    if let Some(only) = only.as_ref() {
        effects &= only.0;
    }
    commands.insert_resource(ExtractedSky {
        uniform: environment.uniform(&frame, environment.drift(&clocks)),
        effects,
        celestials: textures.celestials.id(),
        clouds: textures.clouds.id(),
    });
}

fn cloud_drift(ticks: f64) -> f32 {
    (ticks * CLOUD_BLOCKS_PER_TICK).rem_euclid(CLOUD_DRIFT_SPAN) as f32
}

pub(crate) const fn rgb(packed: u32) -> Vec3 {
    Vec3::new(
        ((packed >> 16) & 0xff) as f32 / 255.0,
        ((packed >> 8) & 0xff) as f32 / 255.0,
        (packed & 0xff) as f32 / 255.0,
    )
}

fn rgba(packed: u32, w: f32) -> [f32; 4] {
    rgb(packed).map(Srgba::gamma_function).extend(w).to_array()
}

/// The light colours feed vanilla's lightmap, which works on the bytes as they are written.
fn raw(packed: u32, w: f32) -> [f32; 4] {
    rgb(packed).extend(w).to_array()
}

fn alpha(packed: u32) -> f32 {
    ((packed >> 24) & 0xff) as f32 / 255.0
}

fn request_sources(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |path: String| {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
            })
            .load(AssetPath::from(path).with_source(vanilla::SOURCE))
    };
    commands.insert_resource(SkySources {
        celestials: celestial_paths().map(load).collect(),
        clouds: CLOUD_LAYERS
            .iter()
            .map(|path| load((*path).to_owned()))
            .collect(),
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
    // Only a stack of layers is an array. A GL backend picks the texture
    // target from the layer count when it creates the texture, so a one-layer
    // texture is a plain 2D one there whatever view is asked for, and binding
    // it against an array in the shader silently samples nothing.
    if handles.len() > 1 {
        array.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..default()
        });
    }
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

    mod jar {
        use super::*;

        #[test]
        fn every_layer_path_names_a_file_in_the_corpus() {
            for path in celestial_paths().chain(CLOUD_LAYERS.iter().map(|p| (*p).to_owned())) {
                assert!(
                    crate::model::Pack::corpus().get(&path).is_some(),
                    "{path} is missing"
                );
            }
        }
    }
}

#[cfg(test)]
mod sky_regression {
    use mcrs_minecraft_dimension::Skybox;
    use mcrs_minecraft_dimension_environment::environment::{
        DimensionEnvironment, EnvironmentAttributes,
    };
    use mcrs_minecraft_environment::attribute::EnvironmentAttributeMap;
    use mcrs_minecraft_environment::timeline::Timeline;
    use mcrs_minecraft_environment::world_clock::ClockState;

    use super::*;
    use crate::sky_state::{test_clock_count, test_resolved};
    use mcrs_minecraft_registries::Named;
    use mcrs_minecraft_resource::ResourceKeyBuf;

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
        let path = mcrs_minecraft_fixtures::vanilla_dir().join(relative);
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
                None => out.push(test_resolved(read::<Timeline<Named>>(&format!(
                    "timeline/{}.json",
                    entry.trim_start_matches("minecraft:")
                )))),
            }
        }
    }

    fn overworld() -> SkyEnvironment {
        let dimension: Dimension = read("dimension_type/overworld.json");
        let mut timelines = Vec::new();
        tagged_timelines("in_overworld", &mut timelines);
        let attributes = EnvironmentAttributes::build(
            &DimensionEnvironment {
                attributes: &dimension.attributes,
                skybox: dimension.skybox,
                can_have_weather: dimension.has_skylight && !dimension.has_ceiling,
            },
            &timelines.iter().collect::<Vec<_>>(),
            test_clock_count(),
        )
        .unwrap();
        let layout = SkyLayout::derive(&attributes);
        let statics = layout.constants(&attributes, &EnvironmentContext { ticks: &[] });
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
                *clock,
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
        let mut frame = SkyFrame::default();
        environment.layout.evaluate(
            &environment.attributes,
            &EnvironmentContext { ticks: &resolved },
            camera,
            Weather::default(),
            &mut frame,
        );
        environment.uniform(&frame, cloud_drift(ticks as f64))
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
    fn the_drift_follows_the_default_clock_by_id_and_reads_none_without_one() {
        let overworld_clock = mcrs_minecraft_registries::Resolver::id(
            mcrs_minecraft_world::registry_load::test_registries(),
            &ResourceKeyBuf::from_location(
                (*mcrs_minecraft_environment::keys::world_clock::OVERWORLD.location()).into(),
            ),
        )
        .unwrap();
        let mut clocks = WorldClocks::default();
        clocks.insert(
            overworld_clock,
            ClockState {
                total_ticks: 6000,
                ..default()
            },
        );

        let mut environment = overworld();
        assert_eq!(environment.drift(&clocks), 0.0);

        environment.clock = Some(overworld_clock);
        assert_eq!(environment.drift(&clocks), cloud_drift(6000.0));
    }
}
