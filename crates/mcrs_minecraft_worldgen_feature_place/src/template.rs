use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::mth::clamped_map;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_core::{Axis, BlockPos, BoundingBox, Direction, dist_manhattan};
use mcrs_minecraft_core::{Mirror, Rotation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_random::{Random, block_pos_seed, shuffled};
use mcrs_minecraft_worldgen_feature::compile::{
    BlockResolver, FeatureCompileError, StateQuery, compile_rule, state_of, states_of,
};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::{
    BlockLayout, Rule, StateMask, WorldGenVolume, WorldStates,
};
use mcrs_minecraft_worldgen_feature::proto::{
    LinearPos, PosRuleTest, ProcessorRule, RuleBlockEntityModifier, StructureProcessor,
};
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_minecraft_worldgen_feature::template::{FrozenTemplate, JigsawBlock, transform};

use crate::block_entity::GeneratedBlockEntity;

fn direction_named(name: &str) -> Option<Direction> {
    Direction::all().into_iter().find(|d| d.name() == name)
}

fn rail_shape(shape: &str, rotation: Rotation) -> Option<&'static str> {
    Some(match (rotation, shape) {
        (Rotation::Clockwise90, "north_south") | (Rotation::Counterclockwise90, "north_south") => {
            "east_west"
        }
        (Rotation::Clockwise90, "east_west") | (Rotation::Counterclockwise90, "east_west") => {
            "north_south"
        }
        (Rotation::Clockwise90, "ascending_east") => "ascending_south",
        (Rotation::Clockwise90, "ascending_west") => "ascending_north",
        (Rotation::Clockwise90, "ascending_north") => "ascending_east",
        (Rotation::Clockwise90, "ascending_south") => "ascending_west",
        (Rotation::Clockwise90, "south_east") => "south_west",
        (Rotation::Clockwise90, "south_west") => "north_west",
        (Rotation::Clockwise90, "north_west") => "north_east",
        (Rotation::Clockwise90, "north_east") => "south_east",
        (Rotation::Clockwise180, "ascending_east") => "ascending_west",
        (Rotation::Clockwise180, "ascending_west") => "ascending_east",
        (Rotation::Clockwise180, "ascending_north") => "ascending_south",
        (Rotation::Clockwise180, "ascending_south") => "ascending_north",
        (Rotation::Clockwise180, "south_east") => "north_west",
        (Rotation::Clockwise180, "south_west") => "north_east",
        (Rotation::Clockwise180, "north_west") => "south_east",
        (Rotation::Clockwise180, "north_east") => "south_west",
        (Rotation::Counterclockwise90, "ascending_east") => "ascending_north",
        (Rotation::Counterclockwise90, "ascending_west") => "ascending_south",
        (Rotation::Counterclockwise90, "ascending_north") => "ascending_west",
        (Rotation::Counterclockwise90, "ascending_south") => "ascending_east",
        (Rotation::Counterclockwise90, "south_east") => "north_east",
        (Rotation::Counterclockwise90, "south_west") => "south_east",
        (Rotation::Counterclockwise90, "north_west") => "south_west",
        (Rotation::Counterclockwise90, "north_east") => "north_west",
        _ => return None,
    })
}

fn value_of<'a>(layout: &'a BlockLayout, state: VoxelId, name: &str) -> Option<&'a str> {
    let property = layout.property(name)?;
    Some(&property.values[layout.value_index(state, property) as usize])
}

/// `BlockState.rotate` for a rotation about Y, over the property names the
/// block overrides act on.
// ponytail: every placed block looks its properties up by name; the upgrade is
// a per-block table built once from `layouts`, as `tree/provider.rs` does.
pub fn rotate_state(world: &WorldStates, state: VoxelId, rotation: Rotation) -> VoxelId {
    if rotation == Rotation::None || world.unrotated.contains(state.0 as usize) {
        return state;
    }
    let Some(layout) = world.layout_of(state) else {
        return state;
    };
    let quarter_turns = rotation as u16;
    let mut out = state;
    for property in &layout.properties {
        let value = &*property.values[layout.value_index(state, property) as usize];
        match &*property.name {
            "facing" => {
                if let Some(d) = direction_named(value) {
                    out = layout.try_set(out, "facing", rotation.rotate(d).name());
                }
            }
            "axis"
                if matches!(
                    rotation,
                    Rotation::Clockwise90 | Rotation::Counterclockwise90
                ) =>
            {
                match value {
                    "x" => out = layout.try_set(out, "axis", "z"),
                    "z" => out = layout.try_set(out, "axis", "x"),
                    _ => {}
                }
            }
            "rotation" => {
                let count = property.values.len() as u16;
                let index = layout.value_index(state, property);
                out = layout.with_index(out, property, (index + count / 4 * quarter_turns) % count);
            }
            "shape" => {
                if let Some(rotated) = rail_shape(value, rotation) {
                    out = layout.try_set(out, "shape", rotated);
                }
            }
            "orientation" => {
                if let Some((front, top)) = value
                    .split_once('_')
                    .and_then(|(f, t)| Some((direction_named(f)?, direction_named(t)?)))
                {
                    let rotated = format!(
                        "{}_{}",
                        rotation.rotate(front).name(),
                        rotation.rotate(top).name()
                    );
                    out = layout.try_set(out, "orientation", &rotated);
                }
            }
            _ => {}
        }
    }
    if Direction::HORIZONTAL
        .iter()
        .all(|d| layout.property(d.name()).is_some())
    {
        for side in Direction::HORIZONTAL {
            let value = value_of(layout, state, side.name()).expect("checked above");
            out = layout.try_set(out, rotation.rotate(side).name(), value);
        }
    }
    out
}

fn rail_mirror(shape: &str, mirror: Mirror) -> Option<&'static str> {
    Some(match (mirror, shape) {
        (Mirror::LeftRight, "ascending_north") => "ascending_south",
        (Mirror::LeftRight, "ascending_south") => "ascending_north",
        (Mirror::LeftRight, "south_east") => "north_east",
        (Mirror::LeftRight, "south_west") => "north_west",
        (Mirror::LeftRight, "north_west") => "south_west",
        (Mirror::LeftRight, "north_east") => "south_east",
        (Mirror::FrontBack, "ascending_east") => "ascending_west",
        (Mirror::FrontBack, "ascending_west") => "ascending_east",
        (Mirror::FrontBack, "south_east") => "south_west",
        (Mirror::FrontBack, "south_west") => "south_east",
        (Mirror::FrontBack, "north_west") => "north_east",
        (Mirror::FrontBack, "north_east") => "north_west",
        _ => return None,
    })
}

// The reference keeps the inner shapes of a front-back mirrored stair as they
// are, so a mirrored inner corner is not the corner a player would build.
fn stair_mirror(shape: &str, mirror: Mirror) -> Option<&'static str> {
    Some(match (mirror, shape) {
        (Mirror::LeftRight, "inner_left") => "inner_right",
        (Mirror::LeftRight, "inner_right") => "inner_left",
        (_, "outer_left") => "outer_right",
        (_, "outer_right") => "outer_left",
        _ => return None,
    })
}

/// `BlockState.mirror`, over the property names the block overrides act on:
/// a facing on the mirrored axis is a half turn, and the rest reflect.
pub fn mirror_state(world: &WorldStates, state: VoxelId, mirror: Mirror) -> VoxelId {
    if mirror == Mirror::None || world.unmirrored.contains(state.0 as usize) {
        return state;
    }
    let Some(layout) = world.layout_of(state) else {
        return state;
    };
    let facing = value_of(layout, state, "facing").and_then(direction_named);
    let flipped = facing.is_some_and(|d| mirror.rotation(d) == Rotation::Clockwise180);
    let mut out = if flipped {
        rotate_state(world, state, Rotation::Clockwise180)
    } else {
        state
    };
    for property in &layout.properties {
        let index = layout.value_index(state, property);
        let count = property.values.len() as u16;
        let value = &*property.values[index as usize];
        match &*property.name {
            "hinge" => out = layout.with_index(out, property, (index + 1) % count),
            "rotation" => out = layout.with_index(out, property, mirror.mirror_index(index, count)),
            "shape" => {
                let mirrored = match facing {
                    Some(_) => flipped.then(|| stair_mirror(value, mirror)).flatten(),
                    None => rail_mirror(value, mirror),
                };
                if let Some(mirrored) = mirrored {
                    out = layout.try_set(out, "shape", mirrored);
                }
            }
            "orientation" => {
                if let Some((front, top)) = value
                    .split_once('_')
                    .and_then(|(f, t)| Some((direction_named(f)?, direction_named(t)?)))
                {
                    let mirrored = format!(
                        "{}_{}",
                        mirror.mirror(front).name(),
                        mirror.mirror(top).name()
                    );
                    out = layout.try_set(out, "orientation", &mirrored);
                }
            }
            _ => {}
        }
    }
    if Direction::HORIZONTAL
        .iter()
        .all(|d| layout.property(d.name()).is_some())
    {
        for side in Direction::HORIZONTAL {
            let value = value_of(layout, state, side.name()).expect("checked above");
            out = layout.try_set(out, mirror.mirror(side).name(), value);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct PosRule {
    pub linear: LinearPos,
    pub axis: Option<Axis>,
}

impl PosRule {
    fn test<R: Random>(&self, world_pos: IVec3, reference: IVec3, rng: &mut R) -> bool {
        let dist = match self.axis {
            None => dist_manhattan(world_pos, reference),
            Some(axis) => (world_pos - reference).abs()[axis as usize],
        };
        let l = &self.linear;
        rng.next_f32()
            <= clamped_map(
                dist as f32,
                l.min_dist as f32,
                l.max_dist as f32,
                l.min_chance as f32,
                l.max_chance as f32,
            )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppendLoot {
    pub loot_table: String,
    pub entity_id: &'static str,
}

impl AppendLoot {
    fn apply(&self, rng: &mut LegacyRandom, nbt: &mut Option<NbtCompound>) {
        let out = nbt.get_or_insert_default();
        out.child_tags
            .retain(|(key, _)| !matches!(key.as_str(), "LootTable" | "LootTableSeed"));
        if out.get("id").is_none() {
            out.put_string("id", self.entity_id.to_owned());
        }
        out.put_string("LootTable", self.loot_table.clone());
        out.put_long("LootTableSeed", rng.next_java_long());
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledRule {
    pub input: Rule,
    pub location: Rule,
    pub position: Option<PosRule>,
    pub output: VoxelId,
    pub modifier: Option<AppendLoot>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompiledProcessor {
    BlockIgnore(StateMask),
    JigsawReplacement {
        jigsaw: StateMask,
    },
    Rule(Vec<CompiledRule>),
    ProtectedBlocks(StateMask),
    BlockRot {
        rottable: Option<StateMask>,
        integrity: f32,
    },
    Gravity {
        heightmap: HeightmapName,
        offset: i32,
    },
    Capped {
        delegate: Box<CompiledProcessor>,
        limit: IntProvider,
        world_seed: i64,
    },
}

pub type CompiledChain = Vec<CompiledProcessor>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainKind {
    Piece {
        projection: Projection,
        legacy: bool,
    },
    Feature,
}

fn block_mask(blocks: &dyn BlockResolver, name: &str) -> Result<StateMask, FeatureCompileError> {
    states_of(
        blocks,
        StateQuery::Block(&ResourceLocation::minecraft(name)),
    )
}

pub fn compile_chain(
    list: &[StructureProcessor],
    kind: ChainKind,
    blocks: &dyn BlockResolver,
    world_seed: i64,
) -> Result<CompiledChain, FeatureCompileError> {
    let mut processors = Vec::with_capacity(list.len() + 3);
    if let ChainKind::Piece { legacy, .. } = kind {
        if !legacy {
            processors.push(CompiledProcessor::BlockIgnore(block_mask(
                blocks,
                "structure_block",
            )?));
        }
        processors.push(CompiledProcessor::JigsawReplacement {
            jigsaw: block_mask(blocks, "jigsaw")?,
        });
    }
    for (index, processor) in list.iter().enumerate() {
        if let Some(compiled) = compile_processor(processor, kind, blocks, world_seed)
            .map_err(|e| e.within(format!("processor {index}")))?
        {
            processors.push(compiled);
        }
    }
    if let ChainKind::Piece { projection, legacy } = kind {
        if projection == Projection::TerrainMatching {
            processors.push(CompiledProcessor::Gravity {
                heightmap: HeightmapName::WorldSurfaceWg,
                offset: -1,
            });
        }
        if legacy {
            processors.push(CompiledProcessor::BlockIgnore(states_of(
                blocks,
                StateQuery::Blocks(&HolderSet::List(vec![
                    ResourceLocation::minecraft("air"),
                    ResourceLocation::minecraft("structure_block"),
                ])),
            )?));
        }
    }
    Ok(processors)
}

fn compile_processor(
    processor: &StructureProcessor,
    kind: ChainKind,
    blocks: &dyn BlockResolver,
    world_seed: i64,
) -> Result<Option<CompiledProcessor>, FeatureCompileError> {
    use StructureProcessor::*;
    let unsupported = |what: &str| Err(FeatureCompileError::Unsupported(what.to_owned()));
    Ok(Some(match processor {
        Nop => return Ok(None),
        BlockIgnore { blocks: states } => CompiledProcessor::BlockIgnore(states_of(
            blocks,
            StateQuery::Blocks(&HolderSet::List(
                states.iter().map(|state| state.name.clone()).collect(),
            )),
        )?),
        JigsawReplacement => CompiledProcessor::JigsawReplacement {
            jigsaw: block_mask(blocks, "jigsaw")?,
        },
        Rule { rules } => CompiledProcessor::Rule(
            rules
                .iter()
                .map(|rule| compile_processor_rule(rule, blocks))
                .collect::<Result<_, _>>()?,
        ),
        ProtectedBlocks { value } => {
            CompiledProcessor::ProtectedBlocks(states_of(blocks, StateQuery::Blocks(value))?)
        }
        // ponytail: with a feature's own random on the settings, `block_rot`
        // draws from the feature stream instead; no shipped list does that.
        BlockRot { .. } if kind == ChainKind::Feature => {
            return unsupported("block_rot in a feature's processor list");
        }
        BlockRot {
            rottable_blocks,
            integrity,
        } => CompiledProcessor::BlockRot {
            rottable: rottable_blocks
                .as_ref()
                .map(|set| states_of(blocks, StateQuery::Blocks(set)))
                .transpose()?,
            integrity: integrity.0 as f32,
        },
        Gravity { heightmap, offset } => CompiledProcessor::Gravity {
            heightmap: heightmap.unwrap_or(HeightmapName::WorldSurfaceWg),
            offset: *offset,
        },
        Capped { delegate, limit } => CompiledProcessor::Capped {
            delegate: Box::new(
                compile_processor(delegate, kind, blocks, world_seed)?
                    .unwrap_or(CompiledProcessor::Rule(Vec::new())),
            ),
            limit: limit.clone(),
            world_seed,
        },
        // ponytail: no shipped list uses these three; each needs its own arm
        // and the draw source it reads.
        BlackstoneReplace => return unsupported("blackstone_replace"),
        BlockAge { .. } => return unsupported("block_age"),
        LavaSubmergedBlock => return unsupported("lava_submerged_block"),
    }))
}

fn compile_processor_rule(
    rule: &ProcessorRule,
    blocks: &dyn BlockResolver,
) -> Result<CompiledRule, FeatureCompileError> {
    let position = match &rule.position_predicate {
        None | Some(PosRuleTest::AlwaysTrue) => None,
        Some(PosRuleTest::LinearPos(linear)) => Some(PosRule {
            linear: linear.clone(),
            axis: None,
        }),
        Some(PosRuleTest::AxisAlignedLinearPos { linear, axis }) => Some(PosRule {
            linear: linear.clone(),
            axis: Some(axis.unwrap_or(Axis::Y)),
        }),
    };
    let output_name = rule.output_state.name.to_string();
    let modifier = match &rule.block_entity_modifier {
        None | Some(RuleBlockEntityModifier::Passthrough) => None,
        Some(RuleBlockEntityModifier::AppendLoot { loot_table }) => {
            // ponytail: the entity id comes from the output block's name, which
            // holds for brushable blocks and the modelled kinds that share a
            // name with their entity; a container output under another name
            // needs a name-to-kind arm.
            let entity_id = match output_name.as_str() {
                "minecraft:suspicious_sand" | "minecraft:suspicious_gravel" => {
                    "minecraft:brushable_block"
                }
                name => *GeneratedBlockEntity::IDS
                    .iter()
                    .find(|id| **id == name)
                    .ok_or_else(|| {
                        FeatureCompileError::Unsupported(format!("append_loot onto {name}"))
                    })?,
            };
            Some(AppendLoot {
                loot_table: loot_table.to_string(),
                entity_id,
            })
        }
        Some(RuleBlockEntityModifier::Clear) => {
            return Err(FeatureCompileError::Unsupported("clear".to_owned()));
        }
    };
    Ok(CompiledRule {
        input: compile_rule(&rule.input_predicate, blocks)?,
        location: compile_rule(&rule.location_predicate, blocks)?,
        position,
        output: state_of(blocks, &rule.output_state)?,
        modifier,
    })
}

pub struct Placement<'a> {
    pub template: &'a FrozenTemplate,
    pub jigsaws: &'a [JigsawBlock],
    pub palette: usize,
    pub position: IVec3,
    pub reference: IVec3,
    pub rotation: Rotation,
    pub clip: Option<BoundingBox>,
    pub chain: &'a CompiledChain,
    pub waterlog: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Processed {
    pub world_pos: IVec3,
    pub template_pos: IVec3,
    pub state: VoxelId,
    pub nbt: Option<NbtCompound>,
}

impl CompiledProcessor {
    fn process<W: WorldGenVolume>(&self, volume: &W, p: &Placement<'_>, b: &mut Processed) -> bool {
        match self {
            CompiledProcessor::BlockIgnore(mask) => !mask.contains(b.state.0 as usize),
            CompiledProcessor::JigsawReplacement { jigsaw } => {
                if !jigsaw.contains(b.state.0 as usize) || b.nbt.is_none() {
                    return true;
                }
                let template_pos = b.template_pos;
                match p
                    .jigsaws
                    .iter()
                    .find(|j| IVec3::from(j.pos.map(i32::from)) == template_pos)
                {
                    None => true,
                    Some(JigsawBlock {
                        final_state: None, ..
                    }) => false,
                    Some(JigsawBlock {
                        final_state: Some(state),
                        ..
                    }) => {
                        b.state = *state;
                        b.nbt = None;
                        true
                    }
                }
            }
            CompiledProcessor::Rule(rules) => {
                let mut rng = LegacyRandom::new(block_pos_seed(b.world_pos));
                let y = b.world_pos.y;
                let location = volume.get(b.world_pos.into());
                for rule in rules {
                    let passes = rule.input.test(b.state, y, &mut rng)
                        && rule.location.test(location, y, &mut rng)
                        && rule
                            .position
                            .as_ref()
                            .is_none_or(|t| t.test(b.world_pos, p.reference, &mut rng));
                    if passes {
                        b.state = rule.output;
                        if let Some(loot) = &rule.modifier {
                            loot.apply(&mut rng, &mut b.nbt);
                        }
                        return true;
                    }
                }
                true
            }
            CompiledProcessor::ProtectedBlocks(mask) => {
                !mask.contains(volume.get(b.world_pos.into()).0 as usize)
            }
            CompiledProcessor::BlockRot {
                rottable,
                integrity,
            } => {
                let mut rng = LegacyRandom::new(block_pos_seed(b.world_pos));
                !rottable
                    .as_ref()
                    .is_none_or(|mask| mask.contains(b.state.0 as usize))
                    || rng.next_f32() <= *integrity
            }
            CompiledProcessor::Gravity { heightmap, offset } => {
                b.world_pos.y = volume.height(*heightmap, b.world_pos.x, b.world_pos.z)
                    + offset
                    + b.template_pos.y;
                true
            }
            CompiledProcessor::Capped { .. } => true,
        }
    }

    fn finalize<W: WorldGenVolume>(
        &self,
        volume: &W,
        p: &Placement<'_>,
        processed: &mut [Processed],
    ) {
        let CompiledProcessor::Capped {
            delegate,
            limit,
            world_seed,
        } = self
        else {
            return;
        };
        if limit.bounds().1 == 0 || processed.is_empty() {
            return;
        }
        let mut rng = LegacyRandom::new(*world_seed as u64).fork_at(p.position);
        let to_replace = limit.sample(&mut rng).min(processed.len() as i32);
        if to_replace < 1 {
            return;
        }
        let order = shuffled(&(0..processed.len()).collect::<Vec<_>>(), &mut rng);
        let mut replaced = 0;
        for index in order {
            if replaced >= to_replace {
                break;
            }
            let mut altered = processed[index].clone();
            if delegate.process(volume, p, &mut altered) && altered != processed[index] {
                processed[index] = altered;
                replaced += 1;
            }
        }
    }
}

fn has_waterlogged(world: &WorldStates, state: VoxelId) -> bool {
    world
        .layout_of(state)
        .is_some_and(|layout| layout.property("waterlogged").is_some())
}

/// `LiquidBlockContainer.placeLiquid` for source water: the block takes the
/// water unless it already holds some or is a double slab; a lit candle or
/// campfire goes out.
fn place_liquid<W: WorldGenVolume>(volume: &mut W, pos: BlockPos, state: VoxelId, fluid: VoxelId) {
    let world = volume.world();
    let Some(layout) = world.layout_of(state) else {
        return;
    };
    if value_of(layout, state, "waterlogged") != Some("false")
        || !world.water_source.contains(fluid.0 as usize)
        || value_of(layout, state, "type") == Some("double")
    {
        return;
    }
    let mut wet = layout.try_set(state, "waterlogged", "true");
    wet = layout.try_set(wet, "lit", "false");
    volume.set(pos, wet);
}

/// `StructureTemplate.placeInWorld` for one palette the caller has already
/// drawn, with no mirror and the pivot at the template origin.
// ponytail: template entities are never placed (172 shipped templates carry
// some); the upgrade is a second typed list delivered like block entities.
pub fn place_template<W: WorldGenVolume>(
    p: &Placement<'_>,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    entities: &mut Vec<GeneratedBlockEntity>,
) -> bool {
    let Some(blocks) = p.template.palettes.get(p.palette) else {
        return false;
    };
    if blocks.is_empty() || p.template.size.contains(&0) {
        return false;
    }
    let inside = |pos: IVec3| p.clip.is_none_or(|clip| clip.is_inside(pos.into()));
    // `evaluatesEntirePieceState`: a capped processor must see every block
    // of the piece, so the clip waits until the write.
    let whole_piece = p
        .chain
        .iter()
        .any(|p| matches!(p, CompiledProcessor::Capped { .. }));

    let mut processed = Vec::with_capacity(blocks.len());
    for block in blocks.iter() {
        let template_pos = IVec3::from(block.pos.map(i32::from));
        let world_pos = transform(template_pos, Mirror::None, p.rotation, IVec3::ZERO) + p.position;
        if !whole_piece && !inside(world_pos) {
            continue;
        }
        let mut b = Processed {
            world_pos,
            template_pos,
            state: block.state,
            nbt: block.nbt.clone(),
        };
        if p.chain
            .iter()
            .all(|processor| processor.process(volume, p, &mut b))
        {
            processed.push(b);
        }
    }
    for processor in p.chain {
        processor.finalize(volume, p, &mut processed);
    }

    let source =
        |volume: &W, state: VoxelId| volume.world().any_source_fluid.contains(state.0 as usize);
    let mut to_fill = Vec::new();
    let mut locked = Vec::new();
    for b in &processed {
        if !inside(b.world_pos) {
            continue;
        }
        let pos = BlockPos::from(b.world_pos);
        let previous = p.waterlog.then(|| volume.get(pos));
        let state = rotate_state(volume.world(), b.state, p.rotation);
        volume.set(pos, state);
        if let Some(nbt) = &b.nbt {
            let seed = GeneratedBlockEntity::wants_loot_seed(nbt).then(|| rng.next_i64());
            match GeneratedBlockEntity::from_template(nbt, pos, seed) {
                Ok(Some(entity)) => entities.push(entity),
                Ok(None) => {}
                Err(error) => debug_assert!(false, "block entity at {pos} failed to load: {error}"),
            }
        }
        if let Some(previous) = previous {
            if source(volume, state) {
                locked.push(pos);
            } else if has_waterlogged(volume.world(), state) {
                place_liquid(volume, pos, state, previous);
                if !source(volume, previous) {
                    to_fill.push(pos);
                }
            }
        }
    }

    let mut filled = true;
    while filled && !to_fill.is_empty() {
        filled = false;
        to_fill.retain(|&pos| {
            let mut fluid = volume.get(pos);
            for direction in [Direction::Up].into_iter().chain(Direction::HORIZONTAL) {
                if source(volume, fluid) {
                    break;
                }
                let neighbor = pos + direction.normal();
                let candidate = volume.get(neighbor);
                if source(volume, candidate) && !locked.contains(&neighbor) {
                    fluid = candidate;
                }
            }
            let state = volume.get(pos);
            if source(volume, fluid) && has_waterlogged(volume.world(), state) {
                place_liquid(volume, pos, state, fluid);
                filled = true;
                return false;
            }
            true
        });
    }
    true
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fixedbitset::FixedBitSet;

    use super::*;
    use mcrs_minecraft_chunk::{Blocks, BlocksMut};
    use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, PropertyLayout, mask_of};
    use mcrs_minecraft_worldgen_feature::template::FrozenBlock;

    // Thirty-two ids per block from each layout's base: the plain blocks at 0,
    // then a fence, a candle and a slab.
    const AIR: VoxelId = VoxelId(0);
    const STRUCTURE_BLOCK: VoxelId = VoxelId(1);
    const JIGSAW: VoxelId = VoxelId(2);
    const WATER: VoxelId = VoxelId(3);
    const DIRT: VoxelId = VoxelId(4);
    const STONE: VoxelId = VoxelId(5);
    const FENCE: VoxelId = VoxelId(32);
    const CANDLE: VoxelId = VoxelId(64);
    const SLAB: VoxelId = VoxelId(96);

    fn property(name: &str, values: &[&str], stride: u16) -> PropertyLayout {
        PropertyLayout {
            name: Arc::from(name),
            values: values.iter().map(|v| Arc::from(*v)).collect(),
            stride,
        }
    }

    const BOOL: [&str; 2] = ["true", "false"];

    fn world() -> WorldStates {
        let sides = |base: u16| BlockLayout {
            base,
            properties: vec![
                property("north", &BOOL, 16),
                property("east", &BOOL, 8),
                property("south", &BOOL, 4),
                property("west", &BOOL, 2),
                property("waterlogged", &BOOL, 1),
            ],
        };
        let layouts: Vec<BlockLayout> = vec![
            BlockLayout {
                base: 0,
                properties: vec![],
            },
            sides(FENCE.0),
            BlockLayout {
                base: CANDLE.0,
                properties: vec![property("lit", &BOOL, 2), property("waterlogged", &BOOL, 1)],
            },
            BlockLayout {
                base: SLAB.0,
                properties: vec![
                    property("type", &["top", "bottom", "double"], 2),
                    property("waterlogged", &BOOL, 1),
                ],
            },
        ];
        let mut block_of_state = vec![0u32; 128];
        let mut any_source = FixedBitSet::with_capacity(128);
        any_source.insert(WATER.0 as usize);
        for (block, layout) in layouts.iter().enumerate() {
            for id in layout.base..layout.base + 32 {
                block_of_state[id as usize] = block as u32;
                if value_of(layout, VoxelId(id), "waterlogged") == Some("true") {
                    any_source.insert(id as usize);
                }
            }
        }
        WorldStates {
            air: AIR,
            air_states: mask_of([AIR.0]),
            water: WATER,
            water_states: mask_of([WATER.0]),
            water_source: Arc::new(any_source.clone()),
            any_source_fluid: Arc::new(any_source),
            block_of_state: block_of_state.into(),
            layouts: layouts.into(),
            ..WorldStates::default()
        }
    }

    fn with(base: VoxelId, pairs: &[(&str, &str)]) -> VoxelId {
        let world = world();
        let layout = world.layout_of(base).unwrap();
        pairs.iter().fold(base, |state, (name, value)| {
            layout.try_set(state, name, value)
        })
    }

    fn dry_fence() -> VoxelId {
        with(
            FENCE,
            &[
                ("north", "false"),
                ("east", "false"),
                ("south", "false"),
                ("west", "false"),
                ("waterlogged", "false"),
            ],
        )
    }

    fn block(pos: [u16; 3], state: VoxelId) -> FrozenBlock {
        FrozenBlock {
            pos,
            state,
            nbt: None,
        }
    }

    fn with_nbt(pos: [u16; 3], state: VoxelId, id: &str) -> FrozenBlock {
        let mut nbt = NbtCompound::default();
        nbt.put_string("id", id.to_owned());
        FrozenBlock {
            pos,
            state,
            nbt: Some(nbt),
        }
    }

    /// Three wide, two tall, three deep: dirt floor, a stone pillar at the
    /// centre, a chest on top of it.
    fn template() -> FrozenTemplate {
        let mut blocks = Vec::new();
        for z in 0..3 {
            for x in 0..3 {
                blocks.push(block([x, 0, z], DIRT));
            }
        }
        blocks.push(block([0, 1, 0], AIR));
        blocks.push(block([1, 1, 1], STONE));
        let mut chest = with_nbt([2, 1, 2], STONE, "minecraft:chest");
        chest
            .nbt
            .as_mut()
            .unwrap()
            .put_string("LootTable", "minecraft:chests/simple_dungeon".to_owned());
        blocks.push(chest);
        FrozenTemplate {
            size: [3, 2, 3],
            palettes: vec![blocks.into_boxed_slice()],
        }
    }

    fn new_region() -> BoxRegion {
        let mut region = BoxRegion::new(BlockPos::new(-8, 0, -8), BlockPos::new(24, 16, 24), AIR);
        region.world = world();
        region
    }

    fn placement<'a>(
        template: &'a FrozenTemplate,
        chain: &'a CompiledChain,
        rotation: Rotation,
        clip: Option<BoundingBox>,
    ) -> Placement<'a> {
        Placement {
            template,
            jigsaws: &[],
            palette: 0,
            position: IVec3::new(4, 5, 4),
            reference: IVec3::new(4, 5, 4),
            rotation,
            clip,
            chain,
            waterlog: false,
        }
    }

    fn place(p: &Placement<'_>) -> (BoxRegion, Vec<GeneratedBlockEntity>, XoroshiroRandom, bool) {
        let mut region = new_region();
        let mut rng = XoroshiroRandom::new(7);
        let mut entities = Vec::new();
        let placed = place_template(p, &mut region, &mut rng, &mut entities);
        (region, entities, rng, placed)
    }

    fn rule(input: Rule, output: VoxelId, modifier: Option<AppendLoot>) -> CompiledProcessor {
        CompiledProcessor::Rule(vec![CompiledRule {
            input,
            location: Rule::AlwaysTrue,
            position: None,
            output,
            modifier,
        }])
    }

    #[test]
    fn writes_follow_template_order_after_the_transform() {
        let template = template();
        let chain = vec![];
        let (region, _, _, placed) =
            place(&placement(&template, &chain, Rotation::Clockwise90, None));
        assert!(placed);
        let expected: Vec<(BlockPos, VoxelId)> = template.palettes[0]
            .iter()
            .map(|b| {
                let pos = transform(
                    IVec3::from(b.pos.map(i32::from)),
                    Mirror::None,
                    Rotation::Clockwise90,
                    IVec3::ZERO,
                ) + IVec3::new(4, 5, 4);
                (BlockPos::from(pos), b.state)
            })
            .collect();
        assert_eq!(region.writes, expected);
        assert_eq!(
            region.writes[1].0,
            BlockPos::new(4, 5, 5),
            "x becomes z under a quarter turn"
        );
    }

    #[test]
    fn an_empty_palette_places_nothing() {
        let template = FrozenTemplate::empty();
        let chain = vec![];
        let (region, _, _, placed) = place(&placement(&template, &chain, Rotation::None, None));
        assert!(!placed);
        assert!(region.writes.is_empty());
    }

    #[test]
    fn the_clip_bounds_the_writes_with_and_without_a_capped_chain() {
        let template = template();
        let draws = rule(
            Rule::RandomStates {
                states: mask_of([DIRT.0]),
                probability: 1.0,
            },
            STONE,
            None,
        );
        let clip = BoundingBox::from_corners(BlockPos::new(4, 0, 4), BlockPos::new(5, 16, 6));
        let chain = vec![draws.clone()];
        let (region, ..) = place(&placement(&template, &chain, Rotation::None, Some(clip)));
        assert!(region.writes.iter().all(|(pos, _)| clip.is_inside(*pos)));
        assert_eq!(
            region.writes.iter().filter(|(_, s)| *s == STONE).count(),
            6 + 1,
            "six dirt and the pillar inside the clip; the chest column is outside"
        );

        let capped = vec![CompiledProcessor::Capped {
            delegate: Box::new(draws),
            limit: IntProvider::Constant(2),
            world_seed: 0x5EED,
        }];
        let (region, ..) = place(&placement(&template, &capped, Rotation::None, Some(clip)));
        assert!(
            region.writes.iter().all(|(pos, _)| clip.is_inside(*pos)),
            "a capped chain still clips at the write"
        );
    }

    #[test]
    fn gravity_moves_a_block_out_of_the_clip() {
        let template = template();
        let chain = vec![CompiledProcessor::Gravity {
            heightmap: HeightmapName::WorldSurfaceWg,
            offset: -1,
        }];
        let clip = BoundingBox::from_corners(BlockPos::new(0, 0, 0), BlockPos::new(15, 12, 15));
        let mut region = new_region().with_height(|_, _, _, _| 30);
        let mut rng = XoroshiroRandom::new(7);
        let mut entities = Vec::new();
        assert!(place_template(
            &placement(&template, &chain, Rotation::None, Some(clip)),
            &mut region,
            &mut rng,
            &mut entities
        ));
        assert!(region.writes.is_empty(), "every block moved to y=29..30");
        assert!(entities.is_empty());
    }

    #[test]
    fn legacy_ignore_drops_air_and_jigsaws_take_their_final_state() {
        let mut template = template();
        let mut jigsaw = with_nbt([1, 1, 1], JIGSAW, "minecraft:jigsaw");
        jigsaw
            .nbt
            .as_mut()
            .unwrap()
            .put_string("final_state", "minecraft:stone".to_owned());
        let mut void = with_nbt([0, 1, 0], JIGSAW, "minecraft:jigsaw");
        void.nbt
            .as_mut()
            .unwrap()
            .put_string("final_state", "minecraft:structure_void".to_owned());
        template.palettes[0] = vec![
            block([0, 0, 0], AIR),
            block([1, 0, 0], STRUCTURE_BLOCK),
            void,
            jigsaw,
            block([2, 1, 2], DIRT),
        ]
        .into_boxed_slice();
        let jigsaws = vec![
            JigsawBlock {
                pos: [1, 1, 1],
                final_state: Some(STONE),
                ..jigsaws_template()
            },
            JigsawBlock {
                pos: [0, 1, 0],
                ..jigsaws_template()
            },
        ];
        let chain = vec![
            CompiledProcessor::JigsawReplacement {
                jigsaw: mask_of([JIGSAW.0]),
            },
            CompiledProcessor::BlockIgnore(mask_of([AIR.0, STRUCTURE_BLOCK.0])),
        ];
        let mut p = placement(&template, &chain, Rotation::None, None);
        p.jigsaws = &jigsaws;
        let (region, entities, ..) = place(&p);
        assert_eq!(
            region.writes,
            vec![
                (BlockPos::new(5, 6, 5), STONE),
                (BlockPos::new(6, 6, 6), DIRT)
            ]
        );
        assert!(entities.is_empty(), "a replaced jigsaw keeps no nbt");
    }

    fn jigsaws_template() -> JigsawBlock {
        JigsawBlock {
            pos: [0, 0, 0],
            front: Direction::North,
            top: Direction::Up,
            joint: mcrs_minecraft_worldgen_feature::template::Joint::Rollable,
            name: ResourceLocation::minecraft("empty"),
            pool: ResourceLocation::minecraft("empty"),
            target: ResourceLocation::minecraft("empty"),
            placement_priority: 0,
            selection_priority: 0,
            final_state: None,
        }
    }

    #[test]
    fn a_container_takes_the_next_long_of_the_stream_only_beside_a_table() {
        let template = template();
        let chain = vec![];
        let (_, entities, mut rng, _) = place(&placement(&template, &chain, Rotation::None, None));
        let mut replay = XoroshiroRandom::new(7);
        let seed = replay.next_i64();
        assert_eq!(rng.next_i64(), replay.next_i64(), "one draw was spent");
        assert_eq!(
            entities,
            vec![GeneratedBlockEntity::chest(
                BlockPos::new(6, 6, 6),
                "minecraft:chests/simple_dungeon".to_owned(),
                seed
            )]
        );

        let mut bare = template.clone();
        bare.palettes[0] = vec![with_nbt([2, 1, 2], STONE, "minecraft:chest")].into_boxed_slice();
        let (_, entities, mut rng, _) = place(&placement(&bare, &chain, Rotation::None, None));
        let mut replay = XoroshiroRandom::new(7);
        replay.next_i64();
        assert_eq!(rng.next_i64(), replay.next_i64(), "drawn but not stored");
        let compound = mcrs_minecraft_nbt::to_nbt_compound(&entities[0]).unwrap();
        assert!(compound.get("LootTableSeed").is_none());
        assert!(compound.get("LootTable").is_none());
    }

    #[test]
    fn append_loot_seeds_from_the_positional_random_and_names_the_entity() {
        let mut template = template();
        template.palettes[0] = vec![block([1, 1, 1], DIRT)].into_boxed_slice();
        let chain = vec![rule(
            Rule::MatchingStates(mask_of([DIRT.0])),
            STONE,
            Some(AppendLoot {
                loot_table: "minecraft:archaeology/desert_well".to_owned(),
                entity_id: "minecraft:brushable_block",
            }),
        )];
        let (region, entities, ..) = place(&placement(&template, &chain, Rotation::None, None));
        assert_eq!(region.writes, vec![(BlockPos::new(5, 6, 5), STONE)]);
        let expected_seed = LegacyRandom::new(block_pos_seed(IVec3::new(5, 6, 5))).next_java_long();
        let compound = mcrs_minecraft_nbt::to_nbt_compound(&entities[0]).unwrap();
        assert_eq!(compound.get_string("id"), Some("minecraft:brushable_block"));
        assert_eq!(compound.get_long("LootTableSeed"), Some(expected_seed));
        assert_eq!(compound.get_int("x"), Some(5));
    }

    #[test]
    fn capped_replaces_at_most_the_limit_in_shuffled_order() {
        let template = template();
        let delegate = rule(Rule::MatchingStates(mask_of([DIRT.0])), STONE, None);
        let chain = vec![CompiledProcessor::Capped {
            delegate: Box::new(delegate),
            limit: IntProvider::Constant(2),
            world_seed: 0x5EED,
        }];
        let (region, ..) = place(&placement(&template, &chain, Rotation::None, None));
        let stones: Vec<BlockPos> = region
            .writes
            .iter()
            .filter(|(pos, s)| *s == STONE && pos.y == 5)
            .map(|(pos, _)| *pos)
            .collect();
        assert_eq!(stones.len(), 2);

        let mut rng = LegacyRandom::new(0x5EED).fork_at(IVec3::new(4, 5, 4));
        let order = shuffled(
            &(0..template.palettes[0].len()).collect::<Vec<_>>(),
            &mut rng,
        );
        let expected: Vec<BlockPos> = order
            .into_iter()
            .filter(|i| *i < 9)
            .take(2)
            .map(|i| {
                let b = &template.palettes[0][i];
                BlockPos::from(IVec3::from(b.pos.map(i32::from)) + IVec3::new(4, 5, 4))
            })
            .collect();
        let mut sorted_expected = expected.clone();
        let mut sorted = stones.clone();
        sorted.sort_by_key(|p| (p.x, p.z));
        sorted_expected.sort_by_key(|p| (p.x, p.z));
        assert_eq!(sorted, sorted_expected);

        let unlimited = vec![CompiledProcessor::Capped {
            delegate: Box::new(rule(Rule::MatchingStates(mask_of([DIRT.0])), STONE, None)),
            limit: IntProvider::Constant(50),
            world_seed: 0x5EED,
        }];
        let (region, ..) = place(&placement(&template, &unlimited, Rotation::None, None));
        assert_eq!(
            region.writes.iter().filter(|(_, s)| *s == STONE).count(),
            9 + 2,
            "the limit clamps to the block count"
        );
    }

    #[test]
    fn a_fence_written_into_water_takes_it_and_floods_its_neighbour() {
        let fence = dry_fence();
        let template = FrozenTemplate {
            size: [2, 1, 1],
            palettes: vec![
                vec![block([0, 0, 0], fence), block([1, 0, 0], fence)].into_boxed_slice(),
            ],
        };
        let chain = vec![];
        let mut p = placement(&template, &chain, Rotation::None, None);
        p.waterlog = true;
        let mut region = new_region();
        region.blocks.set(BlockPos::new(4, 5, 4), WATER);
        let mut rng = XoroshiroRandom::new(1);
        let mut entities = Vec::new();
        place_template(&p, &mut region, &mut rng, &mut entities);
        let wet = with(fence, &[("waterlogged", "true")]);
        assert_eq!(region.get(BlockPos::new(4, 5, 4)), wet);
        assert_eq!(
            region.get(BlockPos::new(5, 5, 4)),
            wet,
            "flooded from the west"
        );
        assert_eq!(
            region.writes,
            vec![
                (BlockPos::new(4, 5, 4), fence),
                (BlockPos::new(4, 5, 4), wet),
                (BlockPos::new(5, 5, 4), fence),
                (BlockPos::new(5, 5, 4), wet),
            ]
        );

        p.waterlog = false;
        let mut region = new_region();
        region.blocks.set(BlockPos::new(4, 5, 4), WATER);
        place_template(&p, &mut region, &mut rng, &mut entities);
        assert_eq!(region.get(BlockPos::new(4, 5, 4)), fence);
        assert_eq!(region.get(BlockPos::new(5, 5, 4)), fence);
    }

    #[test]
    fn a_template_water_source_is_locked_and_a_candle_goes_out() {
        let fence = dry_fence();
        let candle = with(CANDLE, &[("lit", "true"), ("waterlogged", "false")]);
        let template = FrozenTemplate {
            size: [4, 1, 1],
            palettes: vec![
                vec![
                    block([0, 0, 0], WATER),
                    block([1, 0, 0], fence),
                    block([3, 0, 0], candle),
                ]
                .into_boxed_slice(),
            ],
        };
        let chain = vec![];
        let mut p = placement(&template, &chain, Rotation::None, None);
        p.waterlog = true;
        let mut region = new_region();
        region.blocks.set(BlockPos::new(7, 5, 4), WATER);
        let mut rng = XoroshiroRandom::new(1);
        let mut entities = Vec::new();
        place_template(&p, &mut region, &mut rng, &mut entities);
        assert_eq!(
            region.get(BlockPos::new(5, 5, 4)),
            fence,
            "the template's own water does not flood the fence"
        );
        assert_eq!(
            region.get(BlockPos::new(7, 5, 4)),
            with(CANDLE, &[("lit", "false"), ("waterlogged", "true")])
        );
    }

    #[test]
    fn a_double_slab_refuses_water() {
        let slab = with(SLAB, &[("type", "double"), ("waterlogged", "false")]);
        let template = FrozenTemplate {
            size: [1, 1, 1],
            palettes: vec![vec![block([0, 0, 0], slab)].into_boxed_slice()],
        };
        let chain = vec![];
        let mut p = placement(&template, &chain, Rotation::None, None);
        p.waterlog = true;
        let mut region = new_region();
        region.blocks.set(BlockPos::new(4, 5, 4), WATER);
        let mut rng = XoroshiroRandom::new(1);
        place_template(&p, &mut region, &mut rng, &mut Vec::new());
        assert_eq!(region.get(BlockPos::new(4, 5, 4)), slab);
    }

    #[test]
    fn block_rot_draws_only_for_a_rottable_block() {
        let template = template();
        let integrity = 0.5;
        let chain = vec![CompiledProcessor::BlockRot {
            rottable: Some(mask_of([DIRT.0])),
            integrity,
        }];
        let (region, ..) = place(&placement(&template, &chain, Rotation::None, None));
        let kept_dirt = region.writes.iter().filter(|(_, s)| *s == DIRT).count();
        let expected = (0..9)
            .filter(|i| {
                let pos = IVec3::new(4 + i % 3, 5, 4 + i / 3);
                LegacyRandom::new(block_pos_seed(pos)).next_f32() <= integrity
            })
            .count();
        assert_eq!(kept_dirt, expected);
        assert_eq!(
            region.writes.iter().filter(|(_, s)| *s == STONE).count(),
            2,
            "stone is not rottable and never drawn for"
        );
    }

    #[test]
    fn protected_blocks_keep_what_the_world_holds() {
        let template = template();
        let chain = vec![CompiledProcessor::ProtectedBlocks(mask_of([STONE.0]))];
        let mut region = new_region();
        region.blocks.set(BlockPos::new(4, 5, 4), STONE);
        let mut rng = XoroshiroRandom::new(1);
        place_template(
            &placement(&template, &chain, Rotation::None, None),
            &mut region,
            &mut rng,
            &mut Vec::new(),
        );
        assert_eq!(region.get(BlockPos::new(4, 5, 4)), STONE);
        assert_eq!(region.writes.len(), template.palettes[0].len() - 1);
    }

    #[test]
    fn a_position_rule_scales_its_chance_with_the_distance() {
        let template = template();
        let position = PosRule {
            linear: LinearPos {
                min_chance: 0.0,
                max_chance: 1.0,
                min_dist: 0,
                max_dist: 4,
            },
            axis: None,
        };
        let chain = vec![CompiledProcessor::Rule(vec![CompiledRule {
            input: Rule::MatchingStates(mask_of([DIRT.0])),
            location: Rule::AlwaysTrue,
            position: Some(position.clone()),
            output: STONE,
            modifier: None,
        }])];
        let (region, ..) = place(&placement(&template, &chain, Rotation::None, None));
        for (pos, state) in region.writes.iter().filter(|(pos, _)| pos.y == 5) {
            let mut rng = LegacyRandom::new(block_pos_seed(**pos));
            let expected = if position.test(**pos, IVec3::new(4, 5, 4), &mut rng) {
                STONE
            } else {
                DIRT
            };
            assert_eq!(*state, expected, "at {pos}");
        }
        let origin = region
            .writes
            .iter()
            .find(|(pos, _)| *pos == BlockPos::new(4, 5, 4));
        assert_eq!(origin.unwrap().1, DIRT, "distance zero has chance zero");
    }
}
