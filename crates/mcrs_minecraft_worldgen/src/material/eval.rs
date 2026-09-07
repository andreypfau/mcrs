use crate::jmath::mth_floor;
use crate::material::compile::{
    CondId, Condition, MaterialContext, MaterialProgram, NoiseId, Scope, SurfaceNoise, Tri, VeinId,
};
use crate::program::Workspace;
use crate::router::NoiseRouter;
use crate::volume::Volume;
use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_voxel_storage::VoxelId;

/// The water level of a strip in which no fluid has been seen from above.
pub const NO_WATER: i32 = i32::MIN;

/// Every allocation the descent reuses column after column.
#[derive(Default)]
pub struct MaterialScratch {
    bypass_caches: bool,
    workspace: Workspace,
    condition_stamp: Vec<u32>,
    condition_value: Vec<bool>,
    folded: Vec<Option<bool>>,
    noise_stamp: Vec<u32>,
    noise_value: Vec<f64>,
    preliminary: Vec<f32>,
    vein_values: Vec<f32>,
}

impl MaterialScratch {
    /// Force every cache to miss, so a run can be compared block for block
    /// against a memoised one.
    pub fn bypass_caches(&mut self, bypass: bool) {
        self.bypass_caches = bypass;
    }
}

/// The context the tape tests against, over one column.
///
/// `biome_at` answers the biome of a block through the zoom, which needs a grid
/// this crate cannot build; everything else here is a function of the program,
/// the router and the position.
pub struct MaterialEval<'a, B> {
    router: &'a NoiseRouter,
    program: &'a MaterialProgram,
    scratch: &'a mut MaterialScratch,
    biome_at: B,
    memoise: bool,
    preliminary: Volume,
    veins: Volume,
    gen_xz: u32,
    gen_y: u32,
    block_x: i32,
    block_z: i32,
    gradient_x: i32,
    gradient_z: i32,
    surface_depth: i32,
    surface_secondary: f64,
    surface_secondary_stamp: u32,
    min_surface_level: i32,
    min_surface_level_stamp: u32,
    block_y: i32,
    depth_above: i32,
    depth_below: i32,
    water_level: i32,
    biome: u32,
    biome_stamp: u32,
}

impl<'a, B: FnMut(i32, i32, i32) -> u32> MaterialEval<'a, B> {
    /// `None` for a router built without material rules.
    ///
    /// `top` is the highest non-air block of the column and `biomes` every biome
    /// the column can select, which must include the border ring of the grid the
    /// zoom reads: a set narrower than the zoom's reach folds a condition that
    /// should have matched to `never` and writes the wrong block.
    pub fn new(
        router: &'a NoiseRouter,
        scratch: &'a mut MaterialScratch,
        biome_at: B,
        block_x: i32,
        block_z: i32,
        top: i32,
        biomes: &[u32],
    ) -> Option<Self> {
        let program = router.material()?;
        let min_y = router.noise_min_y();
        let memoise = !scratch.bypass_caches;

        scratch.condition_stamp.clear();
        scratch
            .condition_stamp
            .resize(program.conditions().len(), 0);
        scratch.condition_value.clear();
        scratch
            .condition_value
            .resize(program.conditions().len(), false);
        scratch.noise_stamp.clear();
        scratch.noise_stamp.resize(program.noise_count(), 0);
        scratch.noise_value.clear();
        scratch.noise_value.resize(program.noise_count(), 0.0);

        scratch.folded.clear();
        for condition in program.conditions() {
            let folded = match condition.kind {
                Condition::Biome { set } => {
                    match program.biome_sets()[set as usize].fold(biomes.iter().copied()) {
                        Tri::Never => Some(false),
                        Tri::Always => Some(true),
                        Tri::Maybe => None,
                    }
                }
                // An operand is interned before the `not` that names it, so its
                // own fold is already settled here.
                Condition::Not(inner) => scratch.folded[inner as usize].map(|value| !value),
                _ => None,
            };
            scratch.folded.push(folded);
        }

        let preliminary = Volume::dense(IVec3::new(16, 1, 16), IVec3::new(block_x, 0, block_z));
        scratch.preliminary.clear();
        scratch.preliminary.resize(preliminary.len(), 0.0);
        router.fill(
            &mut scratch.workspace,
            &preliminary,
            router.chunk_surface_level(),
            &mut scratch.preliminary,
        );

        let veins = Volume::dense(
            IVec3::new(16, (top - min_y + 1).max(1), 16),
            IVec3::new(block_x, min_y, block_z),
        );
        let points = veins.len();
        scratch.vein_values.clear();
        scratch
            .vein_values
            .resize(2 * program.veins().len() * points, 0.0);
        for (index, vein) in program.veins().iter().enumerate() {
            for (row, root) in [vein.density, vein.richness].into_iter().enumerate() {
                let at = (2 * index + row) * points;
                router.fill(
                    &mut scratch.workspace,
                    &veins,
                    root,
                    &mut scratch.vein_values[at..at + points],
                );
            }
        }

        Some(Self {
            router,
            program,
            scratch,
            biome_at,
            memoise,
            preliminary,
            veins,
            gen_xz: 0,
            gen_y: 0,
            block_x,
            block_z,
            gradient_x: 0,
            gradient_z: 0,
            surface_depth: 0,
            surface_secondary: 0.0,
            surface_secondary_stamp: 0,
            min_surface_level: 0,
            min_surface_level_stamp: 0,
            block_y: 0,
            depth_above: 0,
            depth_below: 0,
            water_level: NO_WATER,
            biome: 0,
            biome_stamp: 0,
        })
    }

    pub fn begin_strip(&mut self, block_x: i32, block_z: i32, gradient_x: i32, gradient_z: i32) {
        self.gen_xz += 1;
        self.gen_y += 1;
        self.block_x = block_x;
        self.block_z = block_z;
        self.gradient_x = gradient_x;
        self.gradient_z = gradient_z;
        self.surface_depth = self.compute_surface_depth();
    }

    pub fn update_y(&mut self, depth_above: i32, depth_below: i32, water_level: i32, block_y: i32) {
        self.gen_y += 1;
        self.block_y = block_y;
        self.water_level = water_level;
        self.depth_above = depth_above;
        self.depth_below = depth_below;
    }

    /// The block the rules produce at the current position, if any.
    pub fn apply(&mut self) -> Option<VoxelId> {
        let program = self.program;
        program.run(self)
    }

    pub fn min_surface_level(&mut self) -> i32 {
        if self.memoise && self.min_surface_level_stamp == self.gen_xz {
            return self.min_surface_level;
        }
        self.min_surface_level_stamp = self.gen_xz;
        let level = match self
            .preliminary
            .index_of_block(self.block_x, 0, self.block_z)
        {
            Some(index) => self.scratch.preliminary[index],
            None => self.sample(
                self.router.chunk_surface_level(),
                IVec3::new(self.block_x, 0, self.block_z),
            ),
        };
        self.min_surface_level = mth_floor(level) + self.surface_depth - 8;
        self.min_surface_level
    }

    fn compute_surface_depth(&mut self) -> i32 {
        let noise = self.noise(self.program.surface_noise(SurfaceNoise::Surface), false);
        let mut random = self
            .program
            .noise_random_at(IVec3::new(self.block_x, 0, self.block_z));
        (noise * 2.75 + 3.0 + random.next_f64() * 0.25) as i32
    }

    pub(crate) fn surface_secondary(&mut self) -> f64 {
        if self.memoise && self.surface_secondary_stamp == self.gen_xz {
            return self.surface_secondary;
        }
        self.surface_secondary_stamp = self.gen_xz;
        self.surface_secondary = self.noise(
            self.program.surface_noise(SurfaceNoise::SurfaceSecondary),
            false,
        );
        self.surface_secondary
    }

    pub(crate) fn biome(&mut self) -> u32 {
        if self.memoise && self.biome_stamp == self.gen_y {
            return self.biome;
        }
        self.biome_stamp = self.gen_y;
        self.biome = (self.biome_at)(self.block_x, self.block_y, self.block_z);
        self.biome
    }

    pub(crate) fn noise(&mut self, noise: NoiseId, is_3d: bool) -> f64 {
        let stamp = if is_3d { self.gen_y } else { self.gen_xz };
        if self.memoise && self.scratch.noise_stamp[noise as usize] == stamp {
            return self.scratch.noise_value[noise as usize];
        }
        let y = if is_3d { f64::from(self.block_y) } else { 0.0 };
        let value = f64::from(self.program.noise(noise).get(
            f64::from(self.block_x),
            y,
            f64::from(self.block_z),
        ));
        self.scratch.noise_stamp[noise as usize] = stamp;
        self.scratch.noise_value[noise as usize] = value;
        value
    }

    fn sample(&mut self, root: usize, pos: IVec3) -> f32 {
        let mut out = [0.0f32];
        self.router.fill(
            &mut self.scratch.workspace,
            &Volume::point(pos),
            root,
            &mut out,
        );
        out[0]
    }

    /// A density prefilled over the column, point-sampled where the position
    /// sits above the fill — which a pillar built before the descent can.
    fn prefilled(&mut self, row: usize, root: usize) -> f32 {
        match self
            .veins
            .index_of_block(self.block_x, self.block_y, self.block_z)
        {
            Some(index) => self.scratch.vein_values[row * self.veins.len() + index],
            None => self.sample(root, IVec3::new(self.block_x, self.block_y, self.block_z)),
        }
    }

    fn compute(&mut self, condition: CondId) -> bool {
        let program = self.program;
        match program.conditions()[condition as usize].kind {
            Condition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => {
                let depth = if ceiling {
                    self.depth_below
                } else {
                    self.depth_above
                };
                let surface_depth = if add_surface_depth {
                    self.surface_depth
                } else {
                    0
                };
                let secondary = if secondary_depth_range == 0 {
                    0
                } else {
                    map(
                        self.surface_secondary(),
                        -1.0,
                        1.0,
                        0.0,
                        f64::from(secondary_depth_range),
                    ) as i32
                };
                depth <= 1 + offset + surface_depth + secondary
            }
            Condition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                self.water_level == NO_WATER
                    || self.block_y + if add_stone_depth { self.depth_above } else { 0 }
                        >= self.water_level + offset + self.surface_depth * surface_depth_multiplier
            }
            Condition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                self.block_y + if add_stone_depth { self.depth_above } else { 0 }
                    >= anchor + self.surface_depth * surface_depth_multiplier
            }
            Condition::Biome { set } => {
                let biome = self.biome();
                program.biome_sets()[set as usize].contains(biome)
            }
            Condition::NoiseThreshold {
                noise,
                min,
                max,
                is_3d,
            } => {
                let value = self.noise(noise, is_3d);
                value >= min.0 && value <= max.0
            }
            Condition::VerticalGradient {
                random,
                true_at_and_below,
                false_at_and_above,
            } => {
                if self.block_y <= true_at_and_below {
                    true
                } else if self.block_y >= false_at_and_above {
                    false
                } else {
                    let probability = map(
                        f64::from(self.block_y),
                        f64::from(true_at_and_below),
                        f64::from(false_at_and_above),
                        1.0,
                        0.0,
                    );
                    let mut random = program
                        .random_at(random, IVec3::new(self.block_x, self.block_y, self.block_z));
                    f64::from(random.next_f32()) < probability
                }
            }
            Condition::Steep => self.gradient_x <= -4 || self.gradient_z >= 4,
            Condition::Hole => self.surface_depth <= 0,
            Condition::AbovePreliminarySurface => self.block_y >= self.min_surface_level(),
            Condition::Not(inner) => !self.test(inner),
        }
    }
}

#[cfg(test)]
impl<B> MaterialEval<'_, B> {
    pub(crate) fn program(&self) -> &MaterialProgram {
        self.program
    }

    pub(crate) fn block_x(&self) -> i32 {
        self.block_x
    }

    pub(crate) fn block_y(&self) -> i32 {
        self.block_y
    }

    pub(crate) fn block_z(&self) -> i32 {
        self.block_z
    }

    pub(crate) fn depth_above(&self) -> i32 {
        self.depth_above
    }

    pub(crate) fn depth_below(&self) -> i32 {
        self.depth_below
    }

    pub(crate) fn water_level(&self) -> i32 {
        self.water_level
    }

    pub(crate) fn surface_depth(&self) -> i32 {
        self.surface_depth
    }

    pub(crate) fn gradient_x(&self) -> i32 {
        self.gradient_x
    }

    pub(crate) fn gradient_z(&self) -> i32 {
        self.gradient_z
    }

    /// Reaches the depths the surface noise would only reach at rare positions.
    /// The stamp goes back to zero because `begin_strip` never leaves `gen_xz`
    /// there, so the preliminary level recomputes against the new depth.
    pub(crate) fn set_surface_depth(&mut self, depth: i32) {
        self.surface_depth = depth;
        self.min_surface_level_stamp = 0;
    }
}

impl<B: FnMut(i32, i32, i32) -> u32> MaterialContext for MaterialEval<'_, B> {
    fn test(&mut self, condition: CondId) -> bool {
        if !self.memoise {
            return self.compute(condition);
        }
        if let Some(value) = self.scratch.folded[condition as usize] {
            return value;
        }
        let stamp = match self.program.conditions()[condition as usize].scope {
            Scope::Xz => self.gen_xz,
            Scope::Y => self.gen_y,
        };
        if self.scratch.condition_stamp[condition as usize] == stamp {
            return self.scratch.condition_value[condition as usize];
        }
        let value = self.compute(condition);
        self.scratch.condition_stamp[condition as usize] = stamp;
        self.scratch.condition_value[condition as usize] = value;
        value
    }

    fn bandlands(&mut self) -> VoxelId {
        let noise = self.noise(
            self.program.surface_noise(SurfaceNoise::ClayBandsOffset),
            false,
        );
        // Rounding here is a floor of the half-shifted value, not Rust's
        // away-from-zero `round`: the two disagree on every negative half and
        // shift the whole band table.
        let offset = ((noise as f32 * 4.0) + 0.5).floor() as i32;
        let bands = self.program.clay_bands();
        bands[(self.block_y + offset).rem_euclid(bands.len() as i32) as usize]
    }

    fn ore_vein(&mut self, vein: VeinId) -> Option<VoxelId> {
        let ore_vein = self.program.veins()[vein as usize];
        let density = self.prefilled(2 * vein as usize, ore_vein.density);
        if density <= 0.0 {
            return None;
        }
        let mut random = self.program.random_at(
            ore_vein.random,
            IVec3::new(self.block_x, self.block_y, self.block_z),
        );
        if random.next_f32() > density {
            return None;
        }
        let richness = self.prefilled(2 * vein as usize + 1, ore_vein.richness);
        if random.next_f32() < richness
            && self.sample(
                ore_vein.filler_gap,
                IVec3::new(self.block_x, self.block_y, self.block_z),
            ) < 0.0
        {
            Some(if random.next_f32() < ore_vein.raw_ore_chance {
                ore_vein.raw_ore
            } else {
                ore_vein.ore
            })
        } else {
            Some(ore_vein.filler)
        }
    }
}

/// A lerp over an inverse lerp, unclamped: a value outside the source range
/// maps outside the target range.
pub(crate) fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    to_min + (value - from_min) / (from_max - from_min) * (to_max - to_min)
}
