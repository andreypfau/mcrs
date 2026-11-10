# Assets and Rendering (0.19)

## Data assets versus runtime assets

Two kinds of data reach an app from files, and they want different handling.

- **Data that is baked once** (balance tables, level definitions, lookup
  tables). Parse and validate it at load, build the runtime form, then drop the
  parsed source. A malformed file then fails at load with context, not later at
  first use. A resource built in `Startup` is often enough; use an `Asset` when
  you want hot reload of that data.
- **Runtime assets** (textures, models, sounds, shaders) that the renderer or
  the audio backend consumes through `Handle<T>`. These use `AssetServer`.

When baked data does go through `AssetServer`, rebuild the runtime form on
`AssetEvent::Modified` and drop the parsed asset afterwards if nothing else
needs it, so two copies do not stay in memory.

## Custom asset loader

```rust
use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
};
use serde::Deserialize;

#[derive(Asset, TypePath, Debug, Deserialize)]
pub struct Palette {
    pub colors: Vec<[u8; 3]>,
}

#[derive(Default, TypePath)]
pub struct PaletteLoader;

impl AssetLoader for PaletteLoader {
    type Asset = Palette;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _ctx: &mut LoadContext<'_>,
    ) -> Result<Palette, std::io::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        serde_json::from_slice(&bytes).map_err(std::io::Error::other)
    }

    fn extensions(&self) -> &[&str] {
        &["palette.json"]
    }
}

app.init_asset::<Palette>().init_asset_loader::<PaletteLoader>();
```

- `AssetLoader` requires `TypePath` on the loader itself, not only on the asset.
- Deserialize into a typed struct. A hand-written walk over `serde_json::Value`
  ties the type to one format and defers errors to the point of use.
- An extension with a dot (`palette.json`) matches the whole suffix, so it does
  not collide with other `.json` loaders.
- The loader runs on `IoTaskPool`. Limit untrusted lengths and counts before
  allocating for them.

## AssetServer

- `asset_server.load(path)` returns a `Handle<T>` at once and loads in the
  background. Read the result from `Res<Assets<T>>` when it is there, or check
  `asset_server.load_state(&handle)`.
- `MessageReader<AssetEvent<T>>` reports `Added`, `Modified`,
  `LoadedWithDependencies`, `Removed` and `Unused`.
- `embedded_asset!(app, "path")` compiles a file into the binary; load it with
  `embedded://crate_name/path`.
- Hot reload needs the `file_watcher` feature of `bevy_asset` (or `bevy`). Keep
  it a development feature.

## Reusable rendering plugins

A rendering plugin meant for reuse reads world data and produces meshes and
draws. It does not spawn the window or the camera, and it does not replace the
host's render graph; the host adds those.

## Background mesh generation

Procedural meshes (terrain, voxels, editors) are built on
`AsyncComputeTaskPool`:

- Each job carries a revision of its input. A result whose revision is older
  than the current one is discarded.
- Keep one queued or running job per region, and coalesce new edits into it.
- Bound how many jobs start and how many meshes upload per frame.
- Swap the mesh and anything derived from it (a collider) together.

## Shader cost

- Values constant across a draw (fog colour, time of day) are computed on the
  CPU once per frame and written to one uniform, not computed per fragment.
- Specialize pipelines by a key (enabled effects) instead of branching at
  runtime in the shader.

## Materials and shaders

```rust
use bevy::{
    pbr::{Material, MaterialPlugin},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub tint: LinearRgba,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
```

- `ShaderRef` is in `bevy::shader` (`bevy_shader`), not in
  `render::render_resource`.
- A custom material binds at group 3 (`MATERIAL_BIND_GROUP_INDEX`). In WGSL
  write `@group(#{MATERIAL_BIND_GROUP}) @binding(0)`.
- To vary a few uniforms on top of PBR, use
  `ExtendedMaterial<StandardMaterial, E>` with `E: MaterialExtension`.
- Reuse one `Handle<Mesh>` and one `Handle<M>` for identical objects; a new
  asset per entity breaks batching.

## Renamed items that answers from memory get wrong

| Old | 0.19 |
|---|---|
| `TransformSystem::TransformPropagate` | `TransformSystems::Propagate` (in `PostUpdate`) |
| `bevy::render::view::VisibilityRange` | `bevy::camera::visibility::VisibilityRange` |
| `PointLight::shadows_enabled` | `PointLight::shadow_maps_enabled` |
| `render::render_resource::ShaderRef` | `bevy::shader::ShaderRef` |
| material group 2 | group 3 |
