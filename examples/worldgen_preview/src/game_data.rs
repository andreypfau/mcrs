use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{self, Blocks, CORPUS_DIRECTORY};
use mcrs_minecraft_datapack::{
    DirectoryPacks, Level, PackContent, PackFiles, REPOSITORY_PACKS, RootPacks,
};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_game_registries::dimension::{Dimensions, bake};
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_game_registries::world_preset_keys::NORMAL;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_heightmap::heightmap_predicates;
use mcrs_minecraft_registries::{Registries, RegistryAdditions, Stage};
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_worldgen::registries::add_vanilla_worldgen;
use mcrs_minecraft_worldgen_feature::template::Template;
use mcrs_minecraft_worldgen_feature::template_file::decode_template;
use mcrs_minecraft_worldgen_generator::dimension_plan::DimensionPlan;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::template_names::named_templates;
use mcrs_minecraft_worldgen_generator::world_tables::{WorldTables, WorldgenInputs};

use crate::arguments::Arguments;

pub struct GameData {
    pub registries: Registries,
    pub blocks: Blocks,
    pub biome_names: Vec<ResourceLocationBuf>,
    pub min_y: i32,
    pub height: u32,
    pub fill: FillContext,
}

impl GameData {
    pub fn load(arguments: &Arguments) -> anyhow::Result<Self> {
        let mut packs = DirectoryPacks::new(arguments.assets.clone(), arguments.vanilla.clone());
        for pack in &arguments.packs {
            packs = packs.with_pack(pack)?;
        }
        let mut additions = RegistryAdditions::default();
        additions
            .local_packs
            .extend(REPOSITORY_PACKS.map(str::to_owned));
        add_vanilla_worldgen(&mut additions)?;
        let (registries, diagnostics) = load_registries(&packs, additions)?;
        for diagnostic in diagnostics.iter() {
            if diagnostic.level() != Level::Info {
                eprintln!("{diagnostic}");
            }
        }

        let blocks = Blocks(Arc::new(block_definitions(&arguments.assets)?));
        let tags = registries
            .tags::<Block>()
            .context("the registries hold no block tags")?;
        let tables = WorldTables::build(WorldgenInputs {
            registries: &registries,
            blocks: &blocks,
            templates: templates(&packs, &registries)?,
            predicates: heightmap_predicates(&blocks, &tags),
        })?;
        for missing in tables.missing() {
            eprintln!("a worldgen table is missing: {missing:?}");
        }

        let entry = bake(preset_dimensions(&registries)?, &registries)
            .map_err(anyhow::Error::msg)?
            .into_iter()
            .find(|(key, _)| *key.location() == arguments.dimension)
            .map(|(_, entry)| entry)
            .with_context(|| format!("the world has no dimension {}", arguments.dimension))?;
        let dimension_type = registries
            .registry::<DimensionTypeFile>()
            .and_then(|types| types.get(entry.dimension_type))
            .context("the dimension names a dimension type the registry does not hold")?;
        let plan = DimensionPlan::build(
            &tables,
            &registries,
            &blocks,
            arguments.seed,
            &arguments.dimension,
            &entry.generator,
        )?;
        let (min_y, height) = (dimension_type.min_y, dimension_type.height);
        let fill = plan.fill_context(min_y, height >> 4);
        let biome_names = registries
            .registry::<BiomeFile>()
            .context("the registries hold no biomes")?
            .iter()
            .map(|(_, name, _)| name.clone())
            .collect();
        Ok(Self {
            registries,
            blocks,
            biome_names,
            min_y,
            height,
            fill,
        })
    }
}

fn preset_dimensions(registries: &Registries) -> anyhow::Result<&Dimensions> {
    let presets = registries
        .registry::<WorldPreset>()
        .context("the registries hold no world presets")?;
    presets
        .by_name(NORMAL.location())
        .and_then(|id| presets.get(id))
        .map(|preset| &preset.dimensions)
        .with_context(|| format!("no world preset {}", NORMAL.location()))
}

fn block_definitions(assets: &Path) -> anyhow::Result<definition::BlockDefinitions> {
    let directory = assets.join(CORPUS_DIRECTORY);
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory).with_context(|| directory.display().to_string())? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let bytes = fs::read(&path).with_context(|| path.display().to_string())?;
            files.push((path.display().to_string(), bytes));
        }
    }
    files.sort();
    let names: Vec<&str> = Block::ALL
        .iter()
        .map(|block| block.as_static_str())
        .collect();
    let (definitions, _) = definition::build_block_definitions(&names, files)?;
    Ok(definitions)
}

// The registries name the templates; each one is read from the highest pack of the stack that
// holds it, overlays and filters included, and a template only built into the server comes last.
fn templates(
    packs: &DirectoryPacks,
    registries: &Registries,
) -> anyhow::Result<BTreeMap<ResourceLocationBuf, Template>> {
    let mut builtin = mcrs_minecraft_worldgen_builtin::templates()?;
    let names = named_templates(registries)?;
    let relative = |name: &ResourceLocationBuf| format!("structure/{}.nbt", name.path());
    let wanted: BTreeSet<String> = names.iter().map(relative).collect();
    let stack = registries.stack(Stage::Reloadable);
    let mut files = PackFiles::default();
    RootPacks::read(packs)?.read_files(packs, stack, &mut files, ".nbt", |path| {
        wanted.contains(path)
    })?;
    let (visible, _) = stack.files(&mut files);
    let mut top: BTreeMap<(&str, &str), &[u8]> = BTreeMap::new();
    for file in visible {
        if let PackContent::Bytes(bytes) = file.content {
            top.insert((file.namespace, file.relative), bytes);
        }
    }
    let mut templates = BTreeMap::new();
    for name in names {
        let template = match top.get(&(name.namespace(), relative(&name).as_str())) {
            Some(bytes) => {
                decode_template(bytes).with_context(|| format!("the template {name}"))?
            }
            None => match builtin.remove(&name) {
                Some(template) => template,
                None => continue,
            },
        };
        templates.insert(name, template);
    }
    Ok(templates)
}
