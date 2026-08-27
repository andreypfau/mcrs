use super::*;

#[derive(Clone, PartialEq)]
pub(super) struct BlendedNoise {
    pub(super) xz_scale: f64,
    pub(super) y_scale: f64,
    pub(super) xz_factor: f64,
    pub(super) y_factor: f64,
    pub(super) smear_scale_multiplier: f32,
    pub(super) xz_multiplier: f64,
    pub(super) y_multiplier: f64,
    pub(super) max_value: f32,
    pub(super) limit_smear: f64,
    pub(super) main_smear: f64,
    /// The fBm value factor folded into each main-noise layer, narrowed to f32
    /// per layer. Applying it once after the sum instead rounds differently.
    pub(super) main_amplitudes: [f32; MAIN_OCTAVES],
    /// Trailing divisor applied after the combine. Modern path passes 128.0; Beta path
    /// passes 1.0 (no division) — verified against ChunkProviderGenerate.java:280-297
    /// which has NO /128 vs BlendedNoise.java:159 which does.
    pub(super) final_divisor: f32,
    pub(super) lower_interpolated_noise: OctavePerlinNoise<f32>,
    pub(super) upper_interpolated_noise: OctavePerlinNoise<f32>,
    pub(super) interpolated_noise: OctavePerlinNoise<f32>,
}

impl Debug for BlendedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlendedNoise")
            .field("xz_scale", &self.xz_scale)
            .field("y_scale", &self.y_scale)
            .field("xz_factor", &self.xz_factor)
            .field("y_factor", &self.y_factor)
            .field("smear_scale_multiplier", &self.smear_scale_multiplier)
            .field("xz_multiplier", &self.xz_multiplier)
            .field("y_multiplier", &self.y_multiplier)
            .field("max_value", &self.max_value)
            .field("final_divisor", &self.final_divisor)
            .finish()
    }
}

const LIMIT_OCTAVES: u32 = 16;
const MAIN_OCTAVES: usize = 8;
const MAIN_VALUE_FACTOR: f64 = 12.75;

fn fbm_amplitudes<const N: usize>(value_factor: f64) -> [f32; N] {
    let base = value_factor / ((1u64 << N) - 1) as f64;
    std::array::from_fn(|i| (base * (1u64 << i) as f64) as f32)
}

impl BlendedNoise {
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f32,
        y_scale: f32,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f32,
        final_divisor: f32,
    ) -> Self {
        let xz_multiplier = 684.412 * xz_scale as f64;
        let y_multiplier = 684.412 * y_scale as f64;
        let limit_smear = y_multiplier * smear_scale_multiplier as f64;
        let main_smear = limit_smear / y_factor;
        let lower_interpolated_noise = OctavePerlinNoise::<f32>::new(
            random,
            1 - LIMIT_OCTAVES as i32,
            vec![1.0; LIMIT_OCTAVES as usize],
            true,
        );
        // Every FLAT_SIMPLEX_GRAD entry has two unit components and one zero, and each
        // lattice offset it dots against stays within [-1, 1], so one octave never leaves
        // [-2, 2]; the 2^i octave weights sum to 2^16 - 1 against the /512 combine.
        let max_value = 2.0 * ((1u32 << LIMIT_OCTAVES) - 1) as f32 / 512.0 / final_divisor;
        BlendedNoise {
            xz_scale: xz_scale as f64,
            y_scale: y_scale as f64,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            xz_multiplier,
            y_multiplier,
            max_value,
            limit_smear,
            main_smear,
            main_amplitudes: fbm_amplitudes(MAIN_VALUE_FACTOR),
            final_divisor,
            lower_interpolated_noise,
            upper_interpolated_noise: OctavePerlinNoise::<f32>::new(
                random,
                1 - LIMIT_OCTAVES as i32,
                vec![1.0; LIMIT_OCTAVES as usize],
                true,
            ),
            interpolated_noise: OctavePerlinNoise::<f32>::new(random, -7, vec![1.0; 8], true),
        }
    }
}

impl RangeFunction for BlendedNoise {
    #[inline]
    fn min_value(&self) -> f32 {
        -self.max_value()
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for BlendedNoise {
    fn sample(&self, pos: IVec3) -> f32 {
        let scaled_x = pos.x as f64 * self.xz_multiplier;
        let scaled_y = pos.y as f64 * self.y_multiplier;
        let scaled_z = pos.z as f64 * self.xz_multiplier;

        // Strength-reduce: halve coordinates each iteration instead of
        // multiplying by a separate factor variable.
        let mut fx = scaled_x / self.xz_factor;
        let mut fy = scaled_y / self.y_factor;
        let mut fz = scaled_z / self.xz_factor;
        let mut main_smear = self.main_smear;

        // Interpolated noise: 8 octaves.
        let mut value = 0.0f32;
        for i in 0..MAIN_OCTAVES {
            let s = self.interpolated_noise.sample_octave(
                i,
                OctavePerlinNoise::maintain_precission(fx),
                OctavePerlinNoise::maintain_precission(fy),
                OctavePerlinNoise::maintain_precission(fz),
                main_smear,
                fy,
            );
            value += s * self.main_amplitudes[i];
            fx *= 0.5;
            fy *= 0.5;
            fz *= 0.5;
            main_smear *= 0.5;
        }

        value += 0.5;
        let need_lower = value < 1.0;
        let need_upper = value > 0.0;
        let mut min = 0.0f32;
        let mut max = 0.0f32;

        // Separate loops for lower/upper noise to keep each OctavePerlinNoise's
        // permutation tables cache-warm during evaluation.
        if need_lower {
            let mut sx = scaled_x;
            let mut sy = scaled_y;
            let mut sz = scaled_z;
            let mut sm: f64 = self.limit_smear;
            let mut amplitude = 1.0f32;
            for i in 0..16 {
                let s = self.lower_interpolated_noise.sample_octave(
                    i,
                    OctavePerlinNoise::maintain_precission(sx),
                    OctavePerlinNoise::maintain_precission(sy),
                    OctavePerlinNoise::maintain_precission(sz),
                    sm,
                    sy,
                );
                min += s * amplitude;
                sx *= 0.5;
                sy *= 0.5;
                sz *= 0.5;
                sm *= 0.5;
                amplitude *= 2.0;
            }
        }
        if need_upper {
            let mut sx = scaled_x;
            let mut sy = scaled_y;
            let mut sz = scaled_z;
            let mut sm: f64 = self.limit_smear;
            let mut amplitude = 1.0f32;
            for i in 0..16 {
                let s = self.upper_interpolated_noise.sample_octave(
                    i,
                    OctavePerlinNoise::maintain_precission(sx),
                    OctavePerlinNoise::maintain_precission(sy),
                    OctavePerlinNoise::maintain_precission(sz),
                    sm,
                    sy,
                );
                max += s * amplitude;
                sx *= 0.5;
                sy *= 0.5;
                sz *= 0.5;
                sm *= 0.5;
                amplitude *= 2.0;
            }
        }

        let start = min / 512.0;
        let end = max / 512.0;
        value = if value < 0.0 {
            start
        } else if value > 1.0 {
            end
        } else {
            start + value * (end - start)
        };
        value / self.final_divisor
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Noise {
    pub(super) noise_name: String,
    pub(super) sampler: NoiseSampler,
    pub(super) xz_scale: f64,
    pub(super) y_scale: f64,
}

impl Debug for Noise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Noise")
            .field("noise_name", &self.noise_name)
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
    fn sample(&self, pos: IVec3) -> f32 {
        let xz_scale = self.xz_scale;
        let y_scale = self.y_scale;
        self.sampler.get(
            pos.x as f64 * xz_scale,
            pos.y as f64 * y_scale,
            pos.z as f64 * xz_scale,
        )
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct ShiftB {
    pub(super) noise_name: String,
    pub(super) sampler: NoiseSampler,
}

impl Debug for ShiftB {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftB")
            .field("noise_name", &self.noise_name)
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
    fn sample(&self, pos: IVec3) -> f32 {
        self.sampler
            .get(pos.z as f64 * 0.25, pos.x as f64 * 0.25, 0.0)
            * 4.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Interpolated {
    pub(super) input_index: usize,
    pub(super) input_members: Box<[u32]>,
    pub(super) cell_size_xz: u32,
    pub(super) cell_size_y: u32,
    pub(super) cell_size_xz_inv: f32,
    pub(super) cell_size_y_inv: f32,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ClampedYGradient {
    pub(super) from_y: f32,
    pub(super) to_y: f32,
    pub(super) from_value: f32,
    pub(super) to_value: f32,
}
impl RangeFunction for ClampedYGradient {
    fn min_value(&self) -> f32 {
        self.from_value.min(self.to_value)
    }

    fn max_value(&self) -> f32 {
        self.from_value.max(self.to_value)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Gradient {
    pub(super) axis: Axis,
    pub(super) tiling: TilingMode,
    pub(super) from_coordinate: i32,
    pub(super) to_coordinate: i32,
    pub(super) from_value: f32,
    pub(super) to_value: f32,
}

impl RangeFunction for Gradient {
    fn min_value(&self) -> f32 {
        self.from_value.min(self.to_value)
    }

    fn max_value(&self) -> f32 {
        self.from_value.max(self.to_value)
    }
}

impl DensityFunction for Gradient {
    fn sample(&self, pos: IVec3) -> f32 {
        let coordinate = match self.axis {
            Axis::X => pos.x,
            Axis::Y => pos.y,
            Axis::Z => pos.z,
        };
        let coordinate_range = self.to_coordinate - self.from_coordinate;
        let relative = coordinate - self.from_coordinate;
        let factor = match self.tiling {
            TilingMode::ClampToEdge => relative,
            TilingMode::Repeat => relative.rem_euclid(coordinate_range),
            TilingMode::MirroredRepeat => {
                let tile = relative.div_euclid(coordinate_range);
                let local = relative - tile * coordinate_range;
                if tile & 1 == 0 {
                    local
                } else {
                    coordinate_range - local
                }
            }
        };
        let t = (factor as f32 / coordinate_range as f32).clamp(0.0, 1.0);
        self.from_value + t * (self.to_value - self.from_value)
    }
}

impl DensityFunction for ClampedYGradient {
    fn sample(&self, pos: IVec3) -> f32 {
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
pub(super) struct EndIslands {
    pub(super) noise: Arc<SimplexNoise>,
}

impl EndIslands {
    pub(super) fn new(world_seed: u64) -> Self {
        let mut random = LegacyRandom::new(world_seed);
        for _ in 0..17292 {
            random.next_i32();
        }
        Self {
            noise: Arc::new(SimplexNoise::from_random_at_origin(&mut random)),
        }
    }

    fn height(&self, section_x: i32, section_z: i32) -> f32 {
        let chunk_x = section_x / 2;
        let chunk_z = section_z / 2;
        let sub_x = (section_x % 2) as f32;
        let sub_z = (section_z % 2) as f32;
        let mut height = -100.0f32;
        for offset_x in -12..=12 {
            for offset_z in -12..=12 {
                let cell_x = (chunk_x + offset_x) as i64;
                let cell_z = (chunk_z + offset_z) as i64;
                if cell_x * cell_x + cell_z * cell_z <= 4096 {
                    continue;
                }
                // vanilla narrows the simplex value to f32 before the threshold test
                if self.noise.sample(cell_x as f64, cell_z as f64, 1.0, 1.0) as f32 >= -0.9 {
                    continue;
                }
                let island_size =
                    ((cell_x as f32).abs() * 3439.0 + (cell_z as f32).abs() * 147.0) % 13.0 + 9.0;
                let dx = sub_x - (offset_x * 2) as f32;
                let dz = sub_z - (offset_z * 2) as f32;
                let candidate =
                    (100.0 - (dx * dx + dz * dz).sqrt() * island_size).clamp(-100.0, 80.0);
                height = height.max(candidate);
            }
        }
        height
    }
}

impl RangeFunction for EndIslands {
    fn min_value(&self) -> f32 {
        -0.84375
    }

    fn max_value(&self) -> f32 {
        0.5625
    }
}

impl DensityFunction for EndIslands {
    fn sample(&self, pos: IVec3) -> f32 {
        (self.height(pos.x / 8, pos.z / 8) - 8.0) / 128.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum IndependentDensityFunction {
    Constant(f32),
    OldBlendedNoise(BlendedNoise),
    Noise(Noise),
    ShiftB(ShiftB),
    ClampedYGradient(ClampedYGradient),
    Gradient(Gradient),
    DistanceToPoint(DistanceToPoint),
    EndOuterIslands(EndIslands),
}

impl IndependentDensityFunction {
    /// Fill `out` a column at a time, so each octave hoists its lattice hashes
    /// across the run. Returns false when the caller must sample per position.
    pub(super) fn fill_columns(
        &self,
        volume: &Volume,
        positions: &[IVec3],
        out: &mut [f32],
    ) -> bool {
        let Self::Noise(noise) = self else {
            return false;
        };
        let height = volume.size().y as usize;
        if height < 2 {
            return false;
        }
        let mut ys = vec![0.0f64; height];
        let mut scratch = ColumnScratch::default();
        for (column, slots) in out.chunks_mut(height).enumerate() {
            let run = &positions[column * height..column * height + height];
            for (slot, pos) in ys.iter_mut().zip(run) {
                *slot = pos.y as f64 * noise.y_scale;
            }
            noise.sampler.get_column(
                run[0].x as f64 * noise.xz_scale,
                run[0].z as f64 * noise.xz_scale,
                &ys,
                slots,
                &mut scratch,
            );
        }
        true
    }
}

impl RangeFunction for IndependentDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.min_value(),
            IndependentDensityFunction::Noise(x) => x.min_value(),
            IndependentDensityFunction::ShiftB(x) => x.min_value(),
            IndependentDensityFunction::ClampedYGradient(x) => x.min_value(),
            IndependentDensityFunction::Gradient(x) => x.min_value(),
            IndependentDensityFunction::DistanceToPoint(x) => x.min_value(),
            IndependentDensityFunction::EndOuterIslands(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.max_value(),
            IndependentDensityFunction::Noise(x) => x.max_value(),
            IndependentDensityFunction::ShiftB(x) => x.max_value(),
            IndependentDensityFunction::ClampedYGradient(x) => x.max_value(),
            IndependentDensityFunction::Gradient(x) => x.max_value(),
            IndependentDensityFunction::DistanceToPoint(x) => x.max_value(),
            IndependentDensityFunction::EndOuterIslands(x) => x.max_value(),
        }
    }
}

impl DensityFunction for IndependentDensityFunction {
    fn sample(&self, pos: IVec3) -> f32 {
        match self {
            IndependentDensityFunction::Constant(x) => *x,
            IndependentDensityFunction::OldBlendedNoise(x) => x.sample(pos),
            IndependentDensityFunction::Noise(x) => x.sample(pos),
            IndependentDensityFunction::ShiftB(x) => x.sample(pos),
            IndependentDensityFunction::ClampedYGradient(x) => x.sample(pos),
            IndependentDensityFunction::Gradient(x) => x.sample(pos),
            IndependentDensityFunction::DistanceToPoint(x) => x.sample(pos),
            IndependentDensityFunction::EndOuterIslands(x) => x.sample(pos),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DependentDensityFunction {
    Linear(Linear),
    Affine(Affine),
    PiecewiseAffine(PiecewiseAffine),
    Slide(Slide),
    Unary(Unary),
    Binary(Binary),
    ShiftedNoise(ShiftedNoise),
    Clamp(Clamp),
    RangeChoice(RangeChoice),
    Spline(Spline),
    FindTopSurface(FindTopSurface),
    Lerp(Lerp),
    Slice(Slice),
}

impl RangeFunction for DependentDensityFunction {
    fn min_value(&self) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.min_value(),
            DependentDensityFunction::Affine(x) => x.min_value(),
            DependentDensityFunction::PiecewiseAffine(x) => x.min_value(),
            DependentDensityFunction::Slide(x) => x.min_value(),
            DependentDensityFunction::Unary(x) => x.min_value(),
            DependentDensityFunction::Binary(x) => x.min_value(),
            DependentDensityFunction::ShiftedNoise(x) => x.min_value(),
            DependentDensityFunction::Clamp(x) => x.min_value(),
            DependentDensityFunction::RangeChoice(x) => x.min_value(),
            DependentDensityFunction::Spline(x) => x.min_value(),
            DependentDensityFunction::FindTopSurface(x) => x.min_value(),
            DependentDensityFunction::Lerp(x) => x.min_value(),
            DependentDensityFunction::Slice(x) => x.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DependentDensityFunction::Linear(x) => x.max_value(),
            DependentDensityFunction::Affine(x) => x.max_value(),
            DependentDensityFunction::PiecewiseAffine(x) => x.max_value(),
            DependentDensityFunction::Slide(x) => x.max_value(),
            DependentDensityFunction::Unary(x) => x.max_value(),
            DependentDensityFunction::Binary(x) => x.max_value(),
            DependentDensityFunction::ShiftedNoise(x) => x.max_value(),
            DependentDensityFunction::Clamp(x) => x.max_value(),
            DependentDensityFunction::RangeChoice(x) => x.max_value(),
            DependentDensityFunction::Spline(x) => x.max_value(),
            DependentDensityFunction::FindTopSurface(x) => x.max_value(),
            DependentDensityFunction::Lerp(x) => x.max_value(),
            DependentDensityFunction::Slice(x) => x.max_value(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Linear {
    pub(super) input_index: usize,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
    pub(super) argument: f32,
    pub(super) operation: LinearOperation,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Affine {
    pub(super) input_index: usize,
    pub(super) scale: f32,
    pub(super) offset: f32,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
pub(super) enum LinearOperation {
    Add,
    Multiply,
}

impl Affine {
    pub(super) fn compute_range(
        input_min: f32,
        input_max: f32,
        scale: f32,
        offset: f32,
    ) -> (f32, f32) {
        if scale >= 0.0 {
            (
                input_min.mul_add(scale, offset),
                input_max.mul_add(scale, offset),
            )
        } else {
            (
                input_max.mul_add(scale, offset),
                input_min.mul_add(scale, offset),
            )
        }
    }
}

impl RangeFunction for Affine {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

/// Piecewise-linear affine: different scales for negative vs non-negative input.
///
/// Replaces patterns like `Affine(Unary::QuarterNegative(x))` or
/// `Affine(Unary::HalfNegative(x))` where the unary damps the negative side.
///
/// Computes: `if x < 0 { x * neg_scale + offset } else { x * pos_scale + offset }`
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PiecewiseAffine {
    pub(super) input_index: usize,
    pub(super) neg_scale: f32,
    pub(super) pos_scale: f32,
    pub(super) offset: f32,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
}

impl PiecewiseAffine {
    pub(super) fn compute_range(
        input_min: f32,
        input_max: f32,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
    ) -> (f32, f32) {
        // Two monotone pieces meeting at zero: the extremes can only sit at an
        // endpoint or at the breakpoint, and the breakpoint only counts when the
        // input interval actually straddles it.
        let apply = |x: f32| {
            if x < 0.0 {
                x * neg_scale + offset
            } else {
                x * pos_scale + offset
            }
        };
        let a = apply(input_min);
        let b = apply(input_max);
        let mut lo = a.min(b);
        let mut hi = a.max(b);
        if input_min < 0.0 && input_max >= 0.0 {
            lo = lo.min(offset);
            hi = hi.max(offset);
        }
        (lo, hi)
    }
}

impl RangeFunction for PiecewiseAffine {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

/// Fused world-boundary "slide" operation.
///
/// Replaces the 5-node chain:
///   `Affine(+a) → Mul(y_grad1) → Affine(+b) → Mul(y_grad2) → Affine(+c)`
///
/// Full computation:
///   `grad2(y) * (grad1(y) * (input + offset_a) + offset_b) + offset_c`
///
/// Fast path: when both gradients saturate to 1.0 (y in the interior range),
/// the three offsets cancel out and the result equals `input + combined_offset`
/// (which is typically ~0, i.e. identity).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Slide {
    pub(super) input_index: usize,

    // First Y-gradient applied (typically "top": 240..256 → 1.0..0.0)
    pub(super) grad1: ClampedYGradient,
    // Second Y-gradient applied (typically "bottom": -64..-40 → 0.0..1.0)
    pub(super) grad2: ClampedYGradient,

    // Three affine offsets (all original affines had scale=1.0)
    pub(super) offset_a: f32, // pre-grad1
    pub(super) offset_b: f32, // between grad1 and grad2
    pub(super) offset_c: f32, // post-grad2

    // Pre-computed: offset_a + offset_b + offset_c
    pub(super) combined_offset: f32,

    // Y range where both gradients saturate to 1.0 (fast path)
    pub(super) fast_path_min_y: f32,
    pub(super) fast_path_max_y: f32,

    pub(super) min_value: f32,
    pub(super) max_value: f32,
}

impl Slide {
    #[inline]
    fn eval_gradient(g: &ClampedYGradient, y: f32) -> f32 {
        if y < g.from_y {
            g.from_value
        } else if y > g.to_y {
            g.to_value
        } else {
            g.from_value + (g.to_value - g.from_value) * (y - g.from_y) / (g.to_y - g.from_y)
        }
    }

    #[inline]
    fn compute(&self, input: f32, y: f32) -> f32 {
        if y > self.fast_path_min_y && y < self.fast_path_max_y {
            input + self.combined_offset
        } else {
            let g1 = Self::eval_gradient(&self.grad1, y);
            let g2 = Self::eval_gradient(&self.grad2, y);
            (g1 * (input + self.offset_a) + self.offset_b).mul_add(g2, self.offset_c)
        }
    }

    /// Compute the Y range where a gradient saturates to exactly 1.0.
    /// Returns (min_y, max_y) or None if the gradient never equals 1.0.
    pub(super) fn saturate_one_range(g: &ClampedYGradient) -> Option<(f32, f32)> {
        let below = g.from_value == 1.0; // y <= from_y → 1.0
        let above = g.to_value == 1.0; // y >= to_y → 1.0
        match (below, above) {
            (true, true) => Some((f32::NEG_INFINITY, f32::INFINITY)),
            (true, false) => Some((f32::NEG_INFINITY, g.from_y)),
            (false, true) => Some((g.to_y, f32::INFINITY)),
            (false, false) => None,
        }
    }
}

impl RangeFunction for Slide {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
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
pub(super) struct Unary {
    pub(super) input_index: usize,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
    pub(super) operation: UnaryOperation,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
pub(super) enum UnaryOperation {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Reciprocal,
    Squeeze,
    Sqrt,
    Log,
    Sign,
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
            UnaryOperation::Reciprocal => 1.0 / value,
            UnaryOperation::Squeeze => {
                let clamped = value.clamp(-1.0, 1.0);
                clamped / 2.0 - clamped.powi(3) / 24.0
            }
            UnaryOperation::Sqrt => value.sqrt(),
            UnaryOperation::Log => (value as f64).ln() as f32,
            // Unlike f32::signum, zero and NaN come back unchanged.
            UnaryOperation::Sign => {
                if value == 0.0 || value.is_nan() {
                    value
                } else if value > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
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
pub(super) struct ShiftedNoise {
    pub(super) noise_name: String,
    pub(super) input_x_index: usize,
    pub(super) input_y_index: usize,
    pub(super) input_z_index: usize,
    pub(super) xz_scale: f64,
    pub(super) y_scale: f64,
    pub(super) sampler: NoiseSampler,
}

impl Debug for ShiftedNoise {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftedNoise")
            .field("noise_name", &self.noise_name)
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Clamp {
    pub(super) input_index: usize,
    /// The datapack bounds already narrowed to the input's own range. Sampling clamps
    /// to these rather than the raw bounds: for any value the input can produce the two
    /// agree, so the narrower pair serves as both the operation and the declared range.
    pub(super) min_value: f32,
    pub(super) max_value: f32,
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RangeChoice {
    pub(super) input_index: usize,
    pub(super) when_in_index: usize,
    pub(super) when_out_index: usize,
    pub(super) min_inclusion_value: f32,
    pub(super) max_exclusion_value: f32,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
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

#[derive(Clone, Debug, PartialEq)]
pub(super) enum SplineValue {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Segment {
    pub(super) left: f32,
    pub(super) dist: f32,             // x[i+1] - x[i]
    pub(super) lower_deriv_dist: f32, // d[i]   * dist
    pub(super) upper_deriv_dist: f32, // d[i+1] * dist
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Spline {
    pub(super) input_index: usize,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
    pub(super) locations: Box<[f32]>,
    pub(super) derivatives: Box<[f32]>,
    pub(super) values: Box<[SplineValue]>,
    pub(super) segments: Box<[Segment]>, // len = locations.len() - 1
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

                let min_delta_right = -max_value_delta_right + min_right - max_left;
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
                dist,
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
        a + t * (b - a)
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

impl SplineValue {
    #[inline]
    fn sample(&self, cache: &[f32]) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample(cache),
            SplineValue::Constant(x) => *x,
        }
    }
}

impl Spline {
    fn sample(&self, cache: &[f32]) -> f32 {
        let location = cache[self.input_index];

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample(cache);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                v0 + d0 * (location - locs[0])
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample(cache);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                v + d * (location - locs[i])
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample(cache);
        let v1 = self.values[i1].sample(cache);

        let seg = self.segments[i0];
        let x = (location - seg.left) / seg.dist;

        let delta = v1 - v0;

        let e0 = seg.lower_deriv_dist - delta;
        let e1 = -seg.upper_deriv_dist + delta;

        let cubic = (x * (1.0 - x)) * Self::lerp(e0, e1, x);
        let linear = Self::lerp(v0, v1, x);

        cubic + linear
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FindTopSurface {
    pub(super) density_index: usize,
    pub(super) density_members: Box<[u32]>,
    pub(super) upper_bound_index: usize,
    pub(super) lower_bound: f32,
    pub(super) cell_height: f32,
    pub(super) max_value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Lerp {
    pub(super) alpha_index: usize,
    pub(super) first_index: usize,
    pub(super) second_index: usize,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
}

impl RangeFunction for Lerp {
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
pub(super) struct Slice {
    pub(super) axis: Axis,
    pub(super) coordinate: i32,
    pub(super) input_index: usize,
    pub(super) input_members: Box<[u32]>,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
}

impl RangeFunction for Slice {
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
pub(super) struct DistanceToPoint {
    pub(super) point: IVec3,
    pub(super) metric: DistanceMetric,
}

impl RangeFunction for DistanceToPoint {
    #[inline]
    fn min_value(&self) -> f32 {
        0.0
    }

    #[inline]
    fn max_value(&self) -> f32 {
        f32::INFINITY
    }
}

impl DensityFunction for DistanceToPoint {
    fn sample(&self, pos: IVec3) -> f32 {
        let d = (self.point - pos).as_vec3();
        match self.metric {
            DistanceMetric::Euclidean => d.length(),
            DistanceMetric::EuclideanSquared => d.length_squared(),
            DistanceMetric::Manhattan => d.x.abs() + d.y.abs() + d.z.abs(),
            DistanceMetric::Chebyshev => d.x.abs().max(d.y.abs()).max(d.z.abs()),
        }
    }
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Binary {
    pub(super) input1_index: usize,
    pub(super) input2_index: usize,
    pub(super) min_value: f32,
    pub(super) max_value: f32,
    pub(super) operation: BinaryOperation,
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
pub(super) enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Min,
    Max,
    Pow,
    Round(RoundingMode),
}

impl BinaryOperation {
    #[inline]
    pub(super) fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            BinaryOperation::Add => a + b,
            BinaryOperation::Subtract => a - b,
            BinaryOperation::Multiply => a * b,
            BinaryOperation::Divide => {
                if a == 0.0 {
                    0.0
                } else {
                    a / b
                }
            }
            BinaryOperation::Min => a.min(b),
            BinaryOperation::Max => a.max(b),
            BinaryOperation::Pow => pow_narrowed(a, b),
            BinaryOperation::Round(mode) => {
                if b == 0.0 {
                    a
                } else {
                    round_to_integer(a / b, mode) * b
                }
            }
        }
    }
}

#[inline]
pub(super) fn round_to_integer(value: f32, mode: RoundingMode) -> f32 {
    match mode {
        RoundingMode::Floor => value.floor(),
        // Java rounds halves up, not away from zero: round(-2.5) is -2.
        RoundingMode::Round => (value + 0.5).floor(),
        RoundingMode::Ceil => value.ceil(),
        RoundingMode::Truncate => {
            if value > 0.0 {
                value.floor()
            } else {
                value.ceil()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DensityFunctionComponent {
    Independent(IndependentDensityFunction),
    Dependent(DependentDensityFunction),
    Interpolated(Interpolated),
}

impl DensityFunctionComponent {
    pub(super) fn as_constant(&self) -> Option<f32> {
        match self {
            DensityFunctionComponent::Independent(x) => match x {
                IndependentDensityFunction::Constant(v) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }
}

impl SplineValue {
    pub(super) fn rewrite_indices(&mut self, redirect: &[usize]) {
        if let SplineValue::Spline(spline) = self {
            spline.rewrite_indices(redirect);
        }
    }
}

impl Spline {
    pub(super) fn rewrite_indices(&mut self, redirect: &[usize]) {
        self.input_index = redirect[self.input_index];
        for value in self.values.iter_mut() {
            value.rewrite_indices(redirect);
        }
    }

    pub(super) fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        f(self.input_index);
        for value in self.values.iter() {
            if let SplineValue::Spline(nested) = value {
                nested.visit_input_indices(f);
            }
        }
    }
}

impl DensityFunctionComponent {
    pub(super) fn rewrite_indices(&mut self, redirect: &[usize]) {
        match self {
            DensityFunctionComponent::Independent(_) => {}
            DensityFunctionComponent::Dependent(dep) => match dep {
                DependentDensityFunction::Linear(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Affine(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Slide(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Unary(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::Binary(x) => {
                    x.input1_index = redirect[x.input1_index];
                    x.input2_index = redirect[x.input2_index];
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    x.input_x_index = redirect[x.input_x_index];
                    x.input_y_index = redirect[x.input_y_index];
                    x.input_z_index = redirect[x.input_z_index];
                }
                DependentDensityFunction::Clamp(x) => {
                    x.input_index = redirect[x.input_index];
                }
                DependentDensityFunction::RangeChoice(x) => {
                    x.input_index = redirect[x.input_index];
                    x.when_in_index = redirect[x.when_in_index];
                    x.when_out_index = redirect[x.when_out_index];
                }
                DependentDensityFunction::Spline(x) => {
                    x.rewrite_indices(redirect);
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    x.density_index = redirect[x.density_index];
                    x.upper_bound_index = redirect[x.upper_bound_index];
                }
                DependentDensityFunction::Lerp(x) => {
                    x.alpha_index = redirect[x.alpha_index];
                    x.first_index = redirect[x.first_index];
                    x.second_index = redirect[x.second_index];
                }
                DependentDensityFunction::Slice(x) => {
                    x.input_index = redirect[x.input_index];
                }
            },
            DensityFunctionComponent::Interpolated(x) => {
                x.input_index = redirect[x.input_index];
            }
        }
    }

    pub(super) fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        match self {
            DensityFunctionComponent::Independent(_) => {}
            DensityFunctionComponent::Dependent(dep) => match dep {
                DependentDensityFunction::Linear(x) => f(x.input_index),
                DependentDensityFunction::Affine(x) => f(x.input_index),
                DependentDensityFunction::PiecewiseAffine(x) => f(x.input_index),
                DependentDensityFunction::Slide(x) => f(x.input_index),
                DependentDensityFunction::Unary(x) => f(x.input_index),
                DependentDensityFunction::Binary(x) => {
                    f(x.input1_index);
                    f(x.input2_index);
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    f(x.input_x_index);
                    f(x.input_y_index);
                    f(x.input_z_index);
                }
                DependentDensityFunction::Clamp(x) => f(x.input_index),
                DependentDensityFunction::RangeChoice(x) => {
                    f(x.input_index);
                    f(x.when_in_index);
                    f(x.when_out_index);
                }
                DependentDensityFunction::Spline(x) => x.visit_input_indices(f),
                DependentDensityFunction::FindTopSurface(x) => {
                    f(x.density_index);
                    f(x.upper_bound_index);
                }
                DependentDensityFunction::Lerp(x) => {
                    f(x.alpha_index);
                    f(x.first_index);
                    f(x.second_index);
                }
                DependentDensityFunction::Slice(x) => f(x.input_index),
            },
            DensityFunctionComponent::Interpolated(x) => f(x.input_index),
        }
    }
}

impl RangeFunction for DensityFunctionComponent {
    fn min_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.min_value(),
            DensityFunctionComponent::Dependent(func) => func.min_value(),
            DensityFunctionComponent::Interpolated(func) => func.min_value(),
        }
    }

    fn max_value(&self) -> f32 {
        match self {
            DensityFunctionComponent::Independent(func) => func.max_value(),
            DensityFunctionComponent::Dependent(func) => func.max_value(),
            DensityFunctionComponent::Interpolated(func) => func.max_value(),
        }
    }
}

#[inline]
pub fn lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}

/// The compiled node arena: everything a fill needs that is not the volume.
#[derive(Clone, Copy)]
pub(super) struct Arena<'a> {
    stack: &'a [DensityFunctionComponent],
}

#[derive(Default)]
pub(super) struct MemberScratch {
    rows: Vec<f32>,
    point: Vec<f32>,
    positions: Vec<IVec3>,
}

impl<'a> Arena<'a> {
    pub(super) fn new(stack: &'a [DensityFunctionComponent]) -> Self {
        Self { stack }
    }

    /// Evaluate the subgraph `members` — topologically ordered, its own root last —
    /// over every position of `volume`.
    pub(super) fn fill_members(self, members: &[u32], volume: &Volume, out: &mut [f32]) {
        self.fill_members_with(members, volume, out, &mut MemberScratch::default());
    }

    /// [`Arena::fill_members`] against caller-owned buffers, for callers that
    /// evaluate the same subgraph many times over.
    pub(super) fn fill_members_with(
        self,
        members: &[u32],
        volume: &Volume,
        out: &mut [f32],
        scratch: &mut MemberScratch,
    ) {
        let n = volume.len();
        let root = *members.last().expect("a subgraph has at least one member") as usize;
        scratch.rows.clear();
        scratch.rows.resize((root + 1) * n, 0.0);
        scratch.point.clear();
        scratch.point.resize(self.stack.len(), 0.0);
        volume.positions_into(&mut scratch.positions);
        for &member in members {
            self.fill_node(
                member as usize,
                volume,
                &scratch.positions,
                &mut scratch.rows,
                &mut scratch.point,
            );
        }
        out.copy_from_slice(&scratch.rows[root * n..root * n + n]);
    }

    pub(super) fn fill_node(
        self,
        i: usize,
        volume: &Volume,
        positions: &[IVec3],
        rows: &mut [f32],
        point: &mut [f32],
    ) {
        let stack = self.stack;
        let n = volume.len();
        let out_base = i * n;
        match &stack[i] {
            DensityFunctionComponent::Independent(f) => {
                let out = &mut rows[out_base..out_base + n];
                if !f.fill_columns(volume, positions, out) {
                    for (p, slot) in out.iter_mut().enumerate() {
                        *slot = f.sample(positions[p]);
                    }
                }
            }
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::Linear(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        let input = rows[a + p];
                        rows[out_base + p] = match x.operation {
                            LinearOperation::Add => input + x.argument,
                            LinearOperation::Multiply => input * x.argument,
                        };
                    }
                }
                DependentDensityFunction::Affine(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = rows[a + p].mul_add(x.scale, x.offset);
                    }
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        let input = rows[a + p];
                        let scale = if input < 0.0 {
                            x.neg_scale
                        } else {
                            x.pos_scale
                        };
                        rows[out_base + p] = input.mul_add(scale, x.offset);
                    }
                }
                DependentDensityFunction::Slide(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = x.compute(rows[a + p], positions[p].y as f32);
                    }
                }
                DependentDensityFunction::Unary(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = x.operation.apply(rows[a + p]);
                    }
                }
                DependentDensityFunction::Binary(x) => {
                    let a = x.input1_index * n;
                    let b = x.input2_index * n;
                    for p in 0..n {
                        rows[out_base + p] = x.operation.apply(rows[a + p], rows[b + p]);
                    }
                }
                DependentDensityFunction::ShiftedNoise(x) => {
                    let (sx, sy, sz) = (
                        x.input_x_index * n,
                        x.input_y_index * n,
                        x.input_z_index * n,
                    );
                    for p in 0..n {
                        let pos = positions[p];
                        rows[out_base + p] = x.sampler.get(
                            pos.x as f64 * x.xz_scale + rows[sx + p] as f64,
                            pos.y as f64 * x.y_scale + rows[sy + p] as f64,
                            pos.z as f64 * x.xz_scale + rows[sz + p] as f64,
                        );
                    }
                }
                DependentDensityFunction::Clamp(x) => {
                    let a = x.input_index * n;
                    for p in 0..n {
                        rows[out_base + p] = rows[a + p].clamp(x.min_value, x.max_value);
                    }
                }
                DependentDensityFunction::RangeChoice(x) => {
                    let a = x.input_index * n;
                    let win = x.when_in_index * n;
                    let wout = x.when_out_index * n;
                    for p in 0..n {
                        let input = rows[a + p];
                        rows[out_base + p] =
                            if input >= x.min_inclusion_value && input < x.max_exclusion_value {
                                rows[win + p]
                            } else {
                                rows[wout + p]
                            };
                    }
                }
                DependentDensityFunction::Lerp(x) => {
                    let al = x.alpha_index * n;
                    let fi = x.first_index * n;
                    let se = x.second_index * n;
                    for p in 0..n {
                        let alpha = rows[al + p];
                        rows[out_base + p] = if alpha == 0.0 {
                            rows[fi + p]
                        } else if alpha == 1.0 {
                            rows[se + p]
                        } else {
                            let first = rows[fi + p];
                            first + alpha * (rows[se + p] - first)
                        };
                    }
                }
                DependentDensityFunction::Spline(x) => {
                    for p in 0..n {
                        for j in 0..i {
                            point[j] = rows[j * n + p];
                        }
                        rows[out_base + p] = x.sample(point);
                    }
                }
                DependentDensityFunction::Slice(x) => {
                    let (_, out) = rows.split_at_mut(out_base);
                    x.fill(self, volume, &mut out[..n]);
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    let a = x.upper_bound_index * n;
                    let (filled, out) = rows.split_at_mut(out_base);
                    x.fill(self, positions, &filled[a..a + n], &mut out[..n]);
                }
            },
            DensityFunctionComponent::Interpolated(x) => {
                let (filled, out) = rows.split_at_mut(out_base);
                let input = x.input_index * n;
                x.sample_volume(self, volume, &filled[input..input + n], &mut out[..n]);
            }
        }
    }
}

impl Slice {
    fn fill(&self, arena: Arena<'_>, volume: &Volume, out: &mut [f32]) {
        let pinned = self.pinned_volume(volume);
        if &pinned == volume {
            arena.fill_members(&self.input_members, volume, out);
            return;
        }
        let mut sliced = vec![0.0f32; pinned.len()];
        arena.fill_members(&self.input_members, &pinned, &mut sliced);
        for z in 0..volume.size().z {
            for x in 0..volume.size().x {
                for y in 0..volume.size().y {
                    let source = match self.axis {
                        Axis::X => pinned.index_unchecked(0, y, z),
                        Axis::Y => pinned.index_unchecked(x, 0, z),
                        Axis::Z => pinned.index_unchecked(x, y, 0),
                    };
                    out[volume.index_unchecked(x, y, z)] = sliced[source];
                }
            }
        }
    }

    /// `volume` with this node's axis collapsed onto its pinned coordinate.
    fn pinned_volume(&self, volume: &Volume) -> Volume {
        let mut size = volume.size();
        let mut min = volume.min_block();
        let axis = match self.axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        };
        size[axis] = 1;
        min[axis] = self.coordinate;
        Volume::new(size, min, volume.step_block())
    }
}

impl FindTopSurface {
    /// The highest cell boundary at or below each upper bound whose density is
    /// positive, probing downwards one cell at a time.
    fn fill(&self, arena: Arena<'_>, positions: &[IVec3], upper_bounds: &[f32], out: &mut [f32]) {
        let mut probed = [0.0f32];
        let mut scratch = MemberScratch::default();
        for (p, slot) in out.iter_mut().enumerate() {
            let top_y = (upper_bounds[p] / self.cell_height).floor() * self.cell_height;
            *slot = if top_y <= self.lower_bound {
                self.lower_bound
            } else {
                let mut current_y = top_y;
                loop {
                    let probe =
                        Volume::point(IVec3::new(positions[p].x, current_y as i32, positions[p].z));
                    arena.fill_members_with(
                        &self.density_members,
                        &probe,
                        &mut probed,
                        &mut scratch,
                    );
                    if probed[0] > 0.0 || current_y <= self.lower_bound {
                        break current_y;
                    }
                    current_y -= self.cell_height;
                }
            };
        }
    }
}

impl Interpolated {
    /// Whether `volume` already samples this node's cell lattice, so the input
    /// can be passed straight through with no interpolation.
    pub(super) fn is_lattice_volume(&self, volume: &Volume) -> bool {
        let xz = self.cell_size_xz as i32;
        let y = self.cell_size_y as i32;
        (volume.step_block().x == xz || volume.size().x == 1)
            && (volume.step_block().y == y || volume.size().y == 1)
            && (volume.step_block().z == xz || volume.size().z == 1)
            && volume.min_block().x.rem_euclid(xz) == 0
            && volume.min_block().y.rem_euclid(y) == 0
            && volume.min_block().z.rem_euclid(xz) == 0
    }

    /// `input_row` must hold the input's values over `volume` whenever
    /// [`Interpolated::is_lattice_volume`] holds; off the lattice this node
    /// resamples its input over its own cell volume and never reads it.
    pub(super) fn sample_volume(
        &self,
        arena: Arena<'_>,
        volume: &Volume,
        input_row: &[f32],
        out: &mut [f32],
    ) {
        if self.is_lattice_volume(volume) {
            out.copy_from_slice(input_row);
        } else if volume.len() == 1 {
            // A single position combines the eight corners exactly, where a
            // volume accumulates along Y. Vanilla splits the same two ways, and
            // the block values a chunk fill produces come from the second.
            out[0] = self.sample_point(arena, volume.min_block());
        } else if volume.step_block() == IVec3::ONE {
            self.fill_block_step(arena, volume, out);
        } else {
            let block_volume =
                Volume::dense(volume.size() * volume.step_block(), volume.min_block());
            let mut block = vec![0.0f32; block_volume.len()];
            self.fill_block_step(arena, &block_volume, &mut block);
            for z in 0..volume.size().z {
                for x in 0..volume.size().x {
                    for y in 0..volume.size().y {
                        out[volume.index_unchecked(x, y, z)] = block[block_volume.index_unchecked(
                            x * volume.step_block().x,
                            y * volume.step_block().y,
                            z * volume.step_block().z,
                        )];
                    }
                }
            }
        }
    }

    fn sample_point(&self, arena: Arena<'_>, pos: IVec3) -> f32 {
        let size_xz = self.cell_size_xz as i32;
        let size_y = self.cell_size_y as i32;
        let x_in_cell = pos.x.rem_euclid(size_xz);
        let y_in_cell = pos.y.rem_euclid(size_y);
        let z_in_cell = pos.z.rem_euclid(size_xz);
        let cell = Volume::new(
            IVec3::splat(2),
            IVec3::new(pos.x - x_in_cell, pos.y - y_in_cell, pos.z - z_in_cell),
            IVec3::new(size_xz, size_y, size_xz),
        );
        let mut corners = [0.0f32; 8];
        arena.fill_members(&self.input_members, &cell, &mut corners);

        let alpha_x = x_in_cell as f32 / size_xz as f32;
        let alpha_y = y_in_cell as f32 / size_y as f32;
        let alpha_z = z_in_cell as f32 / size_xz as f32;
        let at = |x: i32, y: i32, z: i32| corners[cell.index_unchecked(x, y, z)];
        let along_x = |y: i32, z: i32| lerp(alpha_x, at(0, y, z), at(1, y, z));
        let along_xy = |z: i32| lerp(alpha_y, along_x(0, z), along_x(1, z));
        lerp(alpha_z, along_xy(0), along_xy(1))
    }

    fn fill_block_step(&self, arena: Arena<'_>, volume: &Volume, out: &mut [f32]) {
        let xz = self.cell_size_xz as i32;
        let sy = self.cell_size_y as i32;
        let min_cell_x = volume.min_block().x.div_euclid(xz);
        let min_cell_y = volume.min_block().y.div_euclid(sy);
        let min_cell_z = volume.min_block().z.div_euclid(xz);
        let cell_count_x = volume.max_block().x.div_euclid(xz) - min_cell_x + 1;
        let cell_count_y = volume.max_block().y.div_euclid(sy) - min_cell_y + 1;
        let cell_count_z = volume.max_block().z.div_euclid(xz) - min_cell_z + 1;
        let cell_volume = Volume::new(
            IVec3::new(
                cell_count_x + i32::from(volume.max_block().x.rem_euclid(xz) != 0),
                cell_count_y + i32::from(volume.max_block().y.rem_euclid(sy) != 0),
                cell_count_z + i32::from(volume.max_block().z.rem_euclid(xz) != 0),
            ),
            IVec3::new(min_cell_x * xz, min_cell_y * sy, min_cell_z * xz),
            IVec3::new(xz, sy, xz),
        );

        let mut cell = vec![0.0f32; cell_volume.len()];
        arena.fill_members(&self.input_members, &cell_volume, &mut cell);

        for cell_z in 0..cell_count_z {
            let next_cell_z = (cell_z + 1).min(cell_volume.size().z - 1);
            for cell_x in 0..cell_count_x {
                let next_cell_x = (cell_x + 1).min(cell_volume.size().x - 1);
                let mut v000 = cell[cell_volume.index_unchecked(cell_x, 0, cell_z)];
                let mut v100 = cell[cell_volume.index_unchecked(next_cell_x, 0, cell_z)];
                let mut v001 = cell[cell_volume.index_unchecked(cell_x, 0, next_cell_z)];
                let mut v101 = cell[cell_volume.index_unchecked(next_cell_x, 0, next_cell_z)];
                for cell_y in 0..cell_count_y {
                    let next_cell_y = (cell_y + 1).min(cell_volume.size().y - 1);
                    let v010 = cell[cell_volume.index_unchecked(cell_x, next_cell_y, cell_z)];
                    let v110 = cell[cell_volume.index_unchecked(next_cell_x, next_cell_y, cell_z)];
                    let v011 = cell[cell_volume.index_unchecked(cell_x, next_cell_y, next_cell_z)];
                    let v111 =
                        cell[cell_volume.index_unchecked(next_cell_x, next_cell_y, next_cell_z)];
                    self.fill_cell(
                        out,
                        volume,
                        &cell_volume,
                        IVec3::new(cell_x, cell_y, cell_z),
                        [v000, v100, v010, v110, v001, v101, v011, v111],
                    );
                    v000 = v010;
                    v100 = v110;
                    v001 = v011;
                    v101 = v111;
                }
            }
        }
    }

    fn fill_cell(
        &self,
        out: &mut [f32],
        output_volume: &Volume,
        cell_volume: &Volume,
        cell: IVec3,
        [v000, v100, v010, v110, v001, v101, v011, v111]: [f32; 8],
    ) {
        let cell_output_x = cell_volume.block_x(cell.x) - output_volume.min_block().x;
        let cell_output_y = cell_volume.block_y(cell.y) - output_volume.min_block().y;
        let cell_output_z = cell_volume.block_z(cell.z) - output_volume.min_block().z;
        let x0 = 0.max(-cell_output_x);
        let y0 = 0.max(-cell_output_y);
        let z0 = 0.max(-cell_output_z);
        let x1 = (self.cell_size_xz as i32).min(output_volume.size().x - cell_output_x) - 1;
        let y1 = (self.cell_size_y as i32).min(output_volume.size().y - cell_output_y) - 1;
        let z1 = (self.cell_size_xz as i32).min(output_volume.size().z - cell_output_z) - 1;

        for z in z0..=z1 {
            let output_z = cell_output_z + z;
            let alpha_z = z as f32 * self.cell_size_xz_inv;
            let v00_ = lerp(alpha_z, v000, v001);
            let v01_ = lerp(alpha_z, v010, v011);
            let v10_ = lerp(alpha_z, v100, v101);
            let v11_ = lerp(alpha_z, v110, v111);

            for x in x0..=x1 {
                let output_x = cell_output_x + x;
                let alpha_x = x as f32 * self.cell_size_xz_inv;
                let v_0_ = lerp(alpha_x, v00_, v10_);
                let v_1_ = lerp(alpha_x, v01_, v11_);
                let value_step = (v_1_ - v_0_) * self.cell_size_y_inv;
                let mut value = v_0_ + value_step * y0 as f32;
                let start = output_volume.index_unchecked(output_x, cell_output_y + y0, output_z);

                for slot in &mut out[start..start + (y1 - y0 + 1).max(0) as usize] {
                    *slot = value;
                    value += value_step;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dense_layout_is_contiguous_from_min_block() {
        let v = Volume::dense(IVec3::new(3, 5, 2), IVec3::new(-7, 12, 40));
        assert_eq!(v.len(), 30);
        assert_eq!(v.max_block(), IVec3::new(-5, 16, 41));
        let mut seen = vec![false; v.len()];
        for z in 0..v.size().z {
            for x in 0..v.size().x {
                for y in 0..v.size().y {
                    assert_eq!(
                        IVec3::new(v.block_x(x), v.block_y(y), v.block_z(z)),
                        v.min_block() + IVec3::new(x, y, z)
                    );
                    seen[v.index_unchecked(x, y, z)] = true;
                }
            }
        }
        assert!(seen.into_iter().all(|hit| hit));
    }

    #[test]
    fn y_is_the_fastest_axis() {
        let v = Volume::dense(IVec3::new(4, 8, 4), IVec3::ZERO);
        assert_eq!(v.index_unchecked(0, 1, 0), 1);
        assert_eq!(v.index_unchecked(1, 0, 0), 8);
        assert_eq!(v.index_unchecked(0, 0, 1), 32);
    }

    #[test]
    fn a_strided_lattice_index_steps_by_the_cell_size() {
        let v = Volume::new(
            IVec3::new(2, 2, 2),
            IVec3::new(-8, -64, 4),
            IVec3::new(4, 8, 4),
        );
        assert_eq!(v.max_block(), IVec3::new(-1, -49, 11));
        assert_eq!(
            IVec3::new(v.block_x(1), v.block_y(1), v.block_z(1)),
            IVec3::new(-4, -56, 8)
        );
        assert_eq!(v.index_unchecked(0, 1, 0), 1);
        assert_eq!(v.index_unchecked(1, 0, 0), 2);
        assert_eq!(v.index_unchecked(0, 0, 1), 4);
    }

    #[test]
    fn point_is_a_single_dense_cell() {
        let v = Volume::point(IVec3::new(5, -3, 9));
        assert_eq!(v.len(), 1);
        assert_eq!(v.size(), IVec3::ONE);
        assert_eq!(v.step_block(), IVec3::ONE);
        assert_eq!(v.min_block(), IVec3::new(5, -3, 9));
        assert_eq!(v.max_block(), IVec3::new(5, -3, 9));
    }

    #[test]
    #[should_panic]
    fn a_zero_size_volume_cannot_exist() {
        Volume::dense(IVec3::new(1, 0, 1), IVec3::ZERO);
    }

    #[test]
    #[should_panic]
    fn a_zero_step_volume_cannot_exist() {
        Volume::new(IVec3::ONE, IVec3::ZERO, IVec3::new(1, 0, 1));
    }
}
