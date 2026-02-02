use mcrs_protocol::Ident;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HashableF64(pub f64);

// Normally this is bad, but we just care about checking if components are the same
impl Hash for HashableF64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_le_bytes().hash(state);
    }
}

impl Eq for HashableF64 {}

impl From<f64> for HashableF64 {
    #[inline]
    fn from(value: f64) -> Self {
        HashableF64(value)
    }
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "bevy", derive(bevy_asset::Asset, bevy_reflect::TypePath))]
pub enum DensityFunctionHolder {
    Value(HashableF64),
    Reference(Ident<String>),
    Owned(Box<ProtoDensityFunction>),
}

impl From<SingleArgumentFunction> for DensityFunctionHolder {
    fn from(func: SingleArgumentFunction) -> Self {
        func.argument
    }
}

#[derive(Hash, Eq, PartialEq, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ProtoDensityFunction {
    #[serde(alias = "minecraft:blend_alpha")]
    BlendAlpha,
    #[serde(alias = "minecraft:blend_offset")]
    BlendOffset,
    #[serde(alias = "minecraft:beardifier")]
    Beardifier,
    #[serde(alias = "old_blended_noise", rename = "minecraft:old_blended_noise")]
    OldBlendedNoise {
        xz_scale: HashableF64,
        y_scale: HashableF64,
        xz_factor: HashableF64,
        y_factor: HashableF64,
        smear_scale_multiplier: HashableF64,
    },
    #[serde(alias = "interpolated", rename = "minecraft:interpolated")]
    Interpolated(SingleArgumentFunction),
    #[serde(alias = "minecraft:flat_cache")]
    FlatCache(SingleArgumentFunction),
    #[serde(alias = "minecraft:cache_2d")]
    Cache2d(SingleArgumentFunction),
    #[serde(alias = "minecraft:cache_once")]
    CacheOnce(SingleArgumentFunction),
    #[serde(alias = "minecraft:cache_all_in_cell")]
    CacheAllInCell(SingleArgumentFunction),
    #[serde(alias = "noise", rename = "minecraft:noise")]
    Noise {
        noise: NoiseHolder,
        xz_scale: HashableF64,
        y_scale: HashableF64,
    },
    #[serde(alias = "minecraft:end_islands")]
    EndIslands,
    #[serde(alias = "minecraft:weird_scaled_sampler")]
    WeirdScaledSampler {
        input: DensityFunctionHolder,
        noise: NoiseHolder,
        rarity_value_mapper: RarityValueMapper,
    },
    #[serde(alias = "minecraft:shifted_noise")]
    ShiftedNoise {
        shift_x: DensityFunctionHolder,
        shift_y: DensityFunctionHolder,
        shift_z: DensityFunctionHolder,
        xz_scale: HashableF64,
        y_scale: HashableF64,
        noise: NoiseHolder,
    },
    #[serde(rename = "minecraft:range_choice")]
    RangeChoice {
        input: DensityFunctionHolder,
        min_inclusive: HashableF64,
        max_exclusive: HashableF64,
        when_in_range: DensityFunctionHolder,
        when_out_of_range: DensityFunctionHolder,
    },
    #[serde(alias = "minecraft:shift_a")]
    ShiftA { argument: NoiseHolder },
    #[serde(alias = "minecraft:shift_b")]
    ShiftB { argument: NoiseHolder },
    #[serde(rename = "minecraft:shift", alias = "shift")]
    Shift { argument: NoiseHolder },
    #[serde(rename = "minecraft:blend_density", alias = "blend_density")]
    BlendDensity(SingleArgumentFunction),
    #[serde(alias = "minecraft:clamp")]
    Clamp {
        input: DensityFunctionHolder,
        min: HashableF64,
        max: HashableF64,
    },
    #[serde(alias = "minecraft:abs")]
    Abs(SingleArgumentFunction),
    #[serde(alias = "minecraft:square")]
    Square(SingleArgumentFunction),
    #[serde(alias = "minecraft:cube")]
    Cube(SingleArgumentFunction),
    #[serde(alias = "minecraft:half_negative")]
    HalfNegative(SingleArgumentFunction),
    #[serde(alias = "minecraft:quarter_negative")]
    QuarterNegative(SingleArgumentFunction),
    #[serde(alias = "minecraft:invert")]
    Invert(SingleArgumentFunction),
    #[serde(alias = "minecraft:squeeze")]
    Squeeze(SingleArgumentFunction),
    #[serde(alias = "minecraft:add")]
    Add(TwoArgumentFunction),
    #[serde(alias = "minecraft:mul")]
    Mul(TwoArgumentFunction),
    #[serde(alias = "minecraft:min")]
    Min(TwoArgumentFunction),
    #[serde(alias = "minecraft:max")]
    Max(TwoArgumentFunction),
    #[serde(alias = "minecraft:spline")]
    Spline { spline: SplineHolder },
    #[serde(alias = "minecraft:constant")]
    Constant(HashableF64),
    #[serde(alias = "minecraft:y_clamped_gradient")]
    YClampedGradient {
        from_y: i32,
        to_y: i32,
        from_value: HashableF64,
        to_value: HashableF64,
    },
    #[serde(alias = "minecraft:find_top_surface")]
    FindTopSurface {
        density: DensityFunctionHolder,
        upper_bound: DensityFunctionHolder,
        lower_bound: i32,
        cell_height: u32,
    },
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum NoiseHolder {
    Reference(Ident<String>),
    Owned(NoiseParam),
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoiseParam {
    #[cfg(feature = "serde")]
    #[serde(rename = "firstOctave")]
    pub first_octave: i32,
    pub amplitudes: Vec<HashableF64>,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RarityValueMapper {
    #[serde(rename = "type_1")]
    Type1,
    #[serde(rename = "type_2")]
    Type2,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SingleArgumentFunction {
    pub argument: DensityFunctionHolder,
}

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TwoArgumentFunction {
    pub argument1: DensityFunctionHolder,
    pub argument2: DensityFunctionHolder,
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[serde(untagged)]
pub enum SplineHolder {
    Constant(HashableF64),
    Spline(Spline),
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spline {
    pub coordinate: DensityFunctionHolder,
    pub points: Vec<SplinePoint>,
}

#[derive(Hash, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplinePoint {
    pub location: HashableF64,
    pub value: SplineHolder,
    pub derivative: HashableF64,
}

pub trait Visitor {
    fn visit_density_function_holder(&mut self, function: &DensityFunctionHolder) {
        match function {
            DensityFunctionHolder::Value(v) => self.visit_constant(v.0),
            DensityFunctionHolder::Reference(r) => self.visit_reference(r),
            DensityFunctionHolder::Owned(f) => self.visit_density_function(f),
        }
    }

    fn visit_constant(&mut self, value: f64) {}

    fn visit_reference(&mut self, value: &Ident<String>) {}

    fn visit_noise_holder(&mut self, noise: &NoiseHolder) {}

    fn visit_density_function(&mut self, function: &ProtoDensityFunction) {
        match function {
            ProtoDensityFunction::BlendAlpha => self.visit_blend_alpha(),
            ProtoDensityFunction::BlendOffset => self.visit_blend_offset(),
            ProtoDensityFunction::Beardifier => self.visit_beardifier(),
            ProtoDensityFunction::OldBlendedNoise {
                xz_scale,
                y_scale,
                xz_factor,
                y_factor,
                smear_scale_multiplier,
            } => self.visit_old_blended_noise(
                xz_scale.0,
                y_scale.0,
                xz_factor.0,
                y_factor.0,
                smear_scale_multiplier.0,
            ),
            ProtoDensityFunction::Interpolated(arg) => self.visit_interpolated(arg),
            ProtoDensityFunction::FlatCache(arg) => self.visit_flat_cache(arg),
            ProtoDensityFunction::Cache2d(arg) => self.visit_cache2d(arg),
            ProtoDensityFunction::CacheOnce(arg) => self.visit_cache_once(arg),
            ProtoDensityFunction::CacheAllInCell(arg) => self.visit_cache_all_in_cell(arg),
            ProtoDensityFunction::Noise {
                noise,
                xz_scale,
                y_scale,
            } => self.visit_noise(noise, xz_scale.0, y_scale.0),
            ProtoDensityFunction::EndIslands => self.visit_end_islands(),
            ProtoDensityFunction::WeirdScaledSampler {
                input,
                noise,
                rarity_value_mapper,
            } => self.visit_weird_scaled_sampler(input, noise, rarity_value_mapper),
            ProtoDensityFunction::ShiftedNoise {
                shift_x,
                shift_y,
                shift_z,
                xz_scale,
                y_scale,
                noise,
            } => self.visit_shifted_noise(shift_x, shift_y, shift_z, xz_scale.0, y_scale.0, noise),
            ProtoDensityFunction::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => self.visit_range_choice(
                input,
                min_inclusive.0,
                max_exclusive.0,
                when_in_range,
                when_out_of_range,
            ),
            ProtoDensityFunction::ShiftA { argument } => self.visit_shift_a(argument),
            ProtoDensityFunction::ShiftB { argument } => self.visit_shift_b(argument),
            ProtoDensityFunction::Shift { argument } => self.visit_shift(argument),
            ProtoDensityFunction::BlendDensity(x) => self.visit_blend_density(x),
            ProtoDensityFunction::Clamp { input, min, max } => {
                self.visit_clamp(input, min.0, max.0)
            }
            ProtoDensityFunction::Abs(x) => self.visit_abs(x),
            ProtoDensityFunction::Square(x) => self.visit_square(x),
            ProtoDensityFunction::Cube(x) => self.visit_cube(x),
            ProtoDensityFunction::HalfNegative(x) => self.visit_half_negative(x),
            ProtoDensityFunction::QuarterNegative(x) => self.visit_quarter_negative(x),
            ProtoDensityFunction::Invert(x) => self.visit_invert(x),
            ProtoDensityFunction::Squeeze(x) => self.visit_squeeze(x),
            ProtoDensityFunction::Add(x) => self.visit_add(x),
            ProtoDensityFunction::Mul(x) => self.visit_mul(x),
            ProtoDensityFunction::Min(x) => self.visit_min(x),
            ProtoDensityFunction::Max(x) => self.visit_max(x),
            ProtoDensityFunction::Spline { spline } => self.visit_spline(spline),
            ProtoDensityFunction::Constant(x) => self.visit_constant(x.0),
            ProtoDensityFunction::YClampedGradient {
                from_y,
                to_y,
                from_value,
                to_value,
            } => self.visit_y_clamped_gradient(*from_y, *to_y, from_value.0, to_value.0),
            ProtoDensityFunction::FindTopSurface {
                density,
                upper_bound,
                lower_bound,
                cell_height,
            } => self.visit_find_top_surface(density, upper_bound, *lower_bound, *cell_height),
        }
    }

    fn visit_blend_alpha(&mut self) {}
    fn visit_blend_offset(&mut self) {}
    fn visit_beardifier(&mut self) {}
    fn visit_old_blended_noise(
        &mut self,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) {
        // No inner functions to visit
    }

    fn visit_single_argument_function(&mut self, function: &SingleArgumentFunction) {
        self.visit_density_function_holder(&function.argument)
    }

    fn visit_two_argument_function(&mut self, function: &TwoArgumentFunction) {
        self.visit_density_function_holder(&function.argument1);
        self.visit_density_function_holder(&function.argument2)
    }

    fn visit_interpolated(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }

    fn visit_flat_cache(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }

    fn visit_cache2d(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }

    fn visit_cache_once(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }

    fn visit_cache_all_in_cell(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }

    fn visit_noise(&mut self, noise: &NoiseHolder, xz_scale: f64, y_scale: f64) {
        self.visit_noise_holder(noise)
    }

    fn visit_end_islands(&mut self) {}

    fn visit_weird_scaled_sampler(
        &mut self,
        input: &DensityFunctionHolder,
        noise: &NoiseHolder,
        rarity_value_mapper: &RarityValueMapper,
    ) {
        self.visit_noise_holder(noise);
        self.visit_density_function_holder(input)
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
        self.visit_density_function_holder(shift_x);
        self.visit_density_function_holder(shift_y);
        self.visit_density_function_holder(shift_z);
        self.visit_noise_holder(noise)
    }

    fn visit_range_choice(
        &mut self,
        input: &DensityFunctionHolder,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: &DensityFunctionHolder,
        when_out_of_range: &DensityFunctionHolder,
    ) {
        self.visit_density_function_holder(input);
        self.visit_density_function_holder(when_in_range);
        self.visit_density_function_holder(when_out_of_range)
    }

    fn visit_shift_a(&mut self, function: &NoiseHolder) {
        self.visit_noise_holder(function)
    }
    fn visit_shift_b(&mut self, function: &NoiseHolder) {
        self.visit_noise_holder(function)
    }
    fn visit_shift(&mut self, argument: &NoiseHolder) {
        self.visit_noise_holder(argument)
    }
    fn visit_blend_density(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_clamp(&mut self, input: &DensityFunctionHolder, min: f64, max: f64) {
        self.visit_density_function_holder(input)
    }
    fn visit_abs(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_square(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_cube(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_half_negative(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_quarter_negative(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_invert(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_squeeze(&mut self, function: &SingleArgumentFunction) {
        self.visit_single_argument_function(function)
    }
    fn visit_add(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_mul(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_min(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_max(&mut self, function: &TwoArgumentFunction) {
        self.visit_two_argument_function(function)
    }
    fn visit_spline(&mut self, spline: &SplineHolder) {
        match spline {
            SplineHolder::Constant(v) => self.visit_constant(v.0),
            SplineHolder::Spline(spline) => {
                self.visit_density_function_holder(&spline.coordinate);
                for point in &spline.points {
                    self.visit_constant(point.location.0);
                    self.visit_spline(&point.value);
                    self.visit_constant(point.derivative.0);
                }
            }
        }
    }
    fn visit_y_clamped_gradient(&mut self, from_y: i32, to_y: i32, from_value: f64, to_value: f64) {
        // No inner functions to visit
    }
    fn visit_find_top_surface(
        &mut self,
        density: &DensityFunctionHolder,
        upper_bound: &DensityFunctionHolder,
        lower_bound: i32,
        cell_height: u32,
    ) {
        self.visit_density_function_holder(density);
        self.visit_density_function_holder(upper_bound);
    }
}
