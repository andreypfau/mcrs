use crate::material::compile::{MaterialInputs, VeinId};
use crate::material::eval::{MaterialEval, NO_WATER, map};
use crate::material::proto::{
    CaveSurface, MaterialCondition, MaterialConditionHolder, MaterialRule, MaterialRuleHolder,
};
use crate::value_provider::HeightContext;
use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_random::Random;
use mcrs_voxel_storage::VoxelId;
use std::collections::BTreeSet;

/// What each node the walk visited answered, so a differential run can prove it
/// reached every kind rather than assuming it did.
pub(crate) type Visited = BTreeSet<(&'static str, bool)>;

/// The rule tree walked as it is written: references resolved on the way down,
/// no tape, no interning, no caches. It answers the same question as
/// [`MaterialEval::apply`] off the same context, which is the only thing that
/// makes the two comparable.
pub(crate) struct MaterialOracle<'a> {
    inputs: &'a MaterialInputs<'a>,
    root: &'a MaterialRuleHolder,
    height: HeightContext,
}

impl<'a> MaterialOracle<'a> {
    pub(crate) fn new(
        inputs: &'a MaterialInputs<'a>,
        root: &ResourceLocation,
        height: HeightContext,
    ) -> Self {
        let root = inputs.rules.get(root).expect("the root rule is registered");
        Self {
            inputs,
            root,
            height,
        }
    }

    pub(crate) fn apply<
        B: FnMut(i32, i32, i32) -> u32,
        R: FnMut(i32, i32, i32, i32, &mut Vec<u32>) -> bool,
    >(
        &self,
        eval: &mut MaterialEval<'_, B, R>,
        visited: &mut Visited,
    ) -> Option<VoxelId> {
        let mut veins = 0;
        self.rule(self.root, eval, &mut veins, visited)
    }

    fn rule<B: FnMut(i32, i32, i32) -> u32, R: FnMut(i32, i32, i32, i32, &mut Vec<u32>) -> bool>(
        &self,
        holder: &'a MaterialRuleHolder,
        eval: &mut MaterialEval<'_, B, R>,
        veins: &mut VeinId,
        visited: &mut Visited,
    ) -> Option<VoxelId> {
        match self.resolve_rule(holder) {
            MaterialRule::Block { result_state } => {
                Some((self.inputs.block)(result_state).expect("a compiled rule resolves its state"))
            }
            MaterialRule::Sequence { sequence } => {
                for member in sequence {
                    if let Some(state) = self.rule(member, eval, veins, visited) {
                        return Some(state);
                    }
                }
                None
            }
            MaterialRule::Condition { if_true, then_run } => {
                if self.condition(if_true, eval, visited) {
                    self.rule(then_run, eval, veins, visited)
                } else {
                    // The compiler numbers veins over the whole tree, so a
                    // skipped subtree still consumes the ids inside it.
                    self.count_veins(then_run, veins);
                    None
                }
            }
            MaterialRule::Bandlands => {
                visited.insert(("bandlands", true));
                Some(eval.bandlands())
            }
            MaterialRule::OreVein { .. } => {
                let vein = *veins;
                *veins += 1;
                let state = eval.ore_vein(vein);
                visited.insert(("ore_vein", state.is_some()));
                state
            }
        }
    }

    fn count_veins(&self, holder: &'a MaterialRuleHolder, veins: &mut VeinId) {
        match self.resolve_rule(holder) {
            MaterialRule::Sequence { sequence } => {
                for member in sequence {
                    self.count_veins(member, veins);
                }
            }
            MaterialRule::Condition { then_run, .. } => self.count_veins(then_run, veins),
            MaterialRule::OreVein { .. } => *veins += 1,
            MaterialRule::Block { .. } | MaterialRule::Bandlands => {}
        }
    }

    fn condition<
        B: FnMut(i32, i32, i32) -> u32,
        R: FnMut(i32, i32, i32, i32, &mut Vec<u32>) -> bool,
    >(
        &self,
        holder: &'a MaterialConditionHolder,
        eval: &mut MaterialEval<'_, B, R>,
        visited: &mut Visited,
    ) -> bool {
        let (kind, value) = match self.resolve_condition(holder) {
            MaterialCondition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                surface_type,
            } => {
                let depth = match surface_type {
                    CaveSurface::Ceiling => eval.depth_below(),
                    CaveSurface::Floor => eval.depth_above(),
                };
                let surface = if *add_surface_depth {
                    eval.surface_depth()
                } else {
                    0
                };
                let secondary = if *secondary_depth_range == 0 {
                    0
                } else {
                    map(
                        eval.surface_secondary(),
                        -1.0,
                        1.0,
                        0.0,
                        f64::from(*secondary_depth_range),
                    ) as i32
                };
                ("stone_depth", depth <= 1 + offset + surface + secondary)
            }
            MaterialCondition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                let stone = if *add_stone_depth {
                    eval.depth_above()
                } else {
                    0
                };
                let value = eval.water_level() == NO_WATER
                    || eval.block_y() + stone
                        >= eval.water_level()
                            + offset
                            + eval.surface_depth() * surface_depth_multiplier;
                ("water", value)
            }
            MaterialCondition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                let stone = if *add_stone_depth {
                    eval.depth_above()
                } else {
                    0
                };
                let value = eval.block_y() + stone
                    >= anchor.resolve_y(self.height)
                        + eval.surface_depth() * surface_depth_multiplier;
                ("y_above", value)
            }
            MaterialCondition::Biome { biome_is } => {
                let biome = eval.biome();
                let value = biome_is
                    .ids()
                    .iter()
                    .any(|name| (self.inputs.biome)(name) == Some(biome));
                ("biome", value)
            }
            MaterialCondition::NoiseThreshold {
                noise,
                min_threshold,
                max_threshold,
                is_3d,
            } => {
                let id = eval
                    .program()
                    .noise_id(noise, *is_3d)
                    .expect("a compiled noise");
                let value = eval.noise(id, *is_3d);
                (
                    "noise_threshold",
                    value >= min_threshold.0 && value <= max_threshold.0,
                )
            }
            MaterialCondition::VerticalGradient {
                random_name,
                true_at_and_below,
                false_at_and_above,
            } => {
                let below = true_at_and_below.resolve_y(self.height);
                let above = false_at_and_above.resolve_y(self.height);
                let y = eval.block_y();
                let value = if y <= below {
                    true
                } else if y >= above {
                    false
                } else {
                    let probability =
                        map(f64::from(y), f64::from(below), f64::from(above), 1.0, 0.0);
                    let id = eval
                        .program()
                        .random_id(random_name)
                        .expect("a compiled random");
                    let mut random = eval
                        .program()
                        .random_at(id, IVec3::new(eval.block_x(), y, eval.block_z()));
                    f64::from(random.next_f32()) < probability
                };
                ("vertical_gradient", value)
            }
            MaterialCondition::Steep => {
                ("steep", eval.gradient_x() <= -4 || eval.gradient_z() >= 4)
            }
            MaterialCondition::Hole => ("hole", eval.surface_depth() <= 0),
            MaterialCondition::AbovePreliminarySurface => (
                "above_preliminary_surface",
                eval.block_y() >= eval.min_surface_level(),
            ),
            MaterialCondition::Not { invert } => ("not", !self.condition(invert, eval, visited)),
            MaterialCondition::Temperature => {
                panic!("the compiler refuses temperature, so no tree reaching here compiles")
            }
        };
        visited.insert((kind, value));
        value
    }

    fn resolve_rule(&self, holder: &'a MaterialRuleHolder) -> &'a MaterialRule {
        let mut holder = holder;
        loop {
            match holder {
                MaterialRuleHolder::Owned(rule) => return rule,
                MaterialRuleHolder::Reference(id) => {
                    holder = self
                        .inputs
                        .rules
                        .get(id)
                        .expect("a compiled rule reference");
                }
            }
        }
    }

    fn resolve_condition(&self, holder: &'a MaterialConditionHolder) -> &'a MaterialCondition {
        let mut holder = holder;
        loop {
            match holder {
                MaterialConditionHolder::Owned(condition) => return condition,
                MaterialConditionHolder::Reference(id) => {
                    holder = self
                        .inputs
                        .conditions
                        .get(id)
                        .expect("a compiled condition reference");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::compile::tests::{build, material_corpus, resolve_biome, resolve_block};
    use crate::material::eval::MaterialScratch;

    /// Every condition kind the overworld tree reaches. Each has to be seen
    /// holding and not holding, or the run proves nothing about it.
    const CONDITIONS: [&str; 10] = [
        "above_preliminary_surface",
        "biome",
        "hole",
        "noise_threshold",
        "not",
        "steep",
        "stone_depth",
        "vertical_gradient",
        "water",
        "y_above",
    ];

    /// The two rules that are not a constant state. A vein has to be seen both
    /// producing and falling through, which is the tape's one non-returning op;
    /// a clay band always produces.
    const RULES: [(&str, bool); 3] = [("bandlands", true), ("ore_vein", true), ("ore_vein", false)];

    /// The biomes the overworld rule actually tests, read back off the interned
    /// masks so a corpus that gains one is covered without an edit here.
    fn tested_biomes(router: &crate::router::NoiseRouter) -> Vec<u32> {
        let sets = router.material().unwrap().biome_sets();
        let mut ids: Vec<u32> = (0..256u32)
            .filter(|id| sets.iter().any(|set| set.contains(*id)))
            .collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn the_tape_and_the_oracle_agree_over_the_whole_context_range() {
        let router = build("overworld");
        let (rules, conditions) = material_corpus();
        let inputs = MaterialInputs {
            rules: &rules,
            conditions: &conditions,
            block: &resolve_block,
            biome: &resolve_biome,
        };
        let height = HeightContext {
            min_y: router.noise.min_y,
            depth: router.noise.height as i32,
            sea_level: router.sea_level,
        };
        let oracle =
            MaterialOracle::new(&inputs, &ResourceLocation::minecraft("overworld"), height);

        let min_y = router.noise.min_y;
        let top = 96;
        let biomes = tested_biomes(&router);
        // Small palettes so most interned sets fold to `never` or `always` for
        // the column and the folded answers are compared too; one wide palette
        // so the rest stay `maybe`.
        let mut palettes: Vec<Vec<u32>> = biomes.chunks(5).map(<[u32]>::to_vec).collect();
        palettes.push(biomes.clone());

        let mut scratch = MaterialScratch::default();
        let mut visited = Visited::new();
        let mut checked = 0usize;
        let mut produced = 0usize;

        for (column, palette) in palettes.iter().enumerate() {
            let block_x = column as i32 * 16 * 37 - 2048;
            let block_z = column as i32 * 16 * 53 + 496;
            let choose = |x: i32, y: i32, z: i32| {
                let mixed = (x as i64 * 341 + y as i64 * 59 + z as i64 * 7919).unsigned_abs();
                palette[mixed as usize % palette.len()]
            };
            let mut eval = MaterialEval::new(
                &router,
                &mut scratch,
                choose,
                |_, _, _, _, _| false,
                block_x,
                block_z,
                top,
                palette,
            )
            .expect("the overworld router carries material rules");

            for (strip, (x, z)) in [(0, 0), (3, 11), (7, 5), (11, 15), (15, 8)]
                .into_iter()
                .enumerate()
            {
                let (bx, bz) = (block_x + x, block_z + z);
                let gradients = [(0, 0), (-4, 0), (-5, 3), (3, 4), (7, -7)][strip];

                for (round, surface_depth) in [-2, 0, 1, 3, 7].into_iter().enumerate() {
                    eval.begin_strip(bx, bz, gradients.0, gradients.1);
                    eval.set_surface_depth(surface_depth);
                    let level = eval.min_surface_level();

                    let ys = [
                        min_y,
                        min_y + 1,
                        min_y + 4,
                        min_y + 5,
                        min_y + 6,
                        -9,
                        -8,
                        -1,
                        0,
                        11,
                        level - 1,
                        level,
                        level + 1,
                        61,
                        62,
                        63,
                        64,
                        top,
                        top + 64,
                    ];
                    for (step, y) in ys.into_iter().enumerate() {
                        for combination in 0..4 {
                            let index = (round + step + combination) % 6;
                            let depth_above = [0, 1, 2, 5, 12, 35][index];
                            let depth_below = [1, 0, 40, 3, 2, 7][index];
                            let water_level =
                                [NO_WATER, y + 8, y - 2, 63, y - 1 + surface_depth, NO_WATER]
                                    [index];

                            eval.update_y(depth_above, depth_below, water_level, y);
                            let tape = eval.apply();
                            let walked = oracle.apply(&mut eval, &mut visited);
                            assert_eq!(
                                tape,
                                walked,
                                "tape and oracle disagree at \
                                 ({bx}, {y}, {bz}) column {column} biome {} \
                                 depth_above {depth_above} depth_below {depth_below} \
                                 water_level {water_level} surface_depth {surface_depth} \
                                 min_surface_level {level} \
                                 gradient_x {} gradient_z {}",
                                eval.biome(),
                                gradients.0,
                                gradients.1,
                            );
                            checked += 1;
                            produced += usize::from(tape.is_some());
                        }
                    }
                }
            }
        }

        assert!(checked > 10_000, "only {checked} contexts");
        assert!(
            produced * 4 > checked,
            "only {produced} of {checked} contexts produced a block, \
             so the walk mostly never reached a result"
        );

        let missing: Vec<String> = CONDITIONS
            .iter()
            .flat_map(|kind| [(*kind, true), (*kind, false)])
            .chain(RULES)
            .filter(|entry| !visited.contains(entry))
            .map(|(kind, value)| format!("{kind}={value}"))
            .collect();
        assert!(
            missing.is_empty(),
            "these node kinds were never exercised: {}",
            missing.join(", ")
        );
    }
}
