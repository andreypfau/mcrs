use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::schema::PlacementFilter;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::voxel_shape::{
    FACE_MASK_EMPTY, FACE_MASK_FULL, FACE_RESOLUTION, FaceMask, VoxelShape,
};
use mcrs_minecraft_decoration::feature::tree::decorator::{CompiledTreeDecorator, TreePalette};
use mcrs_minecraft_decoration::feature::tree::foliage::Foliage;
use mcrs_minecraft_decoration::feature::tree::provider::{
    SharedNoise, StateProvider, int_property_table, rotation_table,
};
use mcrs_minecraft_decoration::feature::tree::root::{AboveRootPlacement, MangroveRoots};
use mcrs_minecraft_decoration::feature::tree::survive::{
    CANNOT_SUPPORT_SEAGRASS, OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT, SUPPORTS_CACTUS,
    SUPPORTS_LILY_PAD, SUPPORTS_SMALL_DRIPLEAF, SUPPORTS_SUGAR_CANE,
    SUPPORTS_SUGAR_CANE_ADJACENTLY, SUPPORTS_VEGETATION, SurviveFamily, SurviveRule,
    UNSTABLE_BOTTOM_CENTER, family_of,
};
use mcrs_minecraft_decoration::feature::tree::trunk::{TreeStates, Trunk};
use mcrs_minecraft_decoration::feature::tree::{CompiledTree, LeafDistances, TreeTables};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::compile::{
    FeatureCompileError, StateQuery, compile_predicate,
};
use mcrs_minecraft_worldgen::feature::placer::StateMask;
use mcrs_minecraft_worldgen::feature::tree::{
    BlockStateProvider, RootPlacer as ProtoRootPlacer, TreeConfig, TreeDecorator as ProtoDecorator,
    TrunkPlacer as ProtoTrunk,
};
use mcrs_minecraft_worldgen::noise::normal as normal_noise;

use super::feature_program::{Resolver, missing, union_masks};

type Compiled<T> = Result<T, FeatureCompileError>;

fn empty_mask(blocks: &BlockDefinitions) -> FixedBitSet {
    FixedBitSet::with_capacity(blocks.state_count())
}

/// Every state of every block that declares `property`, and what each declares
/// for it.
pub(super) fn with_property<'a>(
    blocks: &'a BlockDefinitions,
    property: &'a str,
) -> impl Iterator<Item = (VoxelId, &'a PropertyValue)> + 'a {
    blocks.blocks().iter().flat_map(move |entry| {
        (0..entry.state_count).filter_map(move |offset| {
            let id = mcrs_minecraft_protocol::BlockStateId(entry.base_state_id.0 + offset);
            let value = entry.value_of(id, property)?;
            Some((VoxelId::from(id.0), value))
        })
    })
}

pub(super) fn state_of(
    blocks: &BlockDefinitions,
    block: &str,
    properties: &[(&str, &str)],
) -> Option<VoxelId> {
    let entry = blocks.block(block)?;
    let mut id = entry.default_state_id;
    for (name, value) in properties {
        id = entry.with_text(id, name, value)?;
    }
    Some(VoxelId::from(id.0))
}

pub fn build_tree_tables(resolver: &Resolver<'_>) -> Compiled<TreeTables> {
    let blocks = resolver.blocks;
    let air = resolver.block_mask("minecraft:air")?;
    let logs = resolver.tag_mask("minecraft:logs")?;
    let leaves = resolver.tag_mask("minecraft:leaves")?;
    let replaceable_by_trees = resolver.tag_mask("minecraft:replaceable_by_trees")?;

    let states = TreeStates {
        valid_tree_pos: union_masks(&[&air, &replaceable_by_trees]),
        logs: logs.clone(),
        air_or_leaves: union_masks(&[&air, &leaves]),
        persistent: boolean_mask(blocks, "persistent", true),
        axis: property_table(blocks, "axis", ["x", "y", "z"]),
        waterlogged: property_table(blocks, "waterlogged", ["false", "true"]),
    };

    let vine_side = std::array::from_fn(|index| {
        state_of(
            blocks,
            "minecraft:vine",
            &[(Direction::HORIZONTAL[index].name(), "true")],
        )
        .unwrap_or_default()
    });
    let state = |block: &str, properties: &[(&str, &str)]| {
        missing(state_of(blocks, block, properties), block)
    };
    let palette = TreePalette {
        vines: resolver.block_mask("minecraft:vine")?,
        shelf_mushrooms: resolver.block_mask("minecraft:shelf_mushroom")?,
        vine_side,
        bee_nest: state("minecraft:bee_nest", &[("facing", "south")])?,
        cocoa: aged_facings(blocks, "minecraft:cocoa"),
        shelf_mushroom: aged_facings(blocks, "minecraft:shelf_mushroom"),
        pale_hanging_moss: [
            state("minecraft:pale_hanging_moss", &[("tip", "false")])?,
            state("minecraft:pale_hanging_moss", &[("tip", "true")])?,
        ],
        creaking_heart: state("minecraft:creaking_heart", &[])?,
    };

    Ok(TreeTables {
        states,
        palette,
        leaf_distance: leaf_distances(blocks, resolver)?,
        survive: survive_rules(resolver)?,
    })
}

fn aged_facings<const AGES: usize>(blocks: &BlockDefinitions, block: &str) -> [[VoxelId; 4]; AGES] {
    std::array::from_fn(|age| {
        std::array::from_fn(|facing| {
            state_of(
                blocks,
                block,
                &[
                    ("age", &age.to_string()),
                    ("facing", Direction::HORIZONTAL[facing].name()),
                ],
            )
            .unwrap_or_default()
        })
    })
}

fn boolean_mask(blocks: &BlockDefinitions, property: &str, wanted: bool) -> StateMask {
    let mut mask = empty_mask(blocks);
    for (state, value) in with_property(blocks, property) {
        if *value == PropertyValue::Bool(wanted) {
            mask.insert(state.0 as usize);
        }
    }
    Arc::new(mask)
}

/// For every state of every block declaring `property`, the state with each of
/// `values` set; a value the block refuses leaves the state as it is.
fn property_table<const N: usize>(
    blocks: &BlockDefinitions,
    property: &str,
    values: [&str; N],
) -> HashMap<u16, [VoxelId; N]> {
    with_property(blocks, property)
        .map(|(state, _)| {
            let id = mcrs_minecraft_protocol::BlockStateId(state.0);
            let entry = blocks.owner(id);
            (
                state.0,
                values.map(|value| {
                    entry
                        .with_text(id, property, value)
                        .map_or(state, |set| VoxelId::from(set.0))
                }),
            )
        })
        .collect()
}

/// `LeavesBlock.getOptionalDistanceAt`, plus the state each distance writes
/// back: the tag answers 0 and is never rewritten, a `distance` answers itself.
fn leaf_distances(blocks: &BlockDefinitions, resolver: &Resolver<'_>) -> Compiled<LeafDistances> {
    let mut table = LeafDistances::default();
    let by_distance = property_table(blocks, "distance", ["1", "2", "3", "4", "5", "6", "7"]);
    for (state, value) in with_property(blocks, "distance") {
        if let PropertyValue::Int(distance) = value {
            table.insert(state, *distance as u8, by_distance[&state.0]);
        }
    }
    for state in resolver
        .tag_mask("minecraft:prevents_nearby_leaf_decay")?
        .ones()
    {
        let state = VoxelId(state as u16);
        table.insert(state, 0, [state; 7]);
    }
    Ok(table)
}

/// Which faces of a state's collision shape can hold something up, as the
/// reference's `SupportType` reads them.
struct FaceSupport {
    /// `SupportType.FULL` upwards.
    sturdy_up: FixedBitSet,
    /// A non-empty upward face, which is all a sea pickle asks of its floor.
    any_up: FixedBitSet,
    /// `SupportType.CENTER` downwards, which is what hangs a spore blossom.
    center_down: FixedBitSet,
}

/// `SupportType.CENTER`: the middle 2/16 of the face has to be covered.
fn covers_center(mask: &FaceMask) -> bool {
    const LO: usize = FACE_RESOLUTION * 7 / 16;
    const HI: usize = FACE_RESOLUTION * 9 / 16;
    (LO..HI).all(|v| {
        (LO..HI).all(|u| {
            let bit = v * FACE_RESOLUTION + u;
            mask[bit >> 6] >> (bit & 63) & 1 == 1
        })
    })
}

fn face_support(blocks: &BlockDefinitions) -> FaceSupport {
    let mut answers: HashMap<u32, [bool; 3]> = HashMap::default();
    let mut out = FaceSupport {
        sturdy_up: empty_mask(blocks),
        any_up: empty_mask(blocks),
        center_down: empty_mask(blocks),
    };
    for id in 0..blocks.state_count() {
        let shape = blocks
            .state(mcrs_minecraft_protocol::BlockStateId(id as u16))
            .collision_shape;
        let answer = *answers.entry(shape.0).or_insert_with(|| {
            let voxels = VoxelShape::from_boxes(blocks.shape(shape));
            let up = voxels.face_mask(mcrs_minecraft_core::Direction::Up);
            [
                *up == FACE_MASK_FULL,
                *up != FACE_MASK_EMPTY,
                covers_center(voxels.face_mask(mcrs_minecraft_core::Direction::Down)),
            ]
        });
        for (holds, mask) in
            answer
                .iter()
                .zip([&mut out.sturdy_up, &mut out.any_up, &mut out.center_down])
        {
            if *holds {
                mask.insert(id);
            }
        }
    }
    out
}

fn survive_rules(resolver: &Resolver<'_>) -> Compiled<HashMap<u32, SurviveRule>> {
    let blocks = resolver.blocks;
    let mut rules = HashMap::default();
    let faces = face_support(blocks);
    let water = resolver.world.water_fluid.as_ref().clone();
    let not_air = {
        let mut mask = empty_mask(blocks);
        mask.insert_range(..blocks.state_count());
        mask.difference_with(&resolver.world.air_states);
        mask
    };
    for (index, entry) in blocks.blocks().iter().enumerate() {
        if let Some(filter) = &entry.placement_filter {
            let rule = placement_rule(resolver, filter).map_err(|e| e.within(&entry.identifier))?;
            rules.insert(index as u32, rule);
            continue;
        }
        let Some(family) = family_of(entry.identifier.as_str()) else {
            continue;
        };
        let rule = match rule_of(resolver, family, &faces, &water, &not_air) {
            Ok(rule) => rule,
            // A shape this build cannot answer — a fluid tag, say — leaves the
            // block with no rule, so a placement testing it fails the compile
            // by name rather than placing on a guess.
            Err(error) if error.is_unsupported() => {
                tracing::warn!(block = entry.identifier.as_str(), %error, "no canSurvive rule");
                continue;
            }
            Err(error) => return Err(error.within(&entry.identifier)),
        };
        rules.insert(index as u32, rule);
    }
    Ok(rules)
}

/// One condition against one vertical face is the only shape the placement
/// filter is read in; anything else is refused at load rather than approximated.
fn placement_rule(resolver: &Resolver<'_>, filter: &PlacementFilter) -> Compiled<SurviveRule> {
    let unsupported = |what: &str| {
        FeatureCompileError::Unsupported(format!("minecraft:placement_filter: {what}"))
    };
    let [condition] = filter.conditions.as_slice() else {
        return Err(unsupported("exactly one condition is read"));
    };
    let offset_y = match condition.allowed_faces.as_slice() {
        [Direction::Up] => -1,
        [Direction::Down] => 1,
        _ => return Err(unsupported("allowed_faces must be [\"up\"] or [\"down\"]")),
    };
    let mut supports = empty_mask(resolver.blocks);
    for entry in &condition.block_filter {
        let query = if entry.is_tag {
            StateQuery::BlockTag(&entry.loc)
        } else {
            StateQuery::Block(&entry.loc)
        };
        supports.union_with(resolver.mask(query)?.as_ref());
    }
    Ok(SurviveRule::SupportedBy { offset_y, supports })
}

fn rule_of(
    resolver: &Resolver<'_>,
    family: SurviveFamily,
    faces: &FaceSupport,
    water: &FixedBitSet,
    not_air: &FixedBitSet,
) -> Compiled<SurviveRule> {
    let tag = |name: &str| resolver.tag_mask(name).map(|mask| mask.as_ref().clone());
    let below = |supports: FixedBitSet| SurviveRule::SupportedBy {
        offset_y: -1,
        supports,
    };
    Ok(match family {
        SurviveFamily::Mushroom => {
            let mut supports = tag(OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)?;
            supports.union_with(&resolver.world.solid_render);
            below(supports)
        }
        SurviveFamily::OnAnythingBelow => below(not_air.clone()),
        SurviveFamily::OnSturdyFaceBelow => below(faces.sturdy_up.clone()),
        SurviveFamily::SeaPickle => below(faces.any_up.clone()),
        SurviveFamily::Seagrass => {
            let mut supports = faces.sturdy_up.clone();
            supports.difference_with(&tag(CANNOT_SUPPORT_SEAGRASS)?);
            below(supports)
        }
        // ponytail: `isFull` is not checked; worldgen water is source water.
        SurviveFamily::TallSeagrass => {
            let mut supports = faces.sturdy_up.clone();
            supports.difference_with(&tag(CANNOT_SUPPORT_SEAGRASS)?);
            let mut not_water = not_air.clone();
            not_water.union_with(&resolver.world.air_states);
            not_water.difference_with(water);
            SurviveRule::SupportedByUnless {
                offset_y: -1,
                supports,
                blocked: not_water,
            }
        }
        SurviveFamily::SmallDripleaf => SurviveRule::SmallDripleaf {
            supports: tag(SUPPORTS_SMALL_DRIPLEAF)?,
            wet_supports: tag(SUPPORTS_VEGETATION)?,
            water: water.clone(),
        },
        SurviveFamily::SporeBlossom => {
            let mut supports = faces.center_down.clone();
            supports.difference_with(&tag(UNSTABLE_BOTTOM_CENTER)?);
            SurviveRule::SupportedByUnless {
                offset_y: 1,
                supports,
                blocked: water.clone(),
            }
        }
        SurviveFamily::LilyPad => {
            let mut supports = tag(SUPPORTS_LILY_PAD)?;
            let fluids = resolver.mask(StateQuery::Fluids(&HolderSet::Tag(
                ResourceLocation::parse(SUPPORTS_LILY_PAD).expect("a literal id"),
            )))?;
            supports.union_with(&fluids);
            SurviveRule::SupportedByUnless {
                offset_y: -1,
                supports,
                blocked: resolver.world.any_fluid.as_ref().clone(),
            }
        }
        SurviveFamily::SugarCane => SurviveRule::SugarCane {
            sugar_cane: resolver
                .block_mask("minecraft:sugar_cane")?
                .as_ref()
                .clone(),
            supports: tag(SUPPORTS_SUGAR_CANE)?,
            adjacent: {
                let mut adjacent = tag(SUPPORTS_SUGAR_CANE_ADJACENTLY)?;
                adjacent.union_with(&resolver.world.water_fluid);
                adjacent
            },
        },
        SurviveFamily::Cactus => {
            let mut blocked = resolver.world.sturdy_up.as_ref().clone();
            blocked.union_with(&resolver.world.lava_fluid);
            let mut supports = tag(SUPPORTS_CACTUS)?;
            let cactus = resolver.block_mask("minecraft:cactus")?;
            supports.union_with(&cactus);
            SurviveRule::Cactus {
                supports,
                blocked,
                liquid: {
                    let mut liquid = resolver.world.water_fluid.as_ref().clone();
                    liquid.union_with(&resolver.world.lava_fluid);
                    liquid
                },
            }
        }
    })
}

pub fn compile_tree(
    config: &TreeConfig,
    tables: &Arc<TreeTables>,
    r: &Resolver<'_>,
) -> Compiled<CompiledTree> {
    let decorators = config
        .decorators
        .iter()
        .map(|decorator| compile_decorator(decorator, r))
        .collect::<Compiled<Vec<_>>>()?;
    Ok(CompiledTree {
        trunk_provider: compile_provider(&config.trunk_provider, r)?,
        foliage_provider: compile_provider(&config.foliage_provider, r)?,
        below_trunk_provider: compile_provider(&config.below_trunk_provider, r)?,
        trunk: compile_trunk(&config.trunk_placer, r)?,
        foliage: Foliage(config.foliage_placer.clone()),
        roots: config
            .root_placer
            .as_ref()
            .map(|placer| compile_root_placer(placer, r))
            .transpose()?,
        minimum_size: config.minimum_size.clone(),
        decorators,
        ignore_vines: config.ignore_vines,
        tables: tables.clone(),
    })
}

pub(super) fn compile_provider(
    provider: &BlockStateProvider,
    r: &Resolver<'_>,
) -> Compiled<StateProvider> {
    Ok(match provider {
        BlockStateProvider::Simple { state } => StateProvider::Simple(r.resolve(state)?),
        BlockStateProvider::Weighted { entries } => StateProvider::Weighted(
            entries
                .iter()
                .map(|entry| Ok((r.resolve(&entry.data)?, entry.weight.0)))
                .collect::<Compiled<Vec<_>>>()?,
        ),
        BlockStateProvider::RuleBased { fallback, rules } => StateProvider::RuleBased {
            fallback: fallback
                .as_ref()
                .map(|provider| compile_provider(provider, r).map(Box::new))
                .transpose()?,
            rules: rules
                .iter()
                .map(|rule| {
                    Ok((
                        compile_predicate(&rule.if_true, r)?,
                        compile_provider(&rule.then, r)?,
                    ))
                })
                .collect::<Compiled<Vec<_>>>()?,
        },
        BlockStateProvider::RandomizedInt {
            source,
            property,
            values,
        } => StateProvider::RandomizedInt {
            source: Box::new(compile_provider(source, r)?),
            property: Arc::new(int_property_table(&r.world.layouts, property)),
            values: values.clone(),
        },
        BlockStateProvider::Rotated { state, direction } => StateProvider::Rotated {
            source: Box::new(compile_provider(state, r)?),
            direction: *direction,
            rotations: Arc::new(rotation_table(&r.world.layouts)),
        },
        BlockStateProvider::RandomBlock { blocks } => {
            StateProvider::RandomBlock(r.block_set_defaults(blocks)?)
        }
        BlockStateProvider::CopyProperties { source } => {
            StateProvider::CopyProperties(Box::new(compile_provider(source, r)?))
        }
        BlockStateProvider::Noise {
            seed,
            noise,
            scale,
            states,
        } => StateProvider::Noise {
            noise: sampler(*seed, noise),
            scale: scale.0 as f32,
            states: block_states_of(states, r)?,
        },
        BlockStateProvider::NoiseThreshold {
            seed,
            noise,
            scale,
            threshold,
            high_chance,
            default_state,
            low_states,
            high_states,
        } => StateProvider::NoiseThreshold {
            noise: sampler(*seed, noise),
            scale: scale.0 as f32,
            threshold: threshold.0 as f32,
            high_chance: high_chance.0 as f32,
            default_state: r.resolve(default_state)?,
            low_states: block_states_of(low_states, r)?,
            high_states: block_states_of(high_states, r)?,
        },
        BlockStateProvider::DualNoise {
            variety,
            slow_noise,
            slow_scale,
            seed,
            noise,
            scale,
            states,
        } => StateProvider::DualNoise {
            slow_noise: sampler(*seed, slow_noise),
            slow_scale: slow_scale.0 as f32,
            variety_min: variety.min_inclusive(),
            variety_max: variety.max_inclusive(),
            noise: sampler(*seed, noise),
            scale: scale.0 as f32,
            states: block_states_of(states, r)?,
        },
    })
}

fn compile_root_placer(placer: &ProtoRootPlacer, r: &Resolver<'_>) -> Compiled<MangroveRoots> {
    let ProtoRootPlacer::Mangrove {
        trunk_offset_y,
        root_provider,
        above_root_placement,
        mangrove_root_placement,
    } = placer;
    Ok(MangroveRoots {
        trunk_offset_y: trunk_offset_y.clone(),
        root_provider: compile_provider(root_provider, r)?,
        above_root_placement: above_root_placement
            .as_ref()
            .map(|above| {
                Ok(AboveRootPlacement {
                    provider: compile_provider(&above.above_root_provider, r)?,
                    chance: above.above_root_placement_chance.0 as f32,
                })
            })
            .transpose()?,
        can_grow_through: r.mask(StateQuery::Blocks(
            &mangrove_root_placement.can_grow_through,
        ))?,
        muddy_roots_in: r.mask(StateQuery::Blocks(&mangrove_root_placement.muddy_roots_in))?,
        muddy_roots_provider: compile_provider(&mangrove_root_placement.muddy_roots_provider, r)?,
        max_root_width: mangrove_root_placement.max_root_width.0,
        max_root_length: mangrove_root_placement.max_root_length.0,
        random_skew_chance: mangrove_root_placement.random_skew_chance.0 as f32,
    })
}

fn block_states_of(
    states: &[mcrs_minecraft_worldgen::proto::BlockState],
    r: &Resolver<'_>,
) -> Compiled<Vec<VoxelId>> {
    states.iter().map(|state| r.resolve(state)).collect()
}

/// `NoiseBasedStateProvider`'s constructor: one legacy source per noise field,
/// each reseeded from the provider's own `seed` rather than the world's.
fn sampler(seed: i64, noise: &mcrs_minecraft_worldgen::proto::NoiseParam) -> SharedNoise {
    let mut random = LegacyRandom::new(seed as u64);
    Arc::new(normal_noise::create(noise, &mut random))
}

fn compile_trunk(trunk: &ProtoTrunk, r: &Resolver<'_>) -> Compiled<Trunk> {
    let grow_through = match trunk {
        ProtoTrunk::UpwardsBranching {
            can_grow_through, ..
        } => Some(r.mask(StateQuery::Blocks(can_grow_through))?),
        _ => None,
    };
    Ok(Trunk {
        placer: trunk.clone(),
        grow_through,
    })
}

pub(super) fn compile_decorator(
    decorator: &ProtoDecorator,
    r: &Resolver<'_>,
) -> Compiled<CompiledTreeDecorator> {
    Ok(CompiledTreeDecorator(
        decorator.map_provider(|provider| compile_provider(provider, r))?,
    ))
}
