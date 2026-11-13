use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::Path;

use bevy::shader::ShaderDefVal;
use mcrs_minecraft_mesh::STREAMS;
use mcrs_minecraft_render::RenderPath;
use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use wesl::error::ResolveError;
use wesl::syntax::{ModulePath, PathOrigin};
use wesl::{CompileOptions, Resolver};

/// The input of a fullscreen fragment shader as Bevy's own module states it.
const FULLSCREEN_VERTEX_OUTPUT: &str = "struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};";

/// The items of Bevy's render modules that the sky shader reads.
const BEVY_MATHS: &str = "const PI: f32 = 3.141592653589793;
const HALF_PI: f32 = 1.57079632679;";
const BEVY_VIEW: &str = "struct View {
    clip_from_world: mat4x4<f32>,
    world_from_clip: mat4x4<f32>,
    world_position: vec3<f32>,
};";

/// The crate's shader files by module path, Bevy's fullscreen module, and the `constants` module
/// Bevy writes from the pipeline's shader defs.
struct Sources {
    constants: String,
}

impl Resolver for Sources {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        match &path.origin {
            PathOrigin::Package(package) if package == "constants" => {
                return Ok(Cow::Borrowed(&self.constants));
            }
            PathOrigin::Package(package) if package == "bevy_core_pipeline" => {
                return Ok(Cow::Borrowed(FULLSCREEN_VERTEX_OUTPUT));
            }
            PathOrigin::Package(package) if package == "bevy_render" => {
                match path.components.as_slice() {
                    [module] if module == "maths" => return Ok(Cow::Borrowed(BEVY_MATHS)),
                    [module] if module == "view" => return Ok(Cow::Borrowed(BEVY_VIEW)),
                    _ => {}
                }
            }
            _ => {}
        }
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(path.components.join("/"))
            .with_extension("wesl");
        std::fs::read_to_string(&file)
            .map(Cow::Owned)
            .map_err(|error| ResolveError::ModuleNotFound(path.clone(), error.to_string()))
    }
}

/// The WGSL Bevy hands the device for a pipeline compiled with `defs`.
pub(crate) fn wgsl(shader: &str, defs: Vec<ShaderDefVal>) -> String {
    wgsl_at(&["shaders", "core", shader], defs)
}

fn wgsl_at(components: &[&str], defs: Vec<ShaderDefVal>) -> String {
    let shader = components.join("/");
    let mut options = CompileOptions {
        imports: true,
        condcomp: true,
        visibility: false,
        ..CompileOptions::default()
    };
    let mut constants = String::new();
    for def in defs {
        let (name, value) = match def {
            ShaderDefVal::Bool(name, on) => {
                options.features.flags.insert(name.to_string(), on.into());
                continue;
            }
            ShaderDefVal::Int(name, value) => (name, i64::from(value)),
            ShaderDefVal::UInt(name, value) => (name, i64::from(value)),
        };
        options.features.flags.insert(name.to_string(), true.into());
        writeln!(constants, "const {name} = {value};").unwrap();
    }
    let main = ModulePath {
        origin: PathOrigin::Absolute,
        components: components.iter().map(|&part| part.to_owned()).collect(),
    };
    wesl::compile(&main, &options, &Sources { constants })
        .unwrap_or_else(|error| panic!("{shader}: {error}"))
        .to_string()
}

fn compile(shader: &str, defs: Vec<ShaderDefVal>) -> naga::Module {
    naga::front::wgsl::parse_str(&wgsl(shader, defs))
        .unwrap_or_else(|error| panic!("{shader}: {error}"))
}

/// The defs of the cull pipelines on a device without subgroups, over `section_slots` sections.
pub(crate) fn cull_defs(section_slots: u32) -> Vec<ShaderDefVal> {
    vec![
        ShaderDefVal::UInt("CULL_THREADS".into(), 32),
        ShaderDefVal::UInt("GROUP_THREADS".into(), 256),
        ShaderDefVal::UInt("GROUPS_PER_STEP".into(), 2),
        ShaderDefVal::UInt("STREAMS".into(), STREAMS as u32),
        ShaderDefVal::UInt("SECTION_SLOTS".into(), section_slots),
        ShaderDefVal::UInt("CAVE_WORDS".into(), section_slots.div_ceil(32)),
        ShaderDefVal::UInt("MAX_WORKGROUPS".into(), 65535),
        ShaderDefVal::Int("MODEL_BIAS_CONSTANT".into(), 0),
        ShaderDefVal::UInt("MODEL_BIAS_SLOPE_BITS".into(), 0),
    ]
}

/// GLSL ES 3.10 builtins naga writes for WGSL ones without checking the version, so a shader that
/// reaches one compiles for WebGL2 and then fails to link in the browser.
const ES_310_BUILTINS: [&str; 6] = [
    "bitfieldExtract",
    "bitfieldInsert",
    "bitfieldReverse",
    "bitCount",
    "findLSB",
    "findMSB",
];

/// Each shader the minimal path draws with, and the defs of a pipeline that uses it beside those
/// of the path.
const PIPELINES: [(&str, &[&str]); 9] = [
    ("greedy", &[]),
    ("greedy", &["WIREFRAME"]),
    ("model", &[]),
    ("model", &["WIREFRAME"]),
    ("lighting_pass", &[]),
    ("lighting_pass", &["LIGHTING_TERM"]),
    ("gbuffer_views", &["SHOW_GRID"]),
    ("show", &[]),
    ("show", &["PYRAMID"]),
];

#[test]
fn the_world_shaders_compile_for_both_paths_and_the_minimal_one_for_webgl2() {
    for (shader, extra) in PIPELINES {
        for path in [RenderPath::Full, RenderPath::Minimal] {
            let mut defs = path.shader_defs();
            defs.extend(extra.iter().map(|&name| ShaderDefVal::from(name)));
            let module = compile(shader, defs);
            let info = Validator::new(ValidationFlags::all(), Capabilities::default())
                .validate(&module)
                .unwrap_or_else(|error| panic!("{shader} {extra:?} on {path:?}: {error:?}"));
            if !path.is_full() {
                assert_writes_for_webgl2(&format!("{shader} {extra:?}"), &module, &info);
            }
        }
    }
}

#[test]
fn the_sky_shader_compiles_with_each_entry_point_for_webgl2() {
    let source = wgsl_at(&["shaders", "sky"], Vec::new());
    let module =
        naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| panic!("sky: {error}"));
    let info = Validator::new(ValidationFlags::all(), Capabilities::default())
        .validate(&module)
        .unwrap_or_else(|error| panic!("sky: {error:?}"));
    for name in [
        "vertex_occluder",
        "fragment_occluder",
        "vertex_disc",
        "fragment_disc",
    ] {
        assert!(
            module.entry_points.iter().any(|entry| entry.name == name),
            "the sky shader has no entry point {name}"
        );
    }
    assert_writes_for_webgl2("sky", &module, &info);
}

fn assert_writes_for_webgl2(label: &str, module: &naga::Module, info: &naga::valid::ModuleInfo) {
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        ..glsl::Options::default()
    };
    for entry in &module.entry_points {
        let pipeline = glsl::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        let mut out = String::new();
        let written = glsl::Writer::new(
            &mut out,
            module,
            info,
            &options,
            &pipeline,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .and_then(|mut writer| writer.write());
        if let Err(error) = written {
            panic!("{label} {} for WebGL2: {error}", entry.name);
        }
        for builtin in ES_310_BUILTINS {
            assert!(
                !out.contains(builtin),
                "{label} {} calls {builtin}, which WebGL2 does not have",
                entry.name
            );
        }
    }
}

#[test]
fn the_cull_shader_compiles_for_the_full_path() {
    let module = compile("cull", cull_defs(64));
    if let Err(error) =
        Validator::new(ValidationFlags::all(), Capabilities::default()).validate(&module)
    {
        panic!("cull: {error:?}");
    }
}
