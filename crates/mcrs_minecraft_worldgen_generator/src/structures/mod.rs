use crate::block_state::try_resolve_state;
use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_assets::DynTagRegistry;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_core::{ResourceLocation, TagKey};
use mcrs_minecraft_registry::DynRegistryIndex;
use mcrs_minecraft_worldgen_density::proto::BlockState as ProtoBlockState;
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;
use mcrs_minecraft_worldgen_feature::spawn_condition::{
    SpawnSelector, VariantTable, VariantTables,
};
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_minecraft_worldgen_feature::template::{
    FrozenTemplate, PaletteState, ResolvedState, Template, TemplateManifest, bounding_box,
};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_structure::frozen::{
    ElementId, FrozenElement, FrozenPool, FrozenSet, FrozenStructure, FrozenStructures,
    OceanRuinConfig, PoolId, SetId, StructureId, StructureKind, TemplateId,
};
use mcrs_minecraft_worldgen_structure::hardcoded::{ruined_portal, shipwreck};
use mcrs_minecraft_worldgen_structure::piece::TERRAIN_MARGIN;
use mcrs_minecraft_worldgen_structure::site::site_implies_piece;
use mcrs_minecraft_worldgen_structure::{
    DecorationStep, PoolAlias, PoolElement, Structure, StructurePlacement, StructureSet,
    TemplatePool, TerrainAdaptation,
};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub mod index;

pub mod place;

// Vanilla marks these `dynamicShape()` and never files them as full blocks when
// ordering a template; the block schema carries no such flag, so the set lives here.
pub(crate) const DYNAMIC_SHAPE_BLOCKS: &[&str] = &[
    "minecraft:moving_piston",
    "minecraft:shulker_box",
    "minecraft:white_shulker_box",
    "minecraft:orange_shulker_box",
    "minecraft:magenta_shulker_box",
    "minecraft:light_blue_shulker_box",
    "minecraft:yellow_shulker_box",
    "minecraft:lime_shulker_box",
    "minecraft:pink_shulker_box",
    "minecraft:gray_shulker_box",
    "minecraft:light_gray_shulker_box",
    "minecraft:cyan_shulker_box",
    "minecraft:purple_shulker_box",
    "minecraft:blue_shulker_box",
    "minecraft:brown_shulker_box",
    "minecraft:green_shulker_box",
    "minecraft:red_shulker_box",
    "minecraft:black_shulker_box",
    "minecraft:bamboo",
    "minecraft:scaffolding",
    "minecraft:powder_snow",
    "minecraft:pointed_dripstone",
    "minecraft:sulfur_spike",
];

pub fn resolve_palette_state(
    blocks: &BlockDefinitions,
    state: &PaletteState,
) -> Option<ResolvedState> {
    let proto = ProtoBlockState {
        name: state.id.clone(),
        properties: state.properties.clone(),
    };
    let id = try_resolve_state(blocks, &proto)?;
    let full_block = blocks
        .state(id)
        .flags
        .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
        && !DYNAMIC_SHAPE_BLOCKS.contains(&state.id.as_str());
    Some(ResolvedState {
        id: VoxelId(id.0),
        full_block,
    })
}

pub struct StructureInputs<'a> {
    pub sets: &'a BTreeMap<ResourceLocation, StructureSet>,
    pub structures: &'a BTreeMap<ResourceLocation, Structure>,
    pub pools: &'a BTreeMap<ResourceLocation, TemplatePool>,
    pub template: &'a dyn Fn(&ResourceLocation) -> Option<Cow<'a, Template>>,
    pub resolve: &'a dyn Fn(&PaletteState) -> Option<ResolvedState>,
    pub biomes: &'a DynRegistryIndex<Biome>,
    pub biome_tags: &'a DynTagRegistry<Biome>,
    pub variants: &'a VariantInputs<'a>,
}

/// The variant registries the structures' mobs draw from, each map in its
/// registry order with the asset's `spawn_conditions`, and each sound registry
/// as its ids in order.
#[derive(Default)]
pub struct VariantInputs<'a> {
    pub chickens: Option<&'a BTreeMap<ResourceLocation, Vec<SpawnSelector>>>,
    pub chicken_sounds: &'a [ResourceLocation],
    pub zombie_nautiluses: Option<&'a BTreeMap<ResourceLocation, Vec<SpawnSelector>>>,
}

const MAX_JIGSAW_RANGE: i32 = 128;

pub fn freeze(inputs: &StructureInputs<'_>) -> Result<FrozenStructures, String> {
    let mut frozen = FrozenStructures::default();
    freeze_pools(inputs, &mut frozen)?;
    freeze_structures(inputs, &mut frozen)?;
    freeze_sets(inputs, &mut frozen)?;
    freeze_variants(inputs, &mut frozen)?;
    Ok(frozen)
}

fn freeze_variants(
    inputs: &StructureInputs<'_>,
    frozen: &mut FrozenStructures,
) -> Result<(), String> {
    let table = |registry: &str,
                 entries: Option<&BTreeMap<ResourceLocation, Vec<SpawnSelector>>>|
     -> Result<VariantTable, String> {
        let Some(entries) = entries else {
            return Ok(VariantTable::default());
        };
        let owner = ResourceLocation::parse(registry).expect("a literal id");
        VariantTable::freeze(
            entries
                .iter()
                .map(|(id, selectors)| (id.clone(), selectors.as_slice())),
            &|set| {
                let mut ids = FixedBitSet::with_capacity(frozen.structures.len());
                match set {
                    HolderSet::Tag(tag) => {
                        return Err(format!(
                            "names the structure tag #{tag}, which is not loaded"
                        ));
                    }
                    HolderSet::One(_) | HolderSet::List(_) => {
                        for id in set.entries() {
                            let index = frozen.structure_ids.get(id).ok_or_else(|| {
                                format!("names the structure {id}, which is not loaded")
                            })?;
                            ids.insert(index.0 as usize);
                        }
                    }
                }
                Ok(Arc::new(ids))
            },
            &|set| biome_mask(inputs, &owner, set),
        )
        .map_err(|error| format!("{registry}: {error}"))
    };
    frozen.variants = VariantTables {
        chickens: table("minecraft:chicken_variant", inputs.variants.chickens)?,
        chicken_sounds: inputs.variants.chicken_sounds.to_vec(),
        zombie_nautiluses: table(
            "minecraft:zombie_nautilus_variant",
            inputs.variants.zombie_nautiluses,
        )?,
        ..VariantTables::default()
    };
    Ok(())
}

fn biome_mask(
    inputs: &StructureInputs<'_>,
    owner: &ResourceLocation,
    set: &HolderSet,
) -> Result<BiomeMask, String> {
    let mut mask = FixedBitSet::with_capacity(inputs.biomes.len() as usize);
    match set {
        HolderSet::Tag(tag) => {
            let key = TagKey::<Biome, _>::from_location(tag.clone());
            let members = inputs.biome_tags.get(&key).ok_or_else(|| {
                format!("{owner}: names the biome tag #{tag}, which is not loaded")
            })?;
            for id in members.iter() {
                mask.insert(id as usize);
            }
        }
        HolderSet::One(_) | HolderSet::List(_) => {
            for id in set.entries() {
                let index = inputs
                    .biomes
                    .get(id.as_str())
                    .ok_or_else(|| format!("{owner}: names the biome {id}, which is not loaded"))?;
                mask.insert(index as usize);
            }
        }
    }
    Ok(Arc::new(mask))
}

fn biome_tag_mask(
    inputs: &StructureInputs<'_>,
    owner: &ResourceLocation,
    tag: &str,
) -> Result<BiomeMask, String> {
    let tag = ResourceLocation::parse(tag).expect("a literal id");
    biome_mask(inputs, owner, &HolderSet::Tag(tag))
}

fn freeze_pools(inputs: &StructureInputs<'_>, frozen: &mut FrozenStructures) -> Result<(), String> {
    frozen.pool_ids = inputs
        .pools
        .keys()
        .enumerate()
        .map(|(index, id)| (id.clone(), PoolId(index as u32)))
        .collect();
    for (id, pool) in inputs.pools {
        let fallback = *frozen.pool_ids.get(&pool.fallback).ok_or_else(|| {
            format!(
                "{id}: names the template pool {}, which is not loaded",
                pool.fallback
            )
        })?;
        let mut expanded = Vec::new();
        let mut max_size = 0;
        for entry in &pool.elements {
            let (element, span) = freeze_element(inputs, frozen, id, &entry.element, None)?;
            if let Some((min_y, max_y)) = span {
                max_size = max_size.max(max_y - min_y + 1);
            }
            expanded.extend(std::iter::repeat_n(element, entry.weight.0 as usize));
        }
        frozen.pools.push(FrozenPool {
            id: id.clone(),
            fallback,
            expanded,
            max_size,
        });
    }
    Ok(())
}

/// The element's id and, unless it is empty, the y extent of its bounding box
/// at the origin with no rotation. A list imposes its projection on every
/// member, however deep.
fn freeze_element(
    inputs: &StructureInputs<'_>,
    frozen: &mut FrozenStructures,
    pool: &ResourceLocation,
    element: &PoolElement,
    imposed: Option<Projection>,
) -> Result<(ElementId, Option<(i32, i32)>), String> {
    let (element, span) = match element {
        PoolElement::Single(single) | PoolElement::LegacySingle(single) => {
            let template = freeze_template(inputs, frozen, pool, &single.location)?;
            let size = frozen.manifests[template.0 as usize].size;
            let bounds = bounding_box(size, IVec3::ZERO, Rotation::None, Mirror::None, IVec3::ZERO);
            (
                FrozenElement::Single {
                    template,
                    legacy: matches!(element, PoolElement::LegacySingle(_)),
                    processors: single.processors.clone(),
                    projection: imposed.unwrap_or(single.projection),
                    liquid_settings: single.override_liquid_settings,
                },
                Some((bounds.min.y, bounds.max.y)),
            )
        }
        PoolElement::List {
            elements,
            projection,
        } => {
            let projection = imposed.unwrap_or(*projection);
            let mut ids = Vec::with_capacity(elements.len());
            let mut span: Option<(i32, i32)> = None;
            for inner in elements {
                let (id, inner_span) =
                    freeze_element(inputs, frozen, pool, inner, Some(projection))?;
                ids.push(id);
                if let Some((min_y, max_y)) = inner_span {
                    span = Some(match span {
                        Some((lo, hi)) => (lo.min(min_y), hi.max(max_y)),
                        None => (min_y, max_y),
                    });
                }
            }
            if span.is_none() {
                return Err(format!(
                    "{pool}: a list element has no bounding box because every component is empty"
                ));
            }
            (
                FrozenElement::List {
                    elements: ids,
                    projection,
                },
                span,
            )
        }
        PoolElement::Feature {
            feature,
            projection,
        } => (
            FrozenElement::Feature {
                feature: feature.clone(),
                projection: imposed.unwrap_or(*projection),
            },
            Some((0, 0)),
        ),
        PoolElement::Empty {} => (FrozenElement::Empty, None),
    };
    frozen.elements.push(element);
    Ok((ElementId(frozen.elements.len() as u32 - 1), span))
}

fn freeze_template(
    inputs: &StructureInputs<'_>,
    frozen: &mut FrozenStructures,
    pool: &ResourceLocation,
    location: &ResourceLocation,
) -> Result<TemplateId, String> {
    if let Some(id) = frozen.template_ids.get(location) {
        return Ok(*id);
    }
    match (inputs.template)(location) {
        Some(template) => freeze_loaded_template(inputs, frozen, location, &template),
        None => {
            tracing::warn!(%pool, %location, "the template is not loaded; the element places nothing");
            Ok(push_template(
                frozen,
                location,
                FrozenTemplate::empty(),
                TemplateManifest::empty(),
            ))
        }
    }
}

fn freeze_structure_templates(
    inputs: &StructureInputs<'_>,
    frozen: &mut FrozenStructures,
    structure: &ResourceLocation,
    paths: &[&str],
) -> Result<(), String> {
    for path in paths {
        let location = ResourceLocation::minecraft(path);
        if frozen.template_ids.contains_key(&location) {
            continue;
        }
        let template = (inputs.template)(&location).ok_or_else(|| {
            format!("{structure}: names the template {location}, which is not loaded")
        })?;
        freeze_loaded_template(inputs, frozen, &location, &template)?;
    }
    Ok(())
}

fn freeze_loaded_template(
    inputs: &StructureInputs<'_>,
    frozen: &mut FrozenStructures,
    location: &ResourceLocation,
    template: &Template,
) -> Result<TemplateId, String> {
    let (template, manifest) = template
        .freeze(location, inputs.resolve)
        .map_err(|error| error.to_string())?;
    check_block_entity_ids(location, &template)?;
    Ok(push_template(frozen, location, template, manifest))
}

fn push_template(
    frozen: &mut FrozenStructures,
    location: &ResourceLocation,
    template: FrozenTemplate,
    manifest: TemplateManifest,
) -> TemplateId {
    let id = TemplateId(frozen.templates.len() as u32);
    frozen.templates.push(Arc::new(template));
    frozen.manifests.push(Arc::new(manifest));
    frozen.template_ids.insert(location.clone(), id);
    id
}

pub(crate) fn check_block_entity_ids(
    location: &ResourceLocation,
    template: &FrozenTemplate,
) -> Result<(), String> {
    for block in template.palettes.iter().flat_map(|palette| palette.iter()) {
        let Some(nbt) = &block.nbt else { continue };
        let id = nbt.get_string("id");
        if id.is_some_and(|id| {
            GeneratedBlockEntity::IDS.contains(&id)
                || matches!(id, "minecraft:jigsaw" | "minecraft:structure_block")
        }) {
            continue;
        }
        return Err(format!(
            "{location}: block entity at {:?} has id {id:?}, which is not modelled",
            block.pos.map(i32::from)
        ));
    }
    Ok(())
}

fn path_then_namespace(a: &ResourceLocation, b: &ResourceLocation) -> Ordering {
    a.path()
        .cmp(b.path())
        .then_with(|| a.namespace().cmp(b.namespace()))
}

fn freeze_structures(
    inputs: &StructureInputs<'_>,
    frozen: &mut FrozenStructures,
) -> Result<(), String> {
    let mut ordered: Vec<(&ResourceLocation, &Structure)> = inputs.structures.iter().collect();
    ordered.sort_by(|a, b| path_then_namespace(a.0, b.0));
    let mut per_step: BTreeMap<DecorationStep, u32> = BTreeMap::new();
    for (id, structure) in ordered {
        let settings = structure.settings();
        let step_index = per_step.entry(settings.step).or_default();
        let biomes = biome_mask(inputs, id, &settings.biomes)?;
        freeze_structure_templates(inputs, frozen, id, structure.templates())?;
        let templates = |names: &[&str]| {
            names
                .iter()
                .map(|name| frozen.template_ids[&ResourceLocation::minecraft(name)])
                .collect::<Vec<TemplateId>>()
        };
        let kind = match structure {
            Structure::Jigsaw { jigsaw, .. } => {
                let start_pool = *frozen.pool_ids.get(&jigsaw.start_pool).ok_or_else(|| {
                    format!(
                        "{id}: names the template pool {}, which is not loaded",
                        jigsaw.start_pool
                    )
                })?;
                let margin = if settings.terrain_adaptation == TerrainAdaptation::None {
                    0
                } else {
                    TERRAIN_MARGIN
                };
                let range = jigsaw.max_distance_from_center.horizontal() + margin;
                if range > MAX_JIGSAW_RANGE {
                    return Err(format!(
                        "{id}: max_distance_from_center {} plus the terrain adaptation margin {margin} exceeds {MAX_JIGSAW_RANGE}",
                        jigsaw.max_distance_from_center.horizontal()
                    ));
                }
                let mut aliases = BTreeSet::new();
                check_aliases(id, &jigsaw.pool_aliases, &frozen.pool_ids, &mut aliases)?;
                check_jigsaw_targets(id, start_pool, &aliases, frozen)?;
                StructureKind::Jigsaw {
                    start_pool,
                    config: jigsaw.clone(),
                }
            }
            Structure::BuriedTreasure { .. } => StructureKind::BuriedTreasure,
            Structure::DesertPyramid { .. } => StructureKind::DesertPyramid,
            Structure::EndCity { .. } => StructureKind::EndCity,
            Structure::Fortress { .. } => StructureKind::Fortress,
            Structure::Igloo { .. } => StructureKind::Igloo,
            Structure::JungleTemple { .. } => StructureKind::JungleTemple,
            Structure::Mineshaft { mineshaft_type, .. } => StructureKind::Mineshaft {
                mineshaft_type: *mineshaft_type,
                blocking: biome_tag_mask(inputs, id, "minecraft:mineshaft_blocking")?,
            },
            Structure::NetherFossil { height, .. } => {
                StructureKind::NetherFossil { height: *height }
            }
            Structure::OceanMonument { .. } => StructureKind::OceanMonument {
                surrounding: biome_tag_mask(
                    inputs,
                    id,
                    "minecraft:required_ocean_monument_surrounding",
                )?,
            },
            Structure::OceanRuin {
                biome_temp,
                large_probability,
                cluster_probability,
                ..
            } => StructureKind::OceanRuin(OceanRuinConfig {
                biome_temp: *biome_temp,
                large_probability: large_probability.0 as f32,
                cluster_probability: cluster_probability.0 as f32,
                frequent_drowned: biome_tag_mask(
                    inputs,
                    id,
                    "minecraft:more_frequent_drowned_spawns",
                )?,
            }),
            Structure::RuinedPortal { setups, .. } => {
                let portals = templates(ruined_portal::PORTALS);
                let giant_portals = templates(ruined_portal::GIANT_PORTALS);
                StructureKind::RuinedPortal {
                    setups: setups.clone(),
                    portals,
                    giant_portals,
                }
            }
            Structure::Shipwreck { is_beached, .. } => StructureKind::Shipwreck {
                is_beached: *is_beached,
                templates: templates(if *is_beached {
                    shipwreck::BEACHED
                } else {
                    shipwreck::OCEAN
                }),
            },
            Structure::Stronghold { .. } => StructureKind::Stronghold,
            Structure::SwampHut { .. } => StructureKind::SwampHut,
            Structure::WoodlandMansion { .. } => StructureKind::WoodlandMansion,
        };
        if site_implies_piece(&kind).is_none() {
            tracing::warn!(structure = %id, "no generator for this structure type; it places nothing");
        }
        frozen
            .structure_ids
            .insert(id.clone(), StructureId(frozen.structures.len() as u32));
        frozen.structures.push(FrozenStructure {
            id: id.clone(),
            step: settings.step,
            step_index: *step_index,
            adaptation: settings.terrain_adaptation,
            biomes,
            kind,
        });
        *step_index += 1;
    }
    Ok(())
}

fn check_aliases(
    structure: &ResourceLocation,
    aliases: &[PoolAlias],
    pools: &BTreeMap<ResourceLocation, PoolId>,
    seen: &mut BTreeSet<ResourceLocation>,
) -> Result<(), String> {
    let claim = |seen: &mut BTreeSet<ResourceLocation>, alias: &ResourceLocation| {
        if seen.insert(alias.clone()) {
            Ok(())
        } else {
            Err(format!(
                "{structure}: the pool alias {alias} is bound twice"
            ))
        }
    };
    let target = |target: &ResourceLocation| {
        if pools.contains_key(target) {
            Ok(())
        } else {
            Err(format!(
                "{structure}: names the template pool {target}, which is not loaded"
            ))
        }
    };
    for binding in aliases {
        match binding {
            PoolAlias::Direct { alias, target: t } => {
                claim(seen, alias)?;
                target(t)?;
            }
            PoolAlias::Random { alias, targets } => {
                claim(seen, alias)?;
                for weighted in targets {
                    target(&weighted.data)?;
                }
            }
            // One group is drawn per resolution, so the groups are alternatives:
            // each must be free of repeats on its own, and any of them may follow.
            PoolAlias::RandomGroup { groups } => {
                let before = seen.clone();
                for group in groups {
                    let mut chosen = before.clone();
                    check_aliases(structure, &group.data, pools, &mut chosen)?;
                    seen.extend(chosen);
                }
            }
        }
    }
    Ok(())
}

/// Every pool a jigsaw can reach from the start pool must name, in each of its
/// templates' jigsaw blocks, either a loaded pool or an alias of this structure.
fn check_jigsaw_targets(
    structure: &ResourceLocation,
    start: PoolId,
    aliases: &BTreeSet<ResourceLocation>,
    frozen: &FrozenStructures,
) -> Result<(), String> {
    let mut visited = BTreeSet::new();
    let mut queue = vec![start];
    let mut elements = Vec::new();
    while let Some(pool) = queue.pop() {
        if !visited.insert(pool) {
            continue;
        }
        let pool = &frozen.pools[pool.0 as usize];
        queue.push(pool.fallback);
        elements.clear();
        elements.extend(pool.expanded.iter().copied());
        while let Some(element) = elements.pop() {
            let template = match &frozen.elements[element.0 as usize] {
                FrozenElement::Single { template, .. } => *template,
                FrozenElement::List {
                    elements: inner, ..
                } => {
                    elements.extend(inner.iter().copied());
                    continue;
                }
                FrozenElement::Feature { .. } | FrozenElement::Empty => continue,
            };
            for jigsaw in frozen.manifests[template.0 as usize]
                .jigsaws
                .iter()
                .flatten()
            {
                if aliases.contains(&jigsaw.pool) {
                    continue;
                }
                match frozen.pool_ids.get(&jigsaw.pool) {
                    Some(target) => queue.push(*target),
                    None => {
                        let location = frozen
                            .template_ids
                            .iter()
                            .find(|(_, id)| **id == template)
                            .map(|(location, _)| location.to_string())
                            .unwrap_or_default();
                        return Err(format!(
                            "{structure}: the jigsaw at {:?} in {location} names the template pool {}, which is neither loaded nor aliased",
                            jigsaw.pos.map(i32::from),
                            jigsaw.pool
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn freeze_sets(inputs: &StructureInputs<'_>, frozen: &mut FrozenStructures) -> Result<(), String> {
    frozen.set_ids = inputs
        .sets
        .keys()
        .enumerate()
        .map(|(index, id)| (id.clone(), SetId(index as u32)))
        .collect();
    for (id, set) in inputs.sets {
        let mut entries = Vec::with_capacity(set.structures.len());
        for entry in &set.structures {
            let structure = *frozen.structure_ids.get(&entry.structure).ok_or_else(|| {
                format!(
                    "{id}: names the structure {}, which is not loaded",
                    entry.structure
                )
            })?;
            entries.push((structure, entry.weight.0));
        }
        let (spreading, preferred_biomes) = match &set.placement {
            StructurePlacement::RandomSpread {
                spreading,
                spacing,
                separation,
                ..
            } => {
                if spacing.0 <= separation.0 {
                    return Err(format!(
                        "{id}: spacing {} is not larger than separation {}",
                        spacing.0, separation.0
                    ));
                }
                (Some(spreading), None)
            }
            StructurePlacement::ConcentricRings {
                spreading,
                preferred_biomes,
                ..
            } => (
                Some(spreading),
                Some(biome_mask(inputs, id, preferred_biomes)?),
            ),
            StructurePlacement::DimensionOrigin {} => (None, None),
        };
        let exclusion = match spreading.and_then(|spreading| spreading.exclusion_zone.as_ref()) {
            Some(zone) => {
                let other = *frozen.set_ids.get(&zone.other_set).ok_or_else(|| {
                    format!(
                        "{id}: names the structure set {}, which is not loaded",
                        zone.other_set
                    )
                })?;
                Some((other, zone.chunk_count.0))
            }
            None => None,
        };
        frozen.sets.push(FrozenSet {
            id: id.clone(),
            placement: set.placement.clone(),
            exclusion,
            preferred_biomes,
            entries,
        });
    }
    for start in 0..frozen.sets.len() {
        let mut chain = vec![start];
        let mut at = start;
        while let Some((next, _)) = frozen.sets[at].exclusion {
            at = next.0 as usize;
            if chain.contains(&at) {
                let names: Vec<String> = chain
                    .iter()
                    .chain([&at])
                    .map(|index| frozen.sets[*index].id.to_string())
                    .collect();
                return Err(format!(
                    "{}: the exclusion zones cycle: {}",
                    frozen.sets[start].id,
                    names.join(" -> ")
                ));
            }
            chain.push(at);
        }
    }
    Ok(())
}

/// The sets a dimension whose source can answer `biomes` places, each with
/// the structures of it that can land there.
pub fn live_sets(
    frozen: &FrozenStructures,
    biomes: &FixedBitSet,
) -> Vec<(SetId, Vec<StructureId>)> {
    frozen
        .sets
        .iter()
        .enumerate()
        .filter_map(|(index, set)| {
            let candidates: Vec<StructureId> = set
                .entries
                .iter()
                .map(|(structure, _)| *structure)
                .filter(|structure| {
                    !frozen.structures[structure.0 as usize]
                        .biomes
                        .is_disjoint(biomes)
                })
                .collect();
            (!candidates.is_empty()).then_some((SetId(index as u32), candidates))
        })
        .collect()
}
