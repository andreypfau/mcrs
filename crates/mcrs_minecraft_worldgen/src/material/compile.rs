use crate::compile::{CompileError, Compiler};
use crate::material::proto::{
    BiomeSet, CaveSurface, MaterialCondition, MaterialConditionHolder, MaterialRule,
    MaterialRuleHolder,
};
use crate::noise::stack::{NoiseStack, Octave};
use crate::program::NodeId;
use crate::proto::{BlockState, HashableF64, NoiseHolder};
use crate::router::NoiseGeneratorSettings;
use crate::value_provider::HeightContext;
use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_random::{Random, RandomSource};
use mcrs_voxel_storage::VoxelId;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub type CondId = u32;
pub type NoiseId = u32;
pub type BiomeSetId = u32;
pub type VeinId = u32;
pub type RandomId = u32;

pub const CLAY_BAND_COUNT: usize = 192;

/// The nine noises the stage samples outside the density graph. The reference
/// hardcodes them, so no datapack file names them and nothing else would load
/// them.
pub const SURFACE_NOISE_NAMES: [&str; 9] = [
    "surface",
    "surface_secondary",
    "clay_bands_offset",
    "badlands_pillar",
    "badlands_pillar_roof",
    "badlands_surface",
    "iceberg_pillar",
    "iceberg_pillar_roof",
    "iceberg_surface",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Guard { condition: CondId, skip_to: u32 },
    Block { state: VoxelId },
    Bandlands,
    OreVein { vein: VeinId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Xz,
    Y,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Condition {
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        ceiling: bool,
    },
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    YAbove {
        anchor: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Biome {
        set: BiomeSetId,
    },
    NoiseThreshold {
        noise: NoiseId,
        min: HashableF64,
        max: HashableF64,
        is_3d: bool,
    },
    VerticalGradient {
        random: RandomId,
        true_at_and_below: i32,
        false_at_and_above: i32,
    },
    Steep,
    Hole,
    AbovePreliminarySurface,
    Not(CondId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledCondition {
    pub kind: Condition,
    pub scope: Scope,
}

/// Root indices into the shared program, never raw node ids: a node that is not
/// registered as a root has no fill plan, and the prune after compilation would
/// renumber it into an unrelated node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OreVein {
    pub ore: VoxelId,
    pub raw_ore: VoxelId,
    pub filler: VoxelId,
    pub raw_ore_chance: f32,
    pub density: usize,
    pub richness: usize,
    pub filler_gap: usize,
    pub random: RandomId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tri {
    Never,
    Maybe,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BiomeMask(Box<[u64]>);

impl BiomeMask {
    fn new(ids: &[u32]) -> Self {
        let words = ids
            .iter()
            .map(|id| *id as usize / 64 + 1)
            .max()
            .unwrap_or(0);
        let mut bits = vec![0u64; words];
        for id in ids {
            bits[*id as usize / 64] |= 1 << (*id % 64);
        }
        BiomeMask(bits.into_boxed_slice())
    }

    #[inline]
    pub fn contains(&self, biome: u32) -> bool {
        let word = biome as usize / 64;
        word < self.0.len() && self.0[word] >> (biome % 64) & 1 != 0
    }

    /// How this set answers for a column that can select exactly `biomes`.
    /// `Never` and `Always` settle the condition statically for the column.
    pub fn fold(&self, biomes: impl IntoIterator<Item = u32>) -> Tri {
        let mut any = false;
        let mut all = true;
        for biome in biomes {
            if self.contains(biome) {
                any = true;
            } else {
                all = false;
            }
        }
        match (any, all) {
            (false, _) => Tri::Never,
            (true, true) => Tri::Always,
            (true, false) => Tri::Maybe,
        }
    }
}

/// What the tape needs from the position under rewrite. The tape walk itself is
/// [`MaterialProgram::run`]; everything it cannot answer from the instruction
/// alone comes from here.
pub trait MaterialContext {
    fn test(&mut self, condition: CondId) -> bool;
    fn bandlands(&mut self) -> VoxelId;
    fn ore_vein(&mut self, vein: VeinId) -> Option<VoxelId>;
}

pub struct MaterialProgram {
    tape: Box<[Op]>,
    conditions: Box<[CompiledCondition]>,
    noises: Box<[Arc<NoiseStack<Octave>>]>,
    biome_sets: Box<[BiomeMask]>,
    veins: Box<[OreVein]>,
    randoms: Box<[RandomSource]>,
    noise_random: RandomSource,
    clay_bands: Box<[VoxelId]>,
    surface_noises: [NoiseId; 9],
    noise_ids: HashMap<(ResourceLocation, bool), NoiseId>,
    random_ids: HashMap<ResourceLocation, RandomId>,
}

impl MaterialProgram {
    pub fn run(&self, context: &mut impl MaterialContext) -> Option<VoxelId> {
        let mut pc = 0usize;
        while let Some(op) = self.tape.get(pc) {
            match *op {
                Op::Guard { condition, skip_to } => {
                    if context.test(condition) {
                        pc += 1;
                    } else {
                        pc = skip_to as usize;
                    }
                }
                Op::Block { state } => return Some(state),
                Op::Bandlands => return Some(context.bandlands()),
                Op::OreVein { vein } => {
                    if let Some(state) = context.ore_vein(vein) {
                        return Some(state);
                    }
                    pc += 1;
                }
            }
        }
        None
    }

    pub fn tape(&self) -> &[Op] {
        &self.tape
    }

    pub fn conditions(&self) -> &[CompiledCondition] {
        &self.conditions
    }

    pub fn biome_sets(&self) -> &[BiomeMask] {
        &self.biome_sets
    }

    pub fn veins(&self) -> &[OreVein] {
        &self.veins
    }

    pub fn noise(&self, noise: NoiseId) -> &NoiseStack<Octave> {
        &self.noises[noise as usize]
    }

    pub fn noise_count(&self) -> usize {
        self.noises.len()
    }

    pub fn noise_id(&self, name: &ResourceLocation, is_3d: bool) -> Option<NoiseId> {
        self.noise_ids.get(&(name.clone(), is_3d)).copied()
    }

    pub fn random_id(&self, name: &ResourceLocation) -> Option<RandomId> {
        self.random_ids.get(name).copied()
    }

    pub fn surface_noise(&self, noise: SurfaceNoise) -> NoiseId {
        self.surface_noises[noise as usize]
    }

    /// The 192-entry table the `bandlands` rule indexes by Y plus the
    /// `clay_bands_offset` noise.
    pub fn clay_bands(&self) -> &[VoxelId] {
        &self.clay_bands
    }

    pub fn random_at(&self, random: RandomId, pos: IVec3) -> RandomSource {
        self.randoms[random as usize].clone().fork_at(pos)
    }

    /// The unnamed stream the surface depth and the icebergs draw from.
    pub fn noise_random_at(&self, pos: IVec3) -> RandomSource {
        self.noise_random.clone().fork_at(pos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceNoise {
    Surface,
    SurfaceSecondary,
    ClayBandsOffset,
    BadlandsPillar,
    BadlandsPillarRoof,
    BadlandsSurface,
    IcebergPillar,
    IcebergPillarRoof,
    IcebergSurface,
}

/// The two registries plus the two resolvers this crate cannot supply itself:
/// it has neither a block registry nor a biome registry.
pub struct MaterialInputs<'a> {
    pub rules: &'a BTreeMap<ResourceLocation, MaterialRuleHolder>,
    pub conditions: &'a BTreeMap<ResourceLocation, MaterialConditionHolder>,
    pub block: &'a dyn Fn(&BlockState) -> Option<VoxelId>,
    pub biome: &'a dyn Fn(&ResourceLocation) -> Option<u32>,
}

pub(crate) fn compile_material<'a>(
    compiler: &mut Compiler<'_>,
    roots: &mut Vec<NodeId>,
    settings: &NoiseGeneratorSettings,
    inputs: &'a MaterialInputs<'a>,
) -> Result<MaterialProgram, CompileError> {
    let mut builder = Builder {
        compiler,
        roots,
        inputs,
        height: HeightContext {
            min_y: settings.noise.min_y,
            depth: settings.noise.height as i32,
            sea_level: settings.sea_level,
        },
        tape: Vec::new(),
        conditions: Vec::new(),
        interned: HashMap::new(),
        noises: Vec::new(),
        noise_ids: HashMap::new(),
        biome_sets: Vec::new(),
        biome_set_ids: HashMap::new(),
        randoms: Vec::new(),
        random_ids: HashMap::new(),
        veins: Vec::new(),
        rule_stack: Vec::new(),
        condition_stack: Vec::new(),
    };

    let root = inputs
        .rules
        .get(&settings.material_rule)
        .ok_or_else(|| CompileError::UnknownRule(settings.material_rule.as_str().to_string()))?;
    builder.rule(root)?;

    let mut surface_noises = [0; 9];
    for (slot, name) in surface_noises.iter_mut().zip(SURFACE_NOISE_NAMES) {
        *slot = builder.noise(&ResourceLocation::minecraft(name), false)?;
    }
    let clay_bands = builder.clay_bands()?;

    Ok(MaterialProgram {
        tape: builder.tape.into_boxed_slice(),
        conditions: builder.conditions.into_boxed_slice(),
        noises: builder.noises.into_boxed_slice(),
        biome_sets: builder.biome_sets.into_boxed_slice(),
        veins: builder.veins.into_boxed_slice(),
        randoms: builder.randoms.into_boxed_slice(),
        noise_random: builder.compiler.positional_random(),
        clay_bands,
        surface_noises,
        noise_ids: builder.noise_ids,
        random_ids: builder.random_ids,
    })
}

struct Builder<'a, 'c, 'r> {
    compiler: &'a mut Compiler<'c>,
    roots: &'a mut Vec<NodeId>,
    inputs: &'r MaterialInputs<'r>,
    height: HeightContext,
    tape: Vec<Op>,
    conditions: Vec<CompiledCondition>,
    interned: HashMap<Condition, CondId>,
    noises: Vec<Arc<NoiseStack<Octave>>>,
    noise_ids: HashMap<(ResourceLocation, bool), NoiseId>,
    biome_sets: Vec<BiomeMask>,
    biome_set_ids: HashMap<BiomeMask, BiomeSetId>,
    randoms: Vec<RandomSource>,
    random_ids: HashMap<ResourceLocation, RandomId>,
    veins: Vec<OreVein>,
    rule_stack: Vec<ResourceLocation>,
    condition_stack: Vec<ResourceLocation>,
}

impl<'r> Builder<'_, '_, 'r> {
    fn rule(&mut self, holder: &'r MaterialRuleHolder) -> Result<(), CompileError> {
        let depth = self.rule_stack.len();
        let rule = self.resolve_rule(holder)?;
        match rule {
            MaterialRule::Block { result_state } => {
                let state = self.block(result_state)?;
                self.tape.push(Op::Block { state });
            }
            MaterialRule::Sequence { sequence } => {
                for member in sequence {
                    self.rule(member)?;
                }
            }
            MaterialRule::Condition { if_true, then_run } => {
                let condition = self.condition(if_true)?;
                let guard = self.tape.len();
                self.tape.push(Op::Guard {
                    condition,
                    skip_to: 0,
                });
                self.rule(then_run)?;
                let end = self.tape.len() as u32;
                self.tape[guard] = Op::Guard {
                    condition,
                    skip_to: end,
                };
            }
            MaterialRule::Bandlands => self.tape.push(Op::Bandlands),
            MaterialRule::OreVein {
                ore_block,
                raw_ore_block,
                filler_block,
                raw_ore_chance,
                density,
                richness,
                filler_gap,
            } => {
                let vein = OreVein {
                    ore: self.block(ore_block)?,
                    raw_ore: self.block(raw_ore_block)?,
                    filler: self.block(filler_block)?,
                    raw_ore_chance: raw_ore_chance.0 as f32,
                    density: self.density_root(density)?,
                    richness: self.density_root(richness)?,
                    filler_gap: self.density_root(filler_gap)?,
                    random: self.random(&ResourceLocation::minecraft("ore")),
                };
                let id = self.veins.len() as VeinId;
                self.veins.push(vein);
                self.tape.push(Op::OreVein { vein: id });
            }
        }
        self.rule_stack.truncate(depth);
        Ok(())
    }

    fn resolve_rule(
        &mut self,
        holder: &'r MaterialRuleHolder,
    ) -> Result<&'r MaterialRule, CompileError> {
        let rules = self.inputs.rules;
        let mut holder = holder;
        loop {
            match holder {
                MaterialRuleHolder::Owned(rule) => return Ok(rule),
                MaterialRuleHolder::Reference(id) => {
                    if self.rule_stack.contains(id) {
                        return Err(CompileError::ReferenceCycle(id.as_str().to_string()));
                    }
                    self.rule_stack.push(id.clone());
                    holder = rules
                        .get(id)
                        .ok_or_else(|| CompileError::UnknownRule(id.as_str().to_string()))?;
                }
            }
        }
    }

    fn condition(&mut self, holder: &'r MaterialConditionHolder) -> Result<CondId, CompileError> {
        let depth = self.condition_stack.len();
        let condition = self.resolve_condition(holder)?;
        let (kind, scope) = match condition {
            MaterialCondition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                surface_type,
            } => (
                Condition::StoneDepth {
                    offset: *offset,
                    add_surface_depth: *add_surface_depth,
                    secondary_depth_range: *secondary_depth_range,
                    ceiling: matches!(surface_type, CaveSurface::Ceiling),
                },
                Scope::Y,
            ),
            MaterialCondition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => (
                Condition::Water {
                    offset: *offset,
                    surface_depth_multiplier: *surface_depth_multiplier,
                    add_stone_depth: *add_stone_depth,
                },
                Scope::Y,
            ),
            MaterialCondition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => (
                Condition::YAbove {
                    anchor: anchor.resolve_y(self.height),
                    surface_depth_multiplier: *surface_depth_multiplier,
                    add_stone_depth: *add_stone_depth,
                },
                Scope::Y,
            ),
            MaterialCondition::Biome { biome_is } => (
                Condition::Biome {
                    set: self.biome_set(biome_is)?,
                },
                Scope::Y,
            ),
            MaterialCondition::NoiseThreshold {
                noise,
                min_threshold,
                max_threshold,
                is_3d,
            } => (
                Condition::NoiseThreshold {
                    noise: self.noise(noise, *is_3d)?,
                    min: *min_threshold,
                    max: *max_threshold,
                    is_3d: *is_3d,
                },
                if *is_3d { Scope::Y } else { Scope::Xz },
            ),
            MaterialCondition::VerticalGradient {
                random_name,
                true_at_and_below,
                false_at_and_above,
            } => (
                Condition::VerticalGradient {
                    random: self.random(random_name),
                    true_at_and_below: true_at_and_below.resolve_y(self.height),
                    false_at_and_above: false_at_and_above.resolve_y(self.height),
                },
                Scope::Y,
            ),
            MaterialCondition::Steep => (Condition::Steep, Scope::Xz),
            MaterialCondition::Hole => (Condition::Hole, Scope::Xz),
            MaterialCondition::AbovePreliminarySurface => {
                (Condition::AbovePreliminarySurface, Scope::Y)
            }
            MaterialCondition::Not { invert } => {
                let inner = self.condition(invert)?;
                let scope = self.conditions[inner as usize].scope;
                (Condition::Not(inner), scope)
            }
            MaterialCondition::Temperature => {
                return Err(CompileError::UnsupportedCondition("temperature"));
            }
        };
        self.condition_stack.truncate(depth);
        Ok(self.intern(kind, scope))
    }

    fn resolve_condition(
        &mut self,
        holder: &'r MaterialConditionHolder,
    ) -> Result<&'r MaterialCondition, CompileError> {
        let registry = self.inputs.conditions;
        let mut holder = holder;
        loop {
            match holder {
                MaterialConditionHolder::Owned(condition) => return Ok(condition),
                MaterialConditionHolder::Reference(id) => {
                    if self.condition_stack.contains(id) {
                        return Err(CompileError::ReferenceCycle(id.as_str().to_string()));
                    }
                    self.condition_stack.push(id.clone());
                    holder = registry
                        .get(id)
                        .ok_or_else(|| CompileError::UnknownCondition(id.as_str().to_string()))?;
                }
            }
        }
    }

    fn intern(&mut self, kind: Condition, scope: Scope) -> CondId {
        if let Some(&id) = self.interned.get(&kind) {
            return id;
        }
        let id = self.conditions.len() as CondId;
        self.interned.insert(kind.clone(), id);
        self.conditions.push(CompiledCondition { kind, scope });
        id
    }

    fn block(&mut self, state: &BlockState) -> Result<VoxelId, CompileError> {
        (self.inputs.block)(state)
            .ok_or_else(|| CompileError::UnknownBlockState(state.name.as_str().to_string()))
    }

    fn biome_set(&mut self, set: &BiomeSet) -> Result<BiomeSetId, CompileError> {
        let mut ids = Vec::with_capacity(set.ids().len());
        for name in set.ids() {
            ids.push(
                (self.inputs.biome)(name)
                    .ok_or_else(|| CompileError::UnknownBiome(name.as_str().to_string()))?,
            );
        }
        let mask = BiomeMask::new(&ids);
        if let Some(&id) = self.biome_set_ids.get(&mask) {
            return Ok(id);
        }
        let id = self.biome_sets.len() as BiomeSetId;
        self.biome_set_ids.insert(mask.clone(), id);
        self.biome_sets.push(mask);
        Ok(id)
    }

    /// Each dimensionality gets its own id: the descent's cache is one slot per id,
    /// and a 2D reading is stamped against a different scope than a 3D one.
    fn noise(&mut self, id: &ResourceLocation, is_3d: bool) -> Result<NoiseId, CompileError> {
        if let Some(&noise) = self.noise_ids.get(&(id.clone(), is_3d)) {
            return Ok(noise);
        }
        let sampler = self
            .compiler
            .noise_sampler(&NoiseHolder::Reference(id.clone()))?;
        let noise = self.noises.len() as NoiseId;
        self.noises.push(sampler);
        self.noise_ids.insert((id.clone(), is_3d), noise);
        Ok(noise)
    }

    fn random(&mut self, name: &ResourceLocation) -> RandomId {
        if let Some(&id) = self.random_ids.get(name) {
            return id;
        }
        let id = self.randoms.len() as RandomId;
        self.randoms
            .push(self.compiler.hashed_random(name.as_str()));
        self.random_ids.insert(name.clone(), id);
        id
    }

    fn density_root(
        &mut self,
        holder: &crate::proto::DensityFunctionHolder,
    ) -> Result<usize, CompileError> {
        let node = self.compiler.compile_root(holder)?;
        let root = self.roots.len();
        self.roots.push(node);
        Ok(root)
    }

    fn clay_bands(&mut self) -> Result<Box<[VoxelId]>, CompileError> {
        let colours = ClayBandColours {
            terracotta: self.named_block("terracotta")?,
            orange: self.named_block("orange_terracotta")?,
            yellow: self.named_block("yellow_terracotta")?,
            brown: self.named_block("brown_terracotta")?,
            red: self.named_block("red_terracotta")?,
            white: self.named_block("white_terracotta")?,
            light_gray: self.named_block("light_gray_terracotta")?,
        };
        let mut random = self.compiler.hashed_random("minecraft:clay_bands");
        Ok(generate_clay_bands(&mut random, &colours))
    }

    fn named_block(&mut self, name: &str) -> Result<VoxelId, CompileError> {
        self.block(&BlockState {
            name: ResourceLocation::minecraft(name),
            properties: None,
        })
    }
}

struct ClayBandColours {
    terracotta: VoxelId,
    orange: VoxelId,
    yellow: VoxelId,
    brown: VoxelId,
    red: VoxelId,
    white: VoxelId,
    light_gray: VoxelId,
}

fn generate_clay_bands(random: &mut RandomSource, colours: &ClayBandColours) -> Box<[VoxelId]> {
    let mut bands = vec![colours.terracotta; CLAY_BAND_COUNT];
    let count = CLAY_BAND_COUNT as i32;

    let mut i = 0;
    while i < count {
        i += random.next_i32_bound(5) + 1;
        if i < count {
            bands[i as usize] = colours.orange;
        }
        i += 1;
    }

    make_bands(random, &mut bands, 1, colours.yellow);
    make_bands(random, &mut bands, 2, colours.brown);
    make_bands(random, &mut bands, 1, colours.red);

    let white_bands = next_i32_between(random, 9, 15);
    let mut drawn = 0;
    let mut start = 0;
    while drawn < white_bands && start < count {
        bands[start as usize] = colours.white;
        if start - 1 > 0 && random.next_bool() {
            bands[start as usize - 1] = colours.light_gray;
        }
        if start + 1 < count && random.next_bool() {
            bands[start as usize + 1] = colours.light_gray;
        }
        drawn += 1;
        start += random.next_i32_bound(16) + 4;
    }

    bands.into_boxed_slice()
}

fn make_bands(random: &mut RandomSource, bands: &mut [VoxelId], base_width: i32, state: VoxelId) {
    let count = next_i32_between(random, 6, 15);
    for _ in 0..count {
        let width = base_width + random.next_i32_bound(3);
        let start = random.next_i32_bound(bands.len() as i32);
        for p in 0..width {
            let at = (start + p) as usize;
            if at >= bands.len() {
                break;
            }
            bands[at] = state;
        }
    }
}

fn next_i32_between(random: &mut RandomSource, min: i32, max_inclusive: i32) -> i32 {
    random.next_i32_bound(max_inclusive - min + 1) + min
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::compile::build_router;
    use crate::compile::tests::{assets, corpus, load_dir};
    use crate::program::Workspace;
    use crate::volume::Volume;
    use std::collections::BTreeSet;

    pub(crate) fn material_corpus() -> (
        BTreeMap<ResourceLocation, MaterialRuleHolder>,
        BTreeMap<ResourceLocation, MaterialConditionHolder>,
    ) {
        let mut rules = BTreeMap::new();
        let root = assets().join("material_rule");
        load_dir(&root, &root, &mut rules);
        let mut conditions = BTreeMap::new();
        let root = assets().join("material_condition");
        load_dir(&root, &root, &mut conditions);
        (rules, conditions)
    }

    /// Stands in for the block and biome registries the compiling crate does not
    /// have: any well-formed id resolves, to a distinct id per name.
    pub(crate) fn resolve_block(state: &BlockState) -> Option<VoxelId> {
        Some(VoxelId(hash_id(state.name.as_str()) as u16))
    }

    pub(crate) fn resolve_biome(name: &ResourceLocation) -> Option<u32> {
        Some(hash_id(name.as_str()) % 256)
    }

    fn hash_id(name: &str) -> u32 {
        name.bytes()
            .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
    }

    pub(crate) fn settings(name: &str) -> NoiseGeneratorSettings {
        let path = assets().join(format!("noise_settings/{name}.json"));
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap()
    }

    pub(crate) fn build(name: &str) -> crate::router::NoiseRouter {
        let (functions, noises) = corpus();
        let (rules, conditions) = material_corpus();
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &resolve_block,
            biome: &resolve_biome,
        };
        build_router(
            &settings(name),
            &functions,
            &noises,
            42,
            VoxelId(1),
            VoxelId(2),
            Some(&inputs),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    #[test]
    fn every_shipped_dimension_compiles_its_material_rule() {
        for name in ["overworld", "nether", "end", "caves", "floating_islands"] {
            let router = build(name);
            let material = router.material().expect("a material program");
            assert!(!material.tape().is_empty(), "{name}: the tape is empty");
        }
    }

    #[test]
    fn every_guard_jumps_to_the_end_of_its_own_subtree() {
        let router = build("overworld");
        let material = router.material().unwrap();
        let tape = material.tape();
        // A well-formed tape is a set of properly nested intervals: walking a
        // guard's body must land exactly on its jump target, never past it.
        fn walk(tape: &[Op], mut pc: usize, end: usize) {
            while pc < end {
                match tape[pc] {
                    Op::Guard { skip_to, .. } => {
                        let skip_to = skip_to as usize;
                        assert!(
                            skip_to > pc && skip_to <= end,
                            "guard at {pc} jumps to {skip_to}, outside [{pc}, {end}]"
                        );
                        walk(tape, pc + 1, skip_to);
                        pc = skip_to;
                    }
                    _ => pc += 1,
                }
            }
            assert_eq!(pc, end, "a subtree overshot its end at {end}");
        }
        walk(tape, 0, tape.len());
        assert!(tape.iter().any(|op| matches!(op, Op::Guard { .. })));
    }

    /// The shipped `material_condition` registry holds six distinct
    /// `stone_depth` shapes and two `water` shapes, and every occurrence of
    /// either kind in the rule corpus is a reference to one of them.
    #[test]
    fn the_shared_condition_files_intern_to_one_id_each() {
        let router = build("overworld");
        let material = router.material().unwrap();

        let kind_of = |id: CondId| &material.conditions()[id as usize].kind;
        let interned = |wanted: fn(&Condition) -> bool| {
            material
                .conditions()
                .iter()
                .filter(|condition| wanted(&condition.kind))
                .count()
        };
        let guards_on = |wanted: fn(&Condition) -> bool| {
            material
                .tape()
                .iter()
                .filter(|op| match op {
                    Op::Guard { condition, .. } => match kind_of(*condition) {
                        Condition::Not(inner) => wanted(kind_of(*inner)),
                        other => wanted(other),
                    },
                    _ => false,
                })
                .count()
        };

        let stone_depths = interned(|c| matches!(c, Condition::StoneDepth { .. }));
        let waters = interned(|c| matches!(c, Condition::Water { .. }));
        assert!(stone_depths <= 6, "{stone_depths} stone_depth conditions");
        assert!(waters <= 2, "{waters} water conditions");
        assert!(
            guards_on(|c| matches!(c, Condition::StoneDepth { .. } | Condition::Water { .. }))
                >= 3 * (stone_depths + waters),
            "the shared conditions are not shared often enough for this to prove anything"
        );

        let guards = material
            .tape()
            .iter()
            .filter(|op| matches!(op, Op::Guard { .. }))
            .count();
        assert!(
            material.conditions().len() * 2 < guards,
            "{} conditions for {guards} guards is not interning",
            material.conditions().len()
        );

        let distinct: BTreeSet<_> = material
            .conditions()
            .iter()
            .map(|condition| format!("{:?}", condition.kind))
            .collect();
        assert_eq!(
            distinct.len(),
            material.conditions().len(),
            "two condition slots hold the same structure"
        );
    }

    #[test]
    fn each_condition_kind_carries_the_scope_the_design_assigns_it() {
        let router = build("overworld");
        let material = router.material().unwrap();
        assert!(!material.conditions().is_empty());
        for condition in material.conditions() {
            let expected = match &condition.kind {
                Condition::Steep | Condition::Hole => Scope::Xz,
                Condition::NoiseThreshold { is_3d, .. } => {
                    if *is_3d {
                        Scope::Y
                    } else {
                        Scope::Xz
                    }
                }
                Condition::StoneDepth { .. }
                | Condition::Water { .. }
                | Condition::YAbove { .. }
                | Condition::Biome { .. }
                | Condition::VerticalGradient { .. }
                | Condition::AbovePreliminarySurface => Scope::Y,
                Condition::Not(inner) => material.conditions()[*inner as usize].scope,
            };
            assert_eq!(
                condition.scope, expected,
                "wrong scope for {:?}",
                condition.kind
            );
        }

        let has_3d = material
            .conditions()
            .iter()
            .any(|c| matches!(c.kind, Condition::NoiseThreshold { is_3d: true, .. }));
        let has_2d = material
            .conditions()
            .iter()
            .any(|c| matches!(c.kind, Condition::NoiseThreshold { is_3d: false, .. }));
        assert!(
            has_2d && has_3d,
            "the corpus has both noise_threshold forms"
        );
        assert!(
            material
                .conditions()
                .iter()
                .any(|c| matches!(c.kind, Condition::AbovePreliminarySurface)),
            "the overworld rule uses above_preliminary_surface"
        );
    }

    #[test]
    fn every_vein_density_function_is_a_root_and_samples() {
        let router = build("overworld");
        let material = router.material().unwrap();
        assert_eq!(material.veins().len(), 2, "copper and iron");

        let mut workspace = Workspace::new();
        let volume = Volume::point(IVec3::new(9, 40, -13));
        let mut out = [0.0f32];
        for vein in material.veins() {
            for root in [vein.density, vein.richness, vein.filler_gap] {
                assert!(root >= 8, "vein roots come after the eight terrain roots");
                router
                    .program()
                    .fill(&mut workspace, &volume, root, &mut out);
                assert!(out[0].is_finite(), "root {root} is not finite");
            }
        }
    }

    /// Golden strings transcribed from a reference run of the band table over
    /// the legacy random stream, which `LegacyRandom` matches bit for bit. Any
    /// slip in the draw order moves the whole tail.
    #[test]
    fn the_band_table_matches_the_reference_draw_order() {
        let colours = ClayBandColours {
            terracotta: VoxelId(0),
            orange: VoxelId(1),
            yellow: VoxelId(2),
            brown: VoxelId(3),
            red: VoxelId(4),
            white: VoxelId(5),
            light_gray: VoxelId(6),
        };
        for (seed, expected) in [
            (
                0i64,
                "540000222000100650133301054401000401334433563300010004406560100000100333101651222000145601050001500100010100001000100012220101220333044122240331010022203301033121010333001004440001001002200330",
            ),
            (
                1,
                "560000100014465400000100000122445633100051000010005033302200065004100010200334401533430100500100100026560065633000133000100001440033301330401000001010044400101000001410100000101010004400104410",
            ),
            (
                123,
                "560101004445603656001000041656102256010033010010001562444365004015100000100103322053331025200222001033365041001000001522000444300010001056220100000121001000010001224410100033144400001010022101",
            ),
            (
                -42,
                "510033561001333300105133300056033330122222205100001001440654010050142256010101000501000444443332203356102225022200330522206561000651000010222001000105010000101010001010044100000133333333333222",
            ),
            (
                5217580947566544282,
                "500010533010050334440133330102265124400010000010100561001000001000656000101014443330056220010065220105644222651022222225000100100105001003331044400010000122201010010000133300200144401000440001",
            ),
        ] {
            let mut random = RandomSource::new(seed as u64, true);
            let bands = generate_clay_bands(&mut random, &colours);
            let drawn: String = bands.iter().map(|state| state.0.to_string()).collect();
            assert_eq!(drawn, expected, "seed {seed}");
        }
    }

    #[test]
    fn the_clay_bands_are_drawn_once_and_are_not_uniform() {
        let router = build("overworld");
        let bands = router.material().unwrap().clay_bands();
        assert_eq!(bands.len(), CLAY_BAND_COUNT);
        let distinct: BTreeSet<_> = bands.iter().collect();
        assert!(
            distinct.len() >= 5,
            "only {} distinct band states",
            distinct.len()
        );
    }

    #[test]
    fn a_biome_set_folds_over_the_columns_biomes() {
        let mask = BiomeMask::new(&[3, 70]);
        assert_eq!(mask.fold([3, 70]), Tri::Always);
        assert_eq!(mask.fold([3, 4]), Tri::Maybe);
        assert_eq!(mask.fold([4, 5]), Tri::Never);
        assert_eq!(mask.fold([]), Tri::Never);
        assert!(!mask.contains(4096));
    }

    fn compile_alone(
        rule: &str,
        block: &dyn Fn(&BlockState) -> Option<VoxelId>,
    ) -> Result<(), CompileError> {
        let (functions, noises) = corpus();
        let (mut rules, conditions) = material_corpus();
        rules.insert(
            ResourceLocation::minecraft("overworld"),
            serde_json::from_str(rule).unwrap(),
        );
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block,
            biome: &resolve_biome,
        };
        build_router(
            &settings("overworld"),
            &functions,
            &noises,
            42,
            VoxelId(1),
            VoxelId(2),
            Some(&inputs),
        )
        .map(|_| ())
    }

    #[test]
    fn an_unresolvable_block_state_is_a_compile_error() {
        let error = compile_alone(
            r#"{"type":"minecraft:block","result_state":"minecraft:nonesuch"}"#,
            &|state| (state.name.as_str() != "minecraft:nonesuch").then_some(VoxelId(7)),
        )
        .unwrap_err();
        assert_eq!(
            error,
            CompileError::UnknownBlockState("minecraft:nonesuch".to_string())
        );
    }

    #[test]
    fn an_unknown_rule_id_is_a_compile_error() {
        let error = compile_alone(
            r#"{"type":"minecraft:sequence","sequence":["minecraft:nonesuch"]}"#,
            &resolve_block,
        )
        .unwrap_err();
        assert_eq!(
            error,
            CompileError::UnknownRule("minecraft:nonesuch".to_string())
        );
    }

    #[test]
    fn an_unknown_rule_type_does_not_parse() {
        let json = r#"{"type":"minecraft:teleport","result_state":"minecraft:stone"}"#;
        let error = serde_json::from_str::<MaterialRule>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains("teleport"), "{error}");
        assert!(serde_json::from_str::<MaterialRuleHolder>(json).is_err());
    }

    #[test]
    fn the_temperature_condition_names_itself_as_unsupported() {
        let error = compile_alone(
            r#"{"type":"minecraft:condition","if_true":{"type":"minecraft:temperature"},"then_run":{"type":"minecraft:block","result_state":"minecraft:stone"}}"#,
            &resolve_block,
        )
        .unwrap_err();
        assert_eq!(error, CompileError::UnsupportedCondition("temperature"));
        assert!(error.to_string().contains("temperature"));
    }

    #[test]
    fn a_rule_that_references_itself_is_a_cycle() {
        let error = compile_alone(
            r#"{"type":"minecraft:sequence","sequence":["minecraft:overworld"]}"#,
            &resolve_block,
        )
        .unwrap_err();
        assert!(
            matches!(error, CompileError::ReferenceCycle(_)),
            "{error:?}"
        );
    }

    #[test]
    fn the_nine_surface_noises_are_interned_and_present() {
        let router = build("overworld");
        let material = router.material().unwrap();
        let ids = [
            SurfaceNoise::Surface,
            SurfaceNoise::SurfaceSecondary,
            SurfaceNoise::ClayBandsOffset,
            SurfaceNoise::BadlandsPillar,
            SurfaceNoise::BadlandsPillarRoof,
            SurfaceNoise::BadlandsSurface,
            SurfaceNoise::IcebergPillar,
            SurfaceNoise::IcebergPillarRoof,
            SurfaceNoise::IcebergSurface,
        ]
        .map(|noise| material.surface_noise(noise));
        assert_eq!(
            ids.iter().collect::<BTreeSet<_>>().len(),
            9,
            "nine distinct samplers"
        );
        for id in ids {
            assert!(material.noise(id).get(1.0, 2.0, 3.0).is_finite());
        }

        // `minecraft:surface` is also named by a noise_threshold condition, so
        // it must be the same interned sampler, not a second instance.
        let threshold = material.conditions().iter().find_map(|c| match c.kind {
            Condition::NoiseThreshold { noise, .. }
                if noise == material.surface_noise(SurfaceNoise::Surface) =>
            {
                Some(noise)
            }
            _ => None,
        });
        assert!(threshold.is_some(), "surface is shared with a threshold");
    }

    #[test]
    fn a_missing_noise_is_a_compile_error() {
        let (functions, _) = corpus();
        let (rules, conditions) = material_corpus();
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &resolve_block,
            biome: &resolve_biome,
        };
        let error = build_router(
            &settings("overworld"),
            &functions,
            &BTreeMap::new(),
            42,
            VoxelId(1),
            VoxelId(2),
            Some(&inputs),
        )
        .map(|_| ())
        .expect_err("an empty noise registry cannot compile the surface noises");
        assert!(matches!(error, CompileError::UnknownNoise(_)), "{error:?}");
    }
}
