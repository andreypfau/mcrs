use crate::density_function::proto::{
    DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction, RarityValueMapper,
    SingleArgumentFunction, SplineHolder, TwoArgumentFunction, Visitor,
};
use crate::noise::normal_noise::NoiseSampler;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use crate::proto::NoiseGeneratorSettings;
use crate::spline::{RangeFunction, SplineFunction};
use bevy_math::{Curve, FloatExt, IVec3};
use mcrs_protocol::Ident;
use mcrs_random::legacy::LegacyRandom;
use mcrs_random::{Random, RandomSource};
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Formatter};
use std::mem::swap;
use std::ops::Index;
use tracing::info_span;

pub mod proto;

struct ChunkNoiseFunctionBuilderOptions {
    // Number of blocks per cell per axis
    horizontal_cell_block_count: usize,
    vertical_cell_block_count: usize,

    // Number of cells per chunk per axis
    vertical_cell_count: usize,
    horizontal_cell_count: usize,

    // The biome coords of this chunk
    pub start_biome_x: i32,
    pub start_biome_z: i32,

    // Number of biome regions per chunk per axis
    pub horizontal_biome_end: usize,
}

trait DensityFunction: RangeFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32;
}

pub fn build_functions(
    functions: &BTreeMap<Ident<String>, ProtoDensityFunction>,
    noises: &BTreeMap<Ident<String>, NoiseParam>,
    noise_settings: &NoiseGeneratorSettings,
    seed: u64,
) -> NoiseRouter {
    let random = RandomSource::new(seed, noise_settings.legacy_random_source);
    let builder_options = ChunkNoiseFunctionBuilderOptions {
        horizontal_cell_block_count: 4,
        vertical_cell_block_count: 8,
        vertical_cell_count: 16,
        horizontal_cell_count: 16,
        start_biome_x: 0,
        start_biome_z: 0,
        horizontal_biome_end: 4,
    };
    let mut builder = FunctionStackBuilder::new(random, functions, noises, &builder_options);
    let final_density_index = builder.component(&noise_settings.noise_router.final_density);
    let temperature_index = builder.component(&noise_settings.noise_router.temperature);
    let vegetation_index = builder.component(&noise_settings.noise_router.vegetation);
    let continents_index = builder.component(&noise_settings.noise_router.continents);
    let erosion_index = builder.component(&noise_settings.noise_router.erosion);
    let depth_index = builder.component(&noise_settings.noise_router.depth);
    let ridges_index = builder.component(&noise_settings.noise_router.ridges);
    let preliminary_surface_level_index =
        builder.component(&noise_settings.noise_router.preliminary_surface_level);

    NoiseRouter {
        final_density_index,
        temperature_index,
        vegetation_index,
        continents_index,
        erosion_index,
        depth_index,
        ridges_index,
        preliminary_surface_level_index,
        stack: Box::from(builder.stack),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoiseRouter {
    final_density_index: usize,
    temperature_index: usize,
    vegetation_index: usize,
    continents_index: usize,
    erosion_index: usize,
    depth_index: usize,
    ridges_index: usize,
    preliminary_surface_level_index: usize,
    stack: Box<[DensityFunctionComponent]>,
}

impl NoiseRouter {
    pub fn final_density(&self, pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&self.stack[..=self.final_density_index], pos)
    }
}

#[derive(Clone, PartialEq)]
struct OldBlendedNoise {
    xz_scale: f32,
    y_scale: f32,
    xz_factor: f32,
    y_factor: f32,
    smear_scale_multiplier: f32,
    xz_multiplier: f32,
    y_multiplier: f32,
    max_value: f32,
    limit_smear: f32,
    main_smear: f32,
    lower_interpolated_noise: OctavePerlinNoise,
    upper_interpolated_noise: OctavePerlinNoise,
    interpolated_noise: OctavePerlinNoise,
}

impl Debug for OldBlendedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OldBlendedNoise")
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("xz_factor", &self.xz_scale)
            .field("y_factor", &self.y_scale)
            .field("smear_scale_multiplier", &self.smear_scale_multiplier)
            .field("xz_multiplier", &self.xz_multiplier)
            .field("y_multiplier", &self.y_multiplier)
            .field("max_value", &self.max_value)
            .finish()
    }
}

impl OldBlendedNoise {
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f32,
        y_scale: f32,
        xz_factor: f32,
        y_factor: f32,
        smear_scale_multiplier: f32,
    ) -> Self {
        let xz_multiplier = 684.412 * xz_scale;
        let y_multiplier = 684.412 * y_scale;
        let limit_smear = y_multiplier * smear_scale_multiplier;
        let main_smear = limit_smear / y_factor;
        let lower_interpolated_noise = OctavePerlinNoise::new(random, -15, vec![1.0; 16], true);
        let max_value = lower_interpolated_noise.edge_value(y_multiplier + 2.0);
        OldBlendedNoise {
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            xz_multiplier,
            y_multiplier,
            max_value,
            limit_smear,
            main_smear,
            lower_interpolated_noise,
            upper_interpolated_noise: OctavePerlinNoise::new(random, -15, vec![1.0; 16], true),
            interpolated_noise: OctavePerlinNoise::new(random, -7, vec![1.0; 8], true),
        }
    }
}

impl RangeFunction for OldBlendedNoise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for OldBlendedNoise {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        // todo: fraction optimization from Pumpkin
        let scaled_x = pos.x as f32 * self.xz_multiplier;
        let scaled_y = pos.y as f32 * self.y_multiplier;
        let scaled_z = pos.z as f32 * self.xz_multiplier;

        let factored_x = scaled_x / self.xz_factor;
        let factored_y = scaled_y / self.y_factor;
        let factored_z = scaled_z / self.xz_factor;

        let mut value = 0.0;
        let mut factor = 1.0;
        for i in 0..8 {
            if let Some(noise) = self.interpolated_noise.get_octave(i) {
                let xx = OctavePerlinNoise::maintain_precission(factored_x * factor);
                let yy = OctavePerlinNoise::maintain_precission(factored_y * factor);
                let zz = OctavePerlinNoise::maintain_precission(factored_z * factor);
                value += noise.sample(
                    xx,
                    yy,
                    zz,
                    (self.main_smear * factor),
                    (factored_y * factor),
                ) as f32
                    / factor;
            }
            factor /= 2.0;
        }

        value = (value / 10.0 + 1.0) / 2.0;
        factor = 1.0;
        let less_than_one = value < 1.0;
        let more_than_zero = value > 0.0;
        let mut min = 0.0;
        let mut max = 0.0;

        let smear = self.limit_smear;
        for i in 0..16 {
            let xx = OctavePerlinNoise::maintain_precission(scaled_x * factor);
            let yy = OctavePerlinNoise::maintain_precission(scaled_y * factor);
            let zz = OctavePerlinNoise::maintain_precission(scaled_z * factor);
            let smears_smear = smear * factor;
            if less_than_one {
                if let Some(noise) = self.lower_interpolated_noise.get_octave(i) {
                    min += noise.sample(xx, yy, zz, smears_smear, scaled_y * factor) / factor;
                }
            }
            if more_than_zero {
                if let Some(noise) = self.upper_interpolated_noise.get_octave(i) {
                    max += noise.sample(xx, yy, zz, smears_smear, scaled_y * factor) / factor;
                }
            }
            factor /= 2.0;
        }

        let start = min / 512.0;
        let end = max / 512.0;
        value = if value < 0.0 {
            start
        } else if value > 1.0 {
            end
        } else {
            value * (end - start) + start
        };
        value / 128.0
    }
}

#[derive(Clone, PartialEq)]
struct Noise {
    sampler: NoiseSampler,
    xz_scale: f32,
    y_scale: f32,
}

impl Debug for Noise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Noise")
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for Noise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.sampler.max_value() as f32
    }
}

impl DensityFunction for Noise {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let xz_scale = self.xz_scale;
        let y_scale = self.y_scale;
        self.sampler.get(
            (pos.x as f32 * xz_scale),
            (pos.y as f32 * y_scale),
            (pos.z as f32 * xz_scale),
        )
    }
}

#[derive(Clone, PartialEq)]
struct ShiftA {
    sampler: NoiseSampler,
}

impl Debug for ShiftA {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftA")
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for ShiftA {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        (self.sampler.max_value() * 4.0) as f32
    }
}

impl DensityFunction for ShiftA {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        self.sampler
            .get((pos.x as f32 * 0.25), 0.0, (pos.z as f32 * 0.25))
            * 4.0
    }
}

#[derive(Clone, PartialEq)]
struct ShiftB {
    sampler: NoiseSampler,
}

impl Debug for ShiftB {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftB")
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for ShiftB {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        (self.sampler.max_value() * 4.0) as f32
    }
}

impl DensityFunction for ShiftB {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        self.sampler
            .get(pos.z as f32 * 0.25, pos.x as f32 * 0.25, 0.0)
            * 4.0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Shift {
    sampler: NoiseSampler,
}

impl RangeFunction for Shift {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.sampler.max_value() * 4.0
    }
}

impl DensityFunction for Shift {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        self.sampler.get(
            pos.z as f32 * 0.25,
            pos.x as f32 * 0.25,
            pos.z as f32 * 0.25,
        ) * 4.0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct BlendDensity {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for BlendDensity {
    #[inline]
    fn min_value(&self) -> f32 {
        f32::NEG_INFINITY
    }

    #[inline]
    fn max_value(&self) -> f32 {
        f32::INFINITY
    }
}

impl DensityFunction for BlendDensity {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
    }
}

#[derive(Clone, PartialEq)]
struct Interpolated {
    input_index: usize,

    pub(crate) start_buffer: Box<[f32]>,
    pub(crate) end_buffer: Box<[f32]>,

    first_pass: [f32; 8],
    second_pass: [f32; 4],
    third_pass: [f32; 2],
    result: f32,

    pub(crate) vertical_cell_count: usize,
    min_value: f32,
    max_value: f32,
}

impl Debug for Interpolated {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interpolated")
            .field("input_index", &self.input_index)
            .field("min_value", &self.min_value)
            .field("max_value", &self.max_value)
            .finish()
    }
}

impl RangeFunction for Interpolated {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl Interpolated {
    pub fn new(
        input_index: usize,
        min_value: f32,
        max_value: f32,
        builder_options: &ChunkNoiseFunctionBuilderOptions,
    ) -> Self {
        Self {
            input_index,
            start_buffer: vec![
                0.0;
                (builder_options.vertical_cell_count + 1)
                    * (builder_options.horizontal_cell_count + 1)
            ]
            .into_boxed_slice(),
            end_buffer: vec![
                0.0;
                (builder_options.vertical_cell_count + 1)
                    * (builder_options.horizontal_cell_count + 1)
            ]
            .into_boxed_slice(),
            first_pass: Default::default(),
            second_pass: Default::default(),
            third_pass: Default::default(),
            result: Default::default(),
            vertical_cell_count: builder_options.vertical_cell_count,
            min_value,
            max_value,
        }
    }

    #[inline]
    pub(crate) fn yz_to_buf_index(&self, cell_y_position: usize, cell_z_position: usize) -> usize {
        cell_z_position * (self.vertical_cell_count + 1) + cell_y_position
    }

    pub(crate) fn on_sampled_cell_corners(
        &mut self,
        cell_y_position: usize,
        cell_z_position: usize,
    ) {
        self.first_pass[0] =
            self.start_buffer[self.yz_to_buf_index(cell_y_position, cell_z_position)];
        self.first_pass[1] =
            self.start_buffer[self.yz_to_buf_index(cell_y_position, cell_z_position + 1)];
        self.first_pass[4] =
            self.end_buffer[self.yz_to_buf_index(cell_y_position, cell_z_position)];
        self.first_pass[5] =
            self.end_buffer[self.yz_to_buf_index(cell_y_position, cell_z_position + 1)];
        self.first_pass[2] =
            self.start_buffer[self.yz_to_buf_index(cell_y_position + 1, cell_z_position)];
        self.first_pass[3] =
            self.start_buffer[self.yz_to_buf_index(cell_y_position + 1, cell_z_position + 1)];
        self.first_pass[6] =
            self.end_buffer[self.yz_to_buf_index(cell_y_position + 1, cell_z_position)];
        self.first_pass[7] =
            self.end_buffer[self.yz_to_buf_index(cell_y_position + 1, cell_z_position + 1)];
    }

    pub(crate) fn interpolate_y(&mut self, delta: f32) {
        self.second_pass[0] = lerp(delta, self.first_pass[0], self.first_pass[2]);
        self.second_pass[2] = lerp(delta, self.first_pass[4], self.first_pass[6]);
        self.second_pass[1] = lerp(delta, self.first_pass[1], self.first_pass[3]);
        self.second_pass[3] = lerp(delta, self.first_pass[5], self.first_pass[7]);
    }

    #[inline]
    pub(crate) fn interpolate_x(&mut self, delta: f32) {
        self.third_pass[0] = lerp(delta, self.second_pass[0], self.second_pass[2]);
        self.third_pass[1] = lerp(delta, self.second_pass[1], self.second_pass[3]);
    }

    #[inline]
    pub(crate) fn interpolate_z(&mut self, delta: f32) {
        self.result = lerp(delta, self.third_pass[0], self.third_pass[1]);
    }

    #[inline]
    pub(crate) fn swap_buffers(&mut self) {
        #[cfg(debug_assertions)]
        let test = self.start_buffer[0];
        swap(&mut self.start_buffer, &mut self.end_buffer);
        #[cfg(debug_assertions)]
        assert_eq!(test, self.end_buffer[0]);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FlatCache {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for FlatCache {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for FlatCache {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Cache2d {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Cache2d {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for Cache2d {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CacheOnce {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for CacheOnce {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for CacheOnce {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CacheAllInCell {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for CacheAllInCell {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for CacheAllInCell {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ClampedYGradient {
    from_y: f32,
    to_y: f32,
    from_value: f32,
    to_value: f32,
}
impl RangeFunction for ClampedYGradient {
    fn min_value(&self) -> f32 {
        self.from_value.min(self.to_value)
    }

    fn max_value(&self) -> f32 {
        self.from_value.max(self.to_value)
    }
}
impl DensityFunction for ClampedYGradient {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let y = pos.y as f32;
        let from_y = self.from_y;
        if y < from_y {
            self.from_value
        } else if y > self.to_y {
            self.to_value
        } else {
            let from_value = self.from_value;
            from_value + (self.to_value - from_value) * (y - from_y) / (self.to_y - from_y)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum IndependentDensityFunction {
    Constant(f32),
    OldBlendedNoise(OldBlendedNoise),
    Noise(Noise),
    ShiftA(ShiftA),
    ShiftB(ShiftB),
    Shift(Shift),
    ClampedYGradient(ClampedYGradient),
}

impl RangeFunction for IndependentDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.min_value(),
            IndependentDensityFunction::Noise(x) => x.min_value(),
            IndependentDensityFunction::ShiftA(x) => x.min_value(),
            IndependentDensityFunction::ShiftB(x) => x.min_value(),
            IndependentDensityFunction::Shift(x) => x.min_value(),
            IndependentDensityFunction::ClampedYGradient(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.max_value(),
            IndependentDensityFunction::Noise(x) => x.max_value(),
            IndependentDensityFunction::ShiftA(x) => x.max_value(),
            IndependentDensityFunction::ShiftB(x) => x.max_value(),
            IndependentDensityFunction::Shift(x) => x.max_value(),
            IndependentDensityFunction::ClampedYGradient(x) => x.max_value(),
        }
    }
}

impl DensityFunction for IndependentDensityFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => {
                let _span = info_span!("Constant::sample").entered();
                *x
            }
            IndependentDensityFunction::OldBlendedNoise(x) => {
                let _span = info_span!("OldBlendedNoise::sample").entered();
                x.sample(stack, pos)
            }
            IndependentDensityFunction::Noise(x) => {
                let _span = info_span!("Noise::sample").entered();
                x.sample(stack, pos)
            }
            IndependentDensityFunction::ShiftA(x) => {
                let _span = info_span!("ShiftA::sample").entered();
                x.sample(stack, pos)
            }
            IndependentDensityFunction::ShiftB(x) => {
                let _span = info_span!("ShiftB::sample").entered();
                x.sample(stack, pos)
            }
            IndependentDensityFunction::Shift(x) => {
                let _span = info_span!("Shift::sample").entered();
                x.sample(stack, pos)
            }
            IndependentDensityFunction::ClampedYGradient(x) => {
                let _span = info_span!("ClampedYGradient::sample").entered();
                x.sample(stack, pos)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum DependentDensityFunction {
    Linear(Linear),
    Unary(Unary),
    Binary(Binary),
    ShiftedNoise(ShiftedNoise),
    WeirdScaled(WeirdScaled),
    Clamp(Clamp),
    RangeChoice(RangeChoice),
    Spline(Spline),
    FindTopSurface(FindTopSurface),
}

#[derive(Clone, Debug, PartialEq)]
enum WrapperDensityFunction {
    BlendDensity(BlendDensity),
    Interpolated(Interpolated),
    FlatCache(FlatCache),
    Cache2d(Cache2d),
    CacheOnce(CacheOnce),
    CacheAllInCell(CacheAllInCell),
}

impl RangeFunction for WrapperDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            WrapperDensityFunction::BlendDensity(x) => x.min_value(),
            WrapperDensityFunction::Interpolated(x) => x.min_value(),
            WrapperDensityFunction::FlatCache(x) => x.min_value(),
            WrapperDensityFunction::Cache2d(x) => x.min_value(),
            WrapperDensityFunction::CacheOnce(x) => x.min_value(),
            WrapperDensityFunction::CacheAllInCell(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            WrapperDensityFunction::BlendDensity(x) => x.max_value(),
            WrapperDensityFunction::Interpolated(x) => x.max_value(),
            WrapperDensityFunction::FlatCache(x) => x.max_value(),
            WrapperDensityFunction::Cache2d(x) => x.max_value(),
            WrapperDensityFunction::CacheOnce(x) => x.max_value(),
            WrapperDensityFunction::CacheAllInCell(x) => x.max_value(),
        }
    }
}

impl DensityFunction for WrapperDensityFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            WrapperDensityFunction::BlendDensity(x) => {
                let _span = info_span!("BlendDensity::sample").entered();
                x.sample(stack, pos)
            }
            WrapperDensityFunction::Interpolated(x) => {
                let _span = info_span!("Interpolated::sample").entered();
                DensityFunctionComponent::sample_from_stack(&stack[..=x.input_index], pos)
            }
            WrapperDensityFunction::FlatCache(x) => {
                let _span = info_span!("FlatCache::sample").entered();
                x.sample(stack, pos)
            }
            WrapperDensityFunction::Cache2d(x) => {
                let _span = info_span!("Cache2d::sample").entered();
                x.sample(stack, pos)
            }
            WrapperDensityFunction::CacheOnce(x) => {
                let _span = info_span!("CacheOnce::sample").entered();
                x.sample(stack, pos)
            }
            WrapperDensityFunction::CacheAllInCell(x) => {
                let _span = info_span!("CacheAllInCell::sample").entered();
                x.sample(stack, pos)
            }
        }
    }
}

impl DensityFunction for DependentDensityFunction {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.sample(stack, pos),
            DependentDensityFunction::Unary(x) => x.sample(stack, pos),
            DependentDensityFunction::Binary(x) => x.sample(stack, pos),
            DependentDensityFunction::ShiftedNoise(x) => x.sample(stack, pos),
            DependentDensityFunction::WeirdScaled(x) => x.sample(stack, pos),
            DependentDensityFunction::Clamp(x) => x.sample(stack, pos),
            DependentDensityFunction::RangeChoice(x) => x.sample(stack, pos),
            DependentDensityFunction::Spline(x) => {
                let _span = info_span!("Spline::sample").entered();
                x.sample(stack, pos)
            }
            DependentDensityFunction::FindTopSurface(x) => x.sample(stack, pos),
        }
    }
}

impl RangeFunction for DependentDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.min_value(),
            DependentDensityFunction::Unary(x) => x.min_value(),
            DependentDensityFunction::Binary(x) => x.min_value(),
            DependentDensityFunction::ShiftedNoise(x) => x.min_value(),
            DependentDensityFunction::WeirdScaled(x) => x.min_value(),
            DependentDensityFunction::Clamp(x) => x.min_value(),
            DependentDensityFunction::RangeChoice(x) => x.min_value(),
            DependentDensityFunction::Spline(x) => x.min_value(),
            DependentDensityFunction::FindTopSurface(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.max_value(),
            DependentDensityFunction::Unary(x) => x.max_value(),
            DependentDensityFunction::Binary(x) => x.max_value(),
            DependentDensityFunction::ShiftedNoise(x) => x.max_value(),
            DependentDensityFunction::WeirdScaled(x) => x.max_value(),
            DependentDensityFunction::Clamp(x) => x.max_value(),
            DependentDensityFunction::RangeChoice(x) => x.max_value(),
            DependentDensityFunction::Spline(x) => x.max_value(),
            DependentDensityFunction::FindTopSurface(x) => x.max_value(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Linear {
    input_index: usize,
    min_value: f32,
    max_value: f32,
    argument: f32,
    operation: LinearOperation,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum LinearOperation {
    Add,
    Multiply,
}

impl DensityFunction for Linear {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = match self.operation {
            LinearOperation::Add => info_span!("Linear::Add::Sample", self.argument).entered(),
            LinearOperation::Multiply => {
                info_span!("Linear::Multiply::sample", self.argument).entered()
            }
        };
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        match self.operation {
            LinearOperation::Add => density + self.argument,
            LinearOperation::Multiply => density * self.argument,
        }
    }
}

impl RangeFunction for Linear {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Unary {
    input_index: usize,
    min_value: f32,
    max_value: f32,
    operation: UnaryOperation,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum UnaryOperation {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Invert,
    Squeeze,
}

impl UnaryOperation {
    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        match self {
            UnaryOperation::Abs => value.abs(),
            UnaryOperation::Square => value.powi(2),
            UnaryOperation::Cube => value.powi(3),
            UnaryOperation::HalfNegative => {
                if value > 0.0 {
                    value
                } else {
                    value * 0.5
                }
            }
            UnaryOperation::QuarterNegative => {
                if value > 0.0 {
                    value
                } else {
                    value * 0.25
                }
            }
            UnaryOperation::Invert => 1.0 / value,
            UnaryOperation::Squeeze => {
                let clamped = value.clamp(-1.0, 1.0);
                clamped / 2.0 - clamped.powi(3) / 24.0
            }
        }
    }
}

impl DensityFunction for Unary {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = match self.operation {
            UnaryOperation::Abs => info_span!("Unary::Abs::sample").entered(),
            UnaryOperation::Square => info_span!("Unary::Square::sample").entered(),
            UnaryOperation::Cube => info_span!("Unary::Cube::sample").entered(),
            UnaryOperation::HalfNegative => info_span!("Unary::HalfNegative::sample").entered(),
            UnaryOperation::QuarterNegative => {
                info_span!("Unary::QuarterNegative::sample").entered()
            }
            UnaryOperation::Invert => info_span!("Unary::Invert::sample").entered(),
            UnaryOperation::Squeeze => info_span!("Unary::Squeeze::sample").entered(),
        };
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        self.operation.apply(density)
    }
}

impl RangeFunction for Unary {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, PartialEq)]
struct ShiftedNoise {
    input_x_index: usize,
    input_y_index: usize,
    input_z_index: usize,
    xz_scale: f32,
    y_scale: f32,
    sampler: NoiseSampler,
}

impl Debug for ShiftedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftedNoise")
            .field("input_x_index", &self.input_x_index)
            .field("input_y_index", &self.input_y_index)
            .field("input_z_index", &self.input_z_index)
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl DensityFunction for ShiftedNoise {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let shifted_x =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_x_index], pos);
        let shifted_y =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_y_index], pos);
        let shifted_z =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_z_index], pos);

        self.sampler.get(
            pos.x as f32 * self.xz_scale + shifted_x,
            pos.y as f32 * self.y_scale + shifted_y,
            pos.z as f32 * self.xz_scale + shifted_z,
        )
    }
}

impl RangeFunction for ShiftedNoise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.sampler.max_value()
    }
}

#[derive(Clone, PartialEq)]
struct WeirdScaled {
    input_index: usize,
    sampler: NoiseSampler,
    mapper: RarityValueMapper,
}

impl Debug for WeirdScaled {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeirdScaled")
            .field("input_index", &self.input_index)
            .field("mapper", &self.mapper)
            .field("min_value", &self.min_value())
            .field("max_value", &self.max_value())
            .finish()
    }
}

impl RangeFunction for WeirdScaled {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        let max_multiplier = match self.mapper {
            RarityValueMapper::Type1 => 2.0,
            RarityValueMapper::Type2 => 3.0,
        };
        max_multiplier * self.sampler.max_value()
    }
}

impl DensityFunction for WeirdScaled {
    // todo: branchless
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = info_span!("WeirdScaled::sample").entered();

        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        let (amp, coord_mul) = match self.mapper {
            RarityValueMapper::Type1 => {
                if density < -0.5 {
                    (0.75, 1.0 / 0.75)
                } else if density < 0.0 {
                    (1.0, 1.0)
                } else if density < 0.5 {
                    (1.5, 1.0 / 1.5)
                } else {
                    (2.0, 0.5)
                }
            }
            RarityValueMapper::Type2 => {
                if density < -0.75 {
                    (0.5, 2.0)
                } else if density < -0.5 {
                    (0.75, 1.0 / 0.75)
                } else if density < 0.5 {
                    (1.0, 1.0)
                } else if density < 0.75 {
                    (2.0, 0.5)
                } else {
                    (3.0, 1.0 / 3.0)
                }
            }
        };
        amp * self.sampler.get(
            pos.x as f32 * coord_mul,
            pos.y as f32 * coord_mul,
            pos.z as f32 * coord_mul,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Clamp {
    input_index: usize,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for Clamp {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for Clamp {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = info_span!("Clamp::sample").entered();

        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        density.clamp(self.min_value, self.max_value)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RangeChoice {
    input_index: usize,
    when_in_index: usize,
    when_out_index: usize,
    min_inclusion_value: f32,
    max_exclusion_value: f32,
    min_value: f32,
    max_value: f32,
}

impl RangeFunction for RangeChoice {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for RangeChoice {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = info_span!("RangeChoice::sample").entered();

        let input_density =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);

        let idx = if input_density >= self.min_inclusion_value
            && input_density < self.max_exclusion_value
        {
            self.when_in_index
        } else {
            self.when_out_index
        };
        DensityFunctionComponent::sample_from_stack(&stack[..=idx], pos)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SplineValue {
    Spline(Spline),
    Constant(f32),
}

impl RangeFunction for SplineValue {
    fn min_value(&self) -> f32 {
        match self {
            SplineValue::Spline(x) => x.min_value(),
            SplineValue::Constant(x) => *x,
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            SplineValue::Spline(x) => x.max_value(),
            SplineValue::Constant(x) => *x,
        }
    }
}

impl DensityFunction for SplineValue {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample(stack, pos),
            SplineValue::Constant(x) => *x,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    left: f32,
    inv_dist: f32,         // 1 / (x[i+1] - x[i])
    lower_deriv_dist: f32, // d[i]   * dist
    upper_deriv_dist: f32, // d[i+1] * dist
}

#[derive(Clone, Debug, PartialEq)]
struct Spline {
    input_index: usize,
    min_value: f32,
    max_value: f32,
    locations: Box<[f32]>,
    derivatives: Box<[f32]>,
    values: Box<[SplineValue]>,
    segments: Box<[Segment]>, // len = locations.len() - 1
}

impl Spline {
    pub fn new(
        input_index: usize,
        coordinate_min: f32,
        coordinate_max: f32,
        locations: Vec<f32>,
        derivatives: Vec<f32>,
        values: Vec<SplineValue>,
    ) -> Self {
        let n = locations.len() - 1;

        let mut min_value = f32::INFINITY;
        let mut max_value = f32::NEG_INFINITY;

        if coordinate_min < locations[0] {
            let extend_min = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].min_value(),
                &derivatives,
                0,
            );
            let extend_max = Self::linear_extend(
                coordinate_min,
                &locations,
                values[0].max_value(),
                &derivatives,
                0,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        if coordinate_max > locations[n] {
            let extend_min = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].min_value(),
                &derivatives,
                n,
            );
            let extend_max = Self::linear_extend(
                coordinate_max,
                &locations,
                values[n].max_value(),
                &derivatives,
                n,
            );
            min_value = min_value.min(extend_min.min(extend_max));
            max_value = max_value.max(extend_min.max(extend_max));
        }

        values.iter().for_each(|v| {
            min_value = min_value.min(v.min_value());
            max_value = max_value.max(v.max_value());
        });

        for i in 0..n {
            let location_left = locations[i];
            let location_right = locations[i + 1];
            let location_delta = location_right - location_left;

            let min_left = values[i].min_value();
            let max_left = values[i].max_value();
            let min_right = values[i + 1].min_value();
            let max_right = values[i + 1].max_value();

            let derivative_left = derivatives[i];
            let derivative_right = derivatives[i + 1];

            if derivative_left != 0.0 || derivative_right != 0.0 {
                let max_value_delta_left = derivative_left * location_delta;
                let max_value_delta_right = derivative_right * location_delta;

                let mut local_min = min_left.min(min_right);
                let mut local_max = max_left.max(max_right);

                let min_delta_left = max_value_delta_left - max_right + min_left;
                let max_delta_left = max_value_delta_left - min_right + max_left;

                let min_delta_right = -max_value_delta_right + min_right - min_left;
                let max_delta_right = -max_value_delta_right + max_right - min_left;

                let min_delta = min_delta_left.min(min_delta_right);
                let max_delta = max_delta_left.max(max_delta_right);

                local_min = local_min.min(local_min + 0.25 * min_delta);
                local_max = local_max.max(local_max + 0.25 * max_delta);

                min_value = min_value.min(local_min);
                max_value = max_value.max(local_max);
            }
        }

        let mut segs = Vec::with_capacity(n);
        for i in 0..n {
            let left = locations[i];
            let dist = locations[i + 1] - left;
            debug_assert!(dist > 0.0, "locations must be strictly increasing");
            segs.push(Segment {
                left,
                inv_dist: 1.0 / dist,
                lower_deriv_dist: derivatives[i] * dist,
                upper_deriv_dist: derivatives[i + 1] * dist,
            });
        }

        Self {
            input_index,
            min_value,
            max_value,
            locations: locations.into_boxed_slice(),
            derivatives: derivatives.into_boxed_slice(),
            values: values.into_boxed_slice(),
            segments: segs.into_boxed_slice(),
        }
    }

    #[inline]
    fn linear_extend(
        point: f32,
        locations: &[f32],
        value: f32,
        derivatives: &[f32],
        i: usize,
    ) -> f32 {
        let f = derivatives[i];
        if f == 0.0 {
            value
        } else {
            value + f * (point - locations[i])
        }
    }

    #[inline(always)]
    fn upper_bound(xs: &[f32], x: f32) -> usize {
        // index of first element > x  (upper_bound)
        match xs.binary_search_by(|v| v.total_cmp(&x)) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }

    #[inline(always)]
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        (b - a).mul_add(t, a)
    }
}

impl RangeFunction for Spline {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for Spline {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let location =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample(stack, pos);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                d0.mul_add(location - locs[0], v0)
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample(stack, pos);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                d.mul_add(location - locs[i], v)
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample(stack, pos);
        let v1 = self.values[i1].sample(stack, pos);

        let seg = self.segments[i0];
        let x = (location - seg.left) * seg.inv_dist;

        let delta = v1 - v0;

        let e0 = seg.lower_deriv_dist - delta;
        let e1 = -seg.upper_deriv_dist + delta;

        let cubic = (x * (1.0 - x)) * Self::lerp(e0, e1, x);
        let linear = Self::lerp(v0, v1, x);

        cubic + linear
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FindTopSurface {
    density_index: usize,
    upper_bound_index: usize,
    lower_bound: f32,
    cell_height: f32,
    max_value: f32,
}

impl RangeFunction for FindTopSurface {
    #[inline]
    fn min_value(&self) -> f32 {
        self.lower_bound
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for FindTopSurface {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = info_span!("FindTopSurface::sample").entered();

        let top_y =
            (DensityFunctionComponent::sample_from_stack(&stack[..=self.upper_bound_index], pos)
                / self.cell_height)
                .floor()
                * self.cell_height;
        if top_y <= self.lower_bound {
            self.lower_bound
        } else {
            let mut current_y = top_y;
            loop {
                let sample_pos = IVec3::new(pos.x, current_y as i32, pos.z);
                let density = DensityFunctionComponent::sample_from_stack(
                    &stack[..=self.density_index],
                    sample_pos,
                );
                if density > 0.0 || current_y <= self.lower_bound {
                    return current_y;
                }
                current_y -= self.cell_height;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Binary {
    input1_index: usize,
    input2_index: usize,
    min_value: f32,
    max_value: f32,
    operation: BinaryOperation,
}

impl DensityFunction for Binary {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let _span = match self.operation {
            BinaryOperation::Add => info_span!("Binary::Add::sample"),
            BinaryOperation::Multiply => info_span!("Binary::Multiply::sample"),
            BinaryOperation::Min => info_span!("Binary::Min::sample"),
            BinaryOperation::Max => info_span!("Binary::Max::sample"),
        }
        .entered();
        let input1_density =
            DensityFunctionComponent::sample_from_stack(&stack[..=self.input1_index], pos);
        match self.operation {
            BinaryOperation::Add => {
                let input2_density =
                    DensityFunctionComponent::sample_from_stack(&stack[..=self.input2_index], pos);
                input1_density + input2_density
            }
            BinaryOperation::Multiply => {
                if input1_density == 0.0 {
                    0.0
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density * input2_density
                }
            }
            BinaryOperation::Min => {
                let input2_min = stack[self.input2_index].min_value();
                if input1_density < input2_min {
                    input1_density
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density.min(input2_density)
                }
            }
            BinaryOperation::Max => {
                let input2_max = stack[self.input2_index].max_value();
                if input1_density > input2_max {
                    input1_density
                } else {
                    let input2_density = DensityFunctionComponent::sample_from_stack(
                        &stack[..=self.input2_index],
                        pos,
                    );
                    input1_density.max(input2_density)
                }
            }
        }
    }
}

impl RangeFunction for Binary {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum BinaryOperation {
    Add,
    Multiply,
    Min,
    Max,
}

#[derive(Clone, Debug, PartialEq)]
enum DensityFunctionComponent {
    Independent(IndependentDensityFunction),
    Dependent(DependentDensityFunction),
    Wrapper(WrapperDensityFunction),
}

impl DensityFunctionComponent {
    fn as_constant(&self) -> Option<f32> {
        match self {
            DensityFunctionComponent::Independent(x) => match x {
                IndependentDensityFunction::Constant(v) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }
}

impl TryFrom<DensityFunctionComponent> for f32 {
    type Error = ();

    fn try_from(value: DensityFunctionComponent) -> Result<Self, Self::Error> {
        if let Some(v) = value.as_constant() {
            Ok(v)
        } else {
            Err(())
        }
    }
}

impl DensityFunctionComponent {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.sample(stack, pos),
            DensityFunctionComponent::Dependent(func) => func.sample(stack, pos),
            DensityFunctionComponent::Wrapper(func) => func.sample(stack, pos),
        }
    }

    fn sample_from_stack(stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let (top_component, component_stack) = stack.split_last().unwrap();
        top_component.sample(component_stack, pos)
    }
}

impl RangeFunction for DensityFunctionComponent {
    fn min_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.min_value(),
            DensityFunctionComponent::Dependent(func) => func.min_value(),
            DensityFunctionComponent::Wrapper(func) => func.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.max_value(),
            DensityFunctionComponent::Dependent(func) => func.max_value(),
            DensityFunctionComponent::Wrapper(func) => func.max_value(),
        }
    }
}

struct FunctionStackBuilder<'a> {
    random: RandomSource,
    functions: &'a BTreeMap<Ident<String>, ProtoDensityFunction>,
    noises: &'a BTreeMap<Ident<String>, NoiseParam>,
    stack: Vec<DensityFunctionComponent>,
    built: HashMap<ProtoDensityFunction, usize>,
    builder_options: &'a ChunkNoiseFunctionBuilderOptions,
}

impl<'a> FunctionStackBuilder<'a> {
    fn new(
        random: RandomSource,
        functions: &'a BTreeMap<Ident<String>, ProtoDensityFunction>,
        noises: &'a BTreeMap<Ident<String>, NoiseParam>,
        builder_options: &'a ChunkNoiseFunctionBuilderOptions,
    ) -> Self {
        Self {
            random,
            functions,
            noises,
            stack: Vec::new(),
            built: HashMap::new(),
            builder_options,
        }
    }
}

impl<'a> FunctionStackBuilder<'a> {
    fn get_index(&mut self, holder: &DensityFunctionHolder) -> Option<usize> {
        match holder {
            DensityFunctionHolder::Value(x) => self
                .built
                .get(&ProtoDensityFunction::Constant(x.clone()))
                .copied(),
            DensityFunctionHolder::Reference(x) => self.built.get(&self.functions[x]).copied(),
            DensityFunctionHolder::Owned(x) => self.built.get(x).copied(),
        }
    }

    fn component(&mut self, holder: &DensityFunctionHolder) -> (usize) {
        self.visit_density_function_holder(holder);
        let idx = self.get_index(holder);
        if idx.is_none() {
            panic!("Component not found after visiting: {:?}", holder);
        }
        let idx = idx.unwrap();
        idx
    }

    fn register_component(
        &mut self,
        proto_density_function: ProtoDensityFunction,
        component: DensityFunctionComponent,
    ) -> usize {
        let idx = self.get_index(&DensityFunctionHolder::Owned(
            proto_density_function.clone().into(),
        ));
        if let Some(index) = idx {
            return index;
        }

        idx.unwrap_or_else(|| {
            let pos = self.stack.iter().position(|c| c == &component);
            if let Some(index) = pos {
                self.built.insert(proto_density_function, index);
                return index;
            }
            let index = self.stack.len();
            self.built.insert(proto_density_function, index);
            self.stack.push(component);
            index
        })
    }
}

impl<'a> Visitor for FunctionStackBuilder<'a> {
    fn visit_constant(&mut self, value: f64) {
        self.register_component(
            ProtoDensityFunction::Constant(value.into()),
            DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(
                value as f32,
            )),
        );
    }

    fn visit_reference(&mut self, value: &Ident<String>) {
        if let Some(x) = self.functions.get(value) {
            self.visit_density_function(&x);
            return;
        }
    }

    fn visit_blend_alpha(&mut self) {
        self.register_component(
            ProtoDensityFunction::BlendAlpha,
            DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(1.0)),
        );
    }

    fn visit_blend_offset(&mut self) {
        self.register_component(
            ProtoDensityFunction::BlendOffset,
            DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(0.0)),
        );
    }

    fn visit_beardifier(&mut self) {
        self.register_component(
            ProtoDensityFunction::Beardifier,
            DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(0.0)),
        );
    }

    fn visit_blend_density(&mut self, function: &SingleArgumentFunction) {
        let (input_index) = self.component(&function.argument);
        let comp = &self.stack[input_index];
        self.register_component(
            ProtoDensityFunction::BlendDensity(SingleArgumentFunction {
                argument: function.argument.clone(),
            }),
            comp.clone(),
        );
    }

    fn visit_flat_cache(&mut self, function: &SingleArgumentFunction) {
        let (input_index) = self.component(&function.argument);
        let input = &self.stack[input_index];
        let min_value = input.min_value();
        let max_value = input.max_value();
        self.register_component(
            ProtoDensityFunction::FlatCache(SingleArgumentFunction {
                argument: function.argument.clone(),
            }),
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::FlatCache(FlatCache {
                input_index,
                min_value,
                max_value,
            })),
        );
    }

    fn visit_cache2d(&mut self, function: &SingleArgumentFunction) {
        let (input_index) = self.component(&function.argument);
        let input = &self.stack[input_index];
        let min_value = input.min_value();
        let max_value = input.max_value();
        self.register_component(
            ProtoDensityFunction::Cache2d(SingleArgumentFunction {
                argument: function.argument.clone(),
            }),
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::Cache2d(Cache2d {
                input_index,
                min_value,
                max_value,
            })),
        );
    }

    fn visit_cache_once(&mut self, function: &SingleArgumentFunction) {
        let (input_index) = self.component(&function.argument);
        let input = &self.stack[input_index];
        let proto = ProtoDensityFunction::CacheOnce(SingleArgumentFunction {
            argument: function.argument.clone(),
        });
        if let Some(constant) = input.as_constant() {
            self.register_component(
                proto,
                DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(
                    constant,
                )),
            );
            return;
        }
        let min_value = input.min_value();
        let max_value = input.max_value();
        self.register_component(
            proto,
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::CacheOnce(CacheOnce {
                input_index,
                min_value,
                max_value,
            })),
        );
    }

    fn visit_cache_all_in_cell(&mut self, function: &SingleArgumentFunction) {
        let (input_index) = self.component(&function.argument);
        let input = &self.stack[input_index];
        let min_value = input.min_value();
        let max_value = input.max_value();
        self.register_component(
            ProtoDensityFunction::CacheAllInCell(SingleArgumentFunction {
                argument: function.argument.clone(),
            }),
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::CacheAllInCell(
                CacheAllInCell {
                    input_index,
                    min_value,
                    max_value,
                },
            )),
        );
    }

    fn visit_abs(&mut self, arg: &SingleArgumentFunction) {
        self.unary(arg, UnaryOperation::Abs);
    }

    fn visit_square(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Square);
    }

    fn visit_cube(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Cube);
    }

    fn visit_half_negative(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::HalfNegative);
    }

    fn visit_quarter_negative(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::QuarterNegative);
    }

    fn visit_invert(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Invert);
    }

    fn visit_squeeze(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Squeeze);
    }

    fn visit_add(&mut self, arg: &TwoArgumentFunction) {
        self.binary(arg, BinaryOperation::Add);
    }

    fn visit_mul(&mut self, function: &TwoArgumentFunction) {
        self.binary(function, BinaryOperation::Multiply);
    }

    fn visit_min(&mut self, function: &TwoArgumentFunction) {
        self.binary(function, BinaryOperation::Min);
    }

    fn visit_max(&mut self, function: &TwoArgumentFunction) {
        self.binary(function, BinaryOperation::Max);
    }

    fn visit_old_blended_noise(
        &mut self,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) {
        let mut random = if let RandomSource::Legacy(_) = self.random {
            RandomSource::new(0, true)
        } else {
            self.random.clone().fork_hash("minecraft:terrain")
        };
        let blended = OldBlendedNoise::new(
            &mut random,
            xz_scale as f32,
            y_scale as f32,
            xz_factor as f32,
            y_factor as f32,
            smear_scale_multiplier as f32,
        );
        self.register_component(
            ProtoDensityFunction::OldBlendedNoise {
                xz_scale: xz_scale.into(),
                y_scale: y_scale.into(),
                xz_factor: xz_factor.into(),
                y_factor: y_factor.into(),
                smear_scale_multiplier: smear_scale_multiplier.into(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::OldBlendedNoise(
                blended,
            )),
        );
    }

    fn visit_noise(&mut self, noise_holder: &NoiseHolder, xz_scale: f64, y_scale: f64) {
        let sampler = self.noise_sampler(noise_holder);
        let proto = ProtoDensityFunction::Noise {
            noise: noise_holder.clone(),
            xz_scale: xz_scale.into(),
            y_scale: y_scale.into(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Independent(IndependentDensityFunction::Noise(Noise {
                sampler,
                xz_scale: xz_scale as f32,
                y_scale: y_scale as f32,
            })),
        );
    }

    fn visit_weird_scaled_sampler(
        &mut self,
        input: &DensityFunctionHolder,
        noise: &NoiseHolder,
        rarity_value_mapper: &RarityValueMapper,
    ) {
        let (input_index) = self.component(input);
        let sampler = self.noise_sampler(noise);
        let proto = ProtoDensityFunction::WeirdScaledSampler {
            input: input.clone(),
            noise: noise.clone(),
            rarity_value_mapper: *rarity_value_mapper,
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::WeirdScaled(
                WeirdScaled {
                    input_index,
                    sampler,
                    mapper: *rarity_value_mapper,
                },
            )),
        );
    }

    fn visit_shifted_noise(
        &mut self,
        shift_x: &DensityFunctionHolder,
        shift_y: &DensityFunctionHolder,
        shift_z: &DensityFunctionHolder,
        xz_scale: f64,
        y_scale: f64,
        noise: &NoiseHolder,
    ) {
        let (input_x_index) = self.component(shift_x);
        let (input_y_index) = self.component(shift_y);
        let (input_z_index) = self.component(shift_z);
        let sampler = self.noise_sampler(noise);
        let proto = ProtoDensityFunction::ShiftedNoise {
            shift_x: shift_x.clone(),
            shift_y: shift_y.clone(),
            shift_z: shift_z.clone(),
            xz_scale: xz_scale.into(),
            y_scale: y_scale.into(),
            noise: noise.clone(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::ShiftedNoise(
                ShiftedNoise {
                    input_x_index,
                    input_y_index,
                    input_z_index,
                    xz_scale: xz_scale as f32,
                    y_scale: y_scale as f32,
                    sampler,
                },
            )),
        );
    }

    fn visit_range_choice(
        &mut self,
        input: &DensityFunctionHolder,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: &DensityFunctionHolder,
        when_out_of_range: &DensityFunctionHolder,
    ) {
        let (input_index) = self.component(input);
        let (when_in_index) = self.component(when_in_range);
        let (when_out_index) = self.component(when_out_of_range);
        let min_value = self.stack[when_in_index]
            .min_value()
            .min(self.stack[when_out_index].min_value());
        let max_value = self.stack[when_in_index]
            .max_value()
            .max(self.stack[when_out_index].max_value());
        let proto = ProtoDensityFunction::RangeChoice {
            input: input.clone(),
            min_inclusive: min_inclusive.into(),
            max_exclusive: max_exclusive.into(),
            when_in_range: when_in_range.clone(),
            when_out_of_range: when_out_of_range.clone(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::RangeChoice(
                RangeChoice {
                    input_index,
                    when_in_index,
                    when_out_index,
                    min_inclusion_value: min_inclusive as f32,
                    max_exclusion_value: max_exclusive as f32,
                    min_value,
                    max_value,
                },
            )),
        );
    }

    fn visit_shift_a(&mut self, function: &NoiseHolder) {
        let sampler = self.noise_sampler(function);
        self.register_component(
            ProtoDensityFunction::ShiftA {
                argument: function.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::ShiftA(ShiftA {
                sampler,
            })),
        );
    }

    fn visit_shift_b(&mut self, function: &NoiseHolder) {
        let sampler = self.noise_sampler(function);
        self.register_component(
            ProtoDensityFunction::ShiftB {
                argument: function.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::ShiftB(ShiftB {
                sampler,
            })),
        );
    }

    fn visit_shift(&mut self, argument: &NoiseHolder) {
        let sampler = self.noise_sampler(argument);
        self.register_component(
            ProtoDensityFunction::Shift {
                argument: argument.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::Shift(Shift {
                sampler,
            })),
        );
    }

    fn visit_clamp(&mut self, input: &DensityFunctionHolder, min: f64, max: f64) {
        let (input_index) = self.component(input);
        let proto = ProtoDensityFunction::Clamp {
            input: input.clone(),
            min: min.into(),
            max: max.into(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::Clamp(Clamp {
                input_index,
                min_value: min as f32,
                max_value: max as f32,
            })),
        );
    }

    fn visit_spline(&mut self, spline: &SplineHolder) {
        let value = self.spline_value(spline);
        match value {
            SplineValue::Constant(x) => {
                self.register_component(
                    ProtoDensityFunction::Constant((x as f64).into()),
                    DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(x)),
                );
            }
            SplineValue::Spline(x) => {
                self.register_component(
                    ProtoDensityFunction::Spline {
                        spline: spline.clone(),
                    },
                    DensityFunctionComponent::Dependent(DependentDensityFunction::Spline(x)),
                );
            }
        }
    }

    fn visit_y_clamped_gradient(&mut self, from_y: i32, to_y: i32, from_value: f64, to_value: f64) {
        self.register_component(
            ProtoDensityFunction::YClampedGradient {
                from_y: from_y.into(),
                to_y: to_y.into(),
                from_value: from_value.into(),
                to_value: to_value.into(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::ClampedYGradient(
                ClampedYGradient {
                    from_y: from_y as f32,
                    to_y: to_y as f32,
                    from_value: from_value as f32,
                    to_value: to_value as f32,
                },
            )),
        );
    }

    fn visit_find_top_surface(
        &mut self,
        density: &DensityFunctionHolder,
        upper_bound: &DensityFunctionHolder,
        lower_bound: i32,
        cell_height: u32,
    ) {
        let (density_index) = self.component(density);
        let (upper_bound_index) = self.component(upper_bound);
        let max_value = self.stack[upper_bound_index]
            .max_value()
            .max(lower_bound as f32);
        let proto = ProtoDensityFunction::FindTopSurface {
            density: density.clone(),
            upper_bound: upper_bound.clone(),
            lower_bound: lower_bound.into(),
            cell_height: cell_height.into(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::FindTopSurface(
                FindTopSurface {
                    density_index,
                    upper_bound_index,
                    lower_bound: lower_bound as f32,
                    cell_height: cell_height as f32,
                    max_value,
                },
            )),
        );
    }

    fn visit_interpolated(&mut self, function: &SingleArgumentFunction) {
        let input_index = self.component(&function.argument);
        let input = &self.stack[input_index];
        let min_value = input.min_value();
        let max_value = input.max_value();

        self.register_component(
            ProtoDensityFunction::Interpolated(SingleArgumentFunction {
                argument: function.argument.clone(),
            }),
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::Interpolated(
                Interpolated::new(input_index, min_value, max_value, self.builder_options),
            )),
        );
    }
}

impl<'a> FunctionStackBuilder<'a> {
    fn spline_value(&mut self, spline_holder: &SplineHolder) -> SplineValue {
        match spline_holder {
            SplineHolder::Constant(x) => SplineValue::Constant(x.0 as f32),
            SplineHolder::Spline(x) => SplineValue::Spline(self.spline(x)),
        }
    }

    fn spline(&mut self, proto_spline: &proto::Spline) -> Spline {
        let (cord_index) = self.component(&proto_spline.coordinate);
        let cord_comp = &self.stack[cord_index];
        let coord_min = cord_comp.min_value();
        let coord_max = cord_comp.max_value();
        let mut values = Vec::with_capacity(proto_spline.points.len());
        let mut derivatives = Vec::with_capacity(proto_spline.points.len());
        let mut locations = Vec::with_capacity(proto_spline.points.len());
        for p in &proto_spline.points {
            values.push(self.spline_value(&p.value));
            derivatives.push(p.derivative.0 as f32);
            locations.push(p.location.0 as f32);
        }
        Spline::new(
            cord_index,
            coord_min,
            coord_max,
            locations,
            derivatives,
            values,
        )
    }

    fn binary(&mut self, arg: &TwoArgumentFunction, operation: BinaryOperation) {
        let (input1_index) = self.component(&arg.argument1);
        let arg1 = &self.stack[input1_index];
        let min1 = arg1.min_value();
        let max1 = arg1.max_value();
        let arg1_constant = arg1.as_constant();

        let (input2_index) = self.component(&arg.argument2);
        let arg2 = &self.stack[input2_index];
        let min2 = arg2.min_value();
        let max2 = arg2.max_value();
        let arg2_constant = arg2.as_constant();

        let (min_value, max_value) = match operation {
            BinaryOperation::Add => (min1 + min2, max1 + max2),
            BinaryOperation::Multiply => {
                let min = if min1 > 0.0 && min2 > 0.0 {
                    min1 * min2
                } else if max1 < 0.0 && max2 < 0.0 {
                    max1 * max2
                } else {
                    (min1 * max2).min(max1 * min2)
                };

                let max = if min1 > 0.0 && min2 > 0.0 {
                    max1 * max2
                } else if max1 < 0.0 && max2 < 0.0 {
                    min1 * min2
                } else {
                    (min1 * min2).max(max1 * max2)
                };

                (min, max)
            }
            BinaryOperation::Min => (min1.min(min2), max1.min(max2)),
            BinaryOperation::Max => (min1.max(min2), max1.max(max2)),
        };
        let proto = match operation {
            BinaryOperation::Add => ProtoDensityFunction::Add(arg.clone()),
            BinaryOperation::Multiply => ProtoDensityFunction::Mul(arg.clone()),
            BinaryOperation::Min => ProtoDensityFunction::Min(arg.clone()),
            BinaryOperation::Max => ProtoDensityFunction::Max(arg.clone()),
        };

        if let BinaryOperation::Add | BinaryOperation::Multiply = operation {
            if let Some((input_index, argument)) = match (arg1_constant, arg2_constant) {
                (Some(x), None) => Some((input2_index, x)),
                (None, Some(x)) => Some((input1_index, x)),
                _ => None,
            } {
                self.register_component(
                    proto,
                    DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(Linear {
                        input_index,
                        min_value,
                        max_value,
                        argument,
                        operation: match operation {
                            BinaryOperation::Add => LinearOperation::Add,
                            BinaryOperation::Multiply => LinearOperation::Multiply,
                            _ => unreachable!(),
                        },
                    })),
                );
                return;
            }
        }
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::Binary(Binary {
                input1_index,
                input2_index,
                min_value,
                max_value,
                operation,
            })),
        );
    }

    fn unary(&mut self, arg: &SingleArgumentFunction, operation: UnaryOperation) {
        let (input_index) = self.component(&arg.argument);
        let input = &self.stack[input_index];
        let min = input.min_value();
        let max = input.max_value();
        let min_image = operation.apply(min);
        let max_image = operation.apply(max);
        let proto = match operation {
            UnaryOperation::Abs => ProtoDensityFunction::Abs(arg.clone()),
            UnaryOperation::Square => ProtoDensityFunction::Square(arg.clone()),
            UnaryOperation::Cube => ProtoDensityFunction::Cube(arg.clone()),
            UnaryOperation::HalfNegative => ProtoDensityFunction::HalfNegative(arg.clone()),
            UnaryOperation::QuarterNegative => ProtoDensityFunction::QuarterNegative(arg.clone()),
            UnaryOperation::Invert => ProtoDensityFunction::Invert(arg.clone()),
            UnaryOperation::Squeeze => ProtoDensityFunction::Squeeze(arg.clone()),
        };

        let (min_value, max_value) = match operation {
            UnaryOperation::Invert => {
                if min < 0.0 && max > 0.0 {
                    (f32::NEG_INFINITY, f32::INFINITY)
                } else {
                    (max_image, min_image)
                }
            }
            UnaryOperation::Abs | UnaryOperation::Square => {
                (min_image.max(0.0), min_image.max(max_image))
            }
            _ => (min_image, max_image),
        };

        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::Unary(Unary {
                input_index,
                min_value,
                max_value,
                operation,
            })),
        );
    }

    fn noise_sampler(&mut self, holder: &NoiseHolder) -> NoiseSampler {
        match holder {
            NoiseHolder::Reference(x) => self.create_noise(x),
            NoiseHolder::Owned(x) => NoiseSampler::new(
                &mut self.random.clone(),
                x.first_octave,
                x.amplitudes.iter().map(|x| x.0 as f32).collect(),
            ),
        }
    }

    fn create_noise(&mut self, id: &Ident<String>) -> NoiseSampler {
        if let RandomSource::Legacy(r) = &self.random {
            match id.as_str() {
                "minecraft:temperature" => {
                    return NoiseSampler::new(&mut LegacyRandom::new(r.seed), -7, vec![1.0, 1.0]);
                }
                "minecraft:vegetation" => {
                    return NoiseSampler::new(
                        &mut LegacyRandom::new(r.seed + 1),
                        -7,
                        vec![1.0, 1.0],
                    );
                }
                "minecraft:offset" => {
                    return NoiseSampler::new(
                        &mut self.random.clone().fork_hash("minecraft:offset"),
                        0,
                        vec![0.0],
                    );
                }
                _ => {}
            }
        }

        let mut random = self.random.clone().fork_hash(id.as_str());
        let noise_param = self.noises.get(id);
        if noise_param.is_none() {
            panic!("Noise not loaded: {}", id);
        }
        let noise_param = noise_param.unwrap();
        NoiseSampler::new(
            &mut random,
            noise_param.first_octave,
            noise_param.amplitudes.iter().map(|x| x.0 as f32).collect(),
        )
    }
}

#[inline]
pub fn lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}
