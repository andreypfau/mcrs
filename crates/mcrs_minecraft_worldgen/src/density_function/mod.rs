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
use tracing::{info, info_span};

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

/// Check if a Binary::Multiply has one ClampedYGradient input.
/// Returns (gradient, other_input_index) if found.
fn extract_mul_y_grad(
    bin: &Binary,
    stack: &[DensityFunctionComponent],
) -> Option<(ClampedYGradient, usize)> {
    if bin.operation != BinaryOperation::Multiply {
        return None;
    }
    if let DensityFunctionComponent::Independent(IndependentDensityFunction::ClampedYGradient(g)) =
        &stack[bin.input1_index]
    {
        return Some((g.clone(), bin.input2_index));
    }
    if let DensityFunctionComponent::Independent(IndependentDensityFunction::ClampedYGradient(g)) =
        &stack[bin.input2_index]
    {
        return Some((g.clone(), bin.input1_index));
    }
    None
}

/// Try to detect and build a Slide from a 5-node pattern:
///   Affine(+c) at `idx` ← Mul(ygrad2, Affine(+b) ← Mul(ygrad1, Affine(+a, input)))
fn try_build_slide(idx: usize, stack: &[DensityFunctionComponent]) -> Option<Slide> {
    // Node at idx must be Affine with scale=1.0 (the outermost "add offset_c")
    let aff_c = match &stack[idx] {
        DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(a))
            if a.scale == 1.0 =>
        {
            a
        }
        _ => return None,
    };
    let outer_min = aff_c.min_value;
    let outer_max = aff_c.max_value;

    // Its input must be Binary::Multiply with a ClampedYGradient
    let mul2 = match &stack[aff_c.input_index] {
        DensityFunctionComponent::Dependent(DependentDensityFunction::Binary(b)) => b,
        _ => return None,
    };
    let (grad2, aff_b_idx) = extract_mul_y_grad(mul2, stack)?;

    // The other Mul input must be Affine with scale=1.0
    let aff_b = match &stack[aff_b_idx] {
        DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(a))
            if a.scale == 1.0 =>
        {
            a
        }
        _ => return None,
    };

    // Its input must be Binary::Multiply with a ClampedYGradient
    let mul1 = match &stack[aff_b.input_index] {
        DensityFunctionComponent::Dependent(DependentDensityFunction::Binary(b)) => b,
        _ => return None,
    };
    let (grad1, aff_a_idx) = extract_mul_y_grad(mul1, stack)?;

    // The other Mul input must be Affine with scale=1.0
    let aff_a = match &stack[aff_a_idx] {
        DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(a))
            if a.scale == 1.0 =>
        {
            a
        }
        _ => return None,
    };

    // Both gradients must have a Y range where they saturate to 1.0
    let (g1_min, g1_max) = Slide::saturate_one_range(&grad1)?;
    let (g2_min, g2_max) = Slide::saturate_one_range(&grad2)?;

    // Intersect the two ranges
    let fast_min = g1_min.max(g2_min);
    let fast_max = g1_max.min(g2_max);
    if fast_min >= fast_max {
        return None; // No overlapping fast-path range
    }

    let combined_offset = aff_a.offset + aff_b.offset + aff_c.offset;

    Some(Slide {
        input_index: aff_a.input_index,
        grad1,
        grad2,
        offset_a: aff_a.offset,
        offset_b: aff_b.offset,
        offset_c: aff_c.offset,
        combined_offset,
        fast_path_min_y: fast_min,
        fast_path_max_y: fast_max,
        min_value: outer_min,
        max_value: outer_max,
    })
}

fn try_flatten_spline(
    spline_idx: usize,
    stack: &[DensityFunctionComponent],
    grid_size: usize,
) -> Option<FlattenedSpline> {
    // 1. Collect all unique coordinate indices referenced by this spline
    let spline = match &stack[spline_idx] {
        DensityFunctionComponent::Dependent(DependentDensityFunction::Spline(s)) => s,
        _ => return None,
    };

    let mut all_coords = Vec::new();
    spline.visit_input_indices(&mut |idx| {
        if !all_coords.contains(&idx) {
            all_coords.push(idx);
        }
    });
    all_coords.sort_unstable();

    if all_coords.is_empty() {
        return None;
    }

    // 2. Determine independent coordinates: those whose dependency cones
    //    don't contain any other collected coordinate.
    let mut independent = Vec::new();
    for &coord_idx in &all_coords {
        let mut is_derived = false;
        // Walk backward from coord_idx through its dependency cone
        let mut visited = vec![false; coord_idx + 1];
        let mut queue = vec![coord_idx];
        visited[coord_idx] = true;
        while let Some(current) = queue.pop() {
            stack[current].visit_input_indices(&mut |dep_idx| {
                if dep_idx < visited.len() && !visited[dep_idx] {
                    visited[dep_idx] = true;
                    queue.push(dep_idx);
                }
            });
        }
        // Check if any OTHER collected coordinate is reachable from this one
        for &other in &all_coords {
            if other != coord_idx && other < visited.len() && visited[other] {
                is_derived = true;
                break;
            }
        }
        if !is_derived {
            independent.push(coord_idx);
        }
    }

    if independent.len() != 3 {
        return None;
    }

    let coord_indices = [independent[0], independent[1], independent[2]];

    // 3. Get ranges of the 3 independent coordinates.
    //    Use static ranges, but also extract location ranges from the spline tree itself
    //    to handle cases where static range analysis is degenerate (e.g., Abs range bug).
    let mut coord_min = [f32::INFINITY; 3];
    let mut coord_max = [f32::NEG_INFINITY; 3];
    // Seed from static ranges
    for i in 0..3 {
        let smin = stack[coord_indices[i]].min_value();
        let smax = stack[coord_indices[i]].max_value();
        coord_min[i] = coord_min[i].min(smin);
        coord_max[i] = coord_max[i].max(smax);
    }
    // Also extract ranges from spline location arrays
    fn collect_spline_ranges(
        spline: &Spline,
        coord_indices: &[usize; 3],
        coord_min: &mut [f32; 3],
        coord_max: &mut [f32; 3],
    ) {
        for (i, &idx) in coord_indices.iter().enumerate() {
            if spline.input_index == idx {
                if let (Some(&first), Some(&last)) =
                    (spline.locations.first(), spline.locations.last())
                {
                    coord_min[i] = coord_min[i].min(first);
                    coord_max[i] = coord_max[i].max(last);
                }
            }
        }
        for val in spline.values.iter() {
            if let SplineValue::Spline(nested) = val {
                collect_spline_ranges(nested, coord_indices, coord_min, coord_max);
            }
        }
    }
    collect_spline_ranges(spline, &coord_indices, &mut coord_min, &mut coord_max);

    for i in 0..3 {
        // Ensure non-degenerate range
        if (coord_max[i] - coord_min[i]).abs() < 1e-10 {
            return None;
        }
    }

    let coord_inv_range = [
        1.0 / (coord_max[0] - coord_min[0]),
        1.0 / (coord_max[1] - coord_min[1]),
        1.0 / (coord_max[2] - coord_min[2]),
    ];

    // 3b. Count unique knot positions per axis to determine per-axis grid sizes.
    //     More knots = more features = need higher resolution.
    let mut knot_counts = [0usize; 3];
    fn count_knots_per_axis(
        spline: &Spline,
        coord_indices: &[usize; 3],
        knot_counts: &mut [usize; 3],
    ) {
        for (i, &idx) in coord_indices.iter().enumerate() {
            if spline.input_index == idx {
                knot_counts[i] = knot_counts[i].max(spline.locations.len());
            }
        }
        for val in spline.values.iter() {
            if let SplineValue::Spline(nested) = val {
                count_knots_per_axis(nested, coord_indices, knot_counts);
            }
        }
    }
    count_knots_per_axis(spline, &coord_indices, &mut knot_counts);

    // Per-axis grid: at least base_grid, but scale up for axes with many knots.
    // Use ~8 grid points per knot interval for good cubic interpolation.
    let mut grid_sizes = [0usize; 3];
    for i in 0..3 {
        let knot_based = if knot_counts[i] > 1 {
            (knot_counts[i] - 1) * 8
        } else {
            grid_size
        };
        grid_sizes[i] = grid_size.max(knot_based);
    }

    let strides = [grid_sizes[1] * grid_sizes[2], grid_sizes[2], 1];

    // 4. Determine which stack entries need forward evaluation
    //    (entries between min(independent) and spline_idx that are in the dependency cone)
    let min_coord = coord_indices[0]; // already sorted

    // Collect the set of entries we need to evaluate: everything in the dependency cone
    // of each all_coords entry, from min_coord..=max_coord (excluding the independent coords,
    // which we set directly)
    let mut needs_eval: Vec<usize> = Vec::new();
    for &c in &all_coords {
        if !coord_indices.contains(&c) {
            needs_eval.push(c);
        }
    }
    // Also find intermediate entries between min_coord and max_coord that feed into derived coords
    for &derived in &needs_eval.clone() {
        let mut queue = vec![derived];
        let mut visited_set = vec![false; stack.len()];
        visited_set[derived] = true;
        while let Some(current) = queue.pop() {
            stack[current].visit_input_indices(&mut |dep_idx| {
                if dep_idx >= min_coord && !visited_set[dep_idx] {
                    visited_set[dep_idx] = true;
                    if !coord_indices.contains(&dep_idx) && !needs_eval.contains(&dep_idx) {
                        needs_eval.push(dep_idx);
                    }
                    queue.push(dep_idx);
                }
            });
        }
    }
    needs_eval.sort_unstable();
    needs_eval.dedup();

    // 5. Build the LUT
    let total = grid_sizes[0] * grid_sizes[1] * grid_sizes[2];
    let mut lut = vec![0.0f32; total];
    let mut lut_min = f32::INFINITY;
    let mut lut_max = f32::NEG_INFINITY;

    let mut cache = vec![0.0f32; stack.len()];

    for i in 0..grid_sizes[0] {
        let c0 =
            coord_min[0] + (i as f32 / (grid_sizes[0] - 1) as f32) * (coord_max[0] - coord_min[0]);
        cache[coord_indices[0]] = c0;

        for j in 0..grid_sizes[1] {
            let c1 = coord_min[1]
                + (j as f32 / (grid_sizes[1] - 1) as f32) * (coord_max[1] - coord_min[1]);
            cache[coord_indices[1]] = c1;

            for k in 0..grid_sizes[2] {
                let c2 = coord_min[2]
                    + (k as f32 / (grid_sizes[2] - 1) as f32) * (coord_max[2] - coord_min[2]);
                cache[coord_indices[2]] = c2;

                // Forward-evaluate derived entries
                for &entry_idx in &needs_eval {
                    cache[entry_idx] = stack[entry_idx].sample_cached(&cache, stack, IVec3::ZERO);
                }

                // Evaluate the spline
                let value = spline.sample_cached(&cache, stack, IVec3::ZERO);
                let lut_idx = i * strides[0] + j * strides[1] + k;
                lut[lut_idx] = value;
                lut_min = lut_min.min(value);
                lut_max = lut_max.max(value);
            }
        }
    }

    Some(FlattenedSpline {
        coord_indices,
        coord_min,
        coord_inv_range,
        grid_sizes,
        strides,
        lut: lut.into_boxed_slice(),
        min_value: lut_min,
        max_value: lut_max,
    })
}

fn optimize_stack(stack: &mut Vec<DensityFunctionComponent>, roots: &mut [usize]) {
    let n = stack.len();
    if n == 0 {
        return;
    }

    let mut redirect: Vec<usize> = (0..n).collect();
    let mut affine_fusions = 0usize;
    let mut piecewise_affine_fusions = 0usize;
    let mut constants_folded = 0usize;
    let mut caches_eliminated = 0usize;
    let mut identities_eliminated = 0usize;
    let mut binary_demotions = 0usize;
    let mut slide_fusions = 0usize;

    // Phase 1: Forward pass — peephole optimize
    for i in 0..n {
        // 1. Resolve cache wrapper redirects (only CacheOnce and CacheAllInCell;
        //    FlatCache and Cache2d are kept alive for column caching)
        let is_eliminated_cache = matches!(
            &stack[i],
            DensityFunctionComponent::Wrapper(
                WrapperDensityFunction::CacheOnce(_) | WrapperDensityFunction::CacheAllInCell(_)
            )
        );

        if is_eliminated_cache {
            let input_index = match &stack[i] {
                DensityFunctionComponent::Wrapper(WrapperDensityFunction::CacheOnce(x)) => {
                    x.input_index
                }
                DensityFunctionComponent::Wrapper(WrapperDensityFunction::CacheAllInCell(x)) => {
                    x.input_index
                }
                _ => unreachable!(),
            };
            redirect[i] = redirect[input_index];
            caches_eliminated += 1;
            continue;
        }

        // 2. Apply redirects to current entry's inputs
        stack[i].rewrite_indices(&redirect);

        // 3. Binary optimizations: constant folding, demotion, and range elimination
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::Binary(bin)) =
            &stack[i]
        {
            let c1 = stack[bin.input1_index].as_constant();
            let c2 = stack[bin.input2_index].as_constant();
            let replacement = match (c1, c2, bin.operation) {
                // Both constant → fold for all operations
                (Some(a), Some(b), op) => {
                    let result = match op {
                        BinaryOperation::Add => a + b,
                        BinaryOperation::Multiply => a * b,
                        BinaryOperation::Min => a.min(b),
                        BinaryOperation::Max => a.max(b),
                    };
                    Some(DensityFunctionComponent::Independent(
                        IndependentDensityFunction::Constant(result),
                    ))
                }
                // One constant, Add/Multiply → demote to Linear
                (Some(c), None, BinaryOperation::Add | BinaryOperation::Multiply)
                | (None, Some(c), BinaryOperation::Add | BinaryOperation::Multiply) => {
                    let input_index = if c1.is_some() {
                        bin.input2_index
                    } else {
                        bin.input1_index
                    };
                    let operation = match bin.operation {
                        BinaryOperation::Add => LinearOperation::Add,
                        BinaryOperation::Multiply => LinearOperation::Multiply,
                        _ => unreachable!(),
                    };
                    Some(DensityFunctionComponent::Dependent(
                        DependentDensityFunction::Linear(Linear {
                            input_index,
                            min_value: bin.min_value,
                            max_value: bin.max_value,
                            argument: c,
                            operation,
                        }),
                    ))
                }
                _ => None,
            };
            if let Some(r) = replacement {
                if r.as_constant().is_some() {
                    constants_folded += 1;
                } else {
                    binary_demotions += 1;
                }
                stack[i] = r;
                // Fall through — the new Linear/Constant will be caught by subsequent steps
            }
        }

        // 3b. Binary Min/Max range elimination
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::Binary(bin)) =
            &stack[i]
        {
            let in1 = &stack[bin.input1_index];
            let in2 = &stack[bin.input2_index];
            match bin.operation {
                // Min(x, y) where x.max <= y.min → x always wins
                BinaryOperation::Min => {
                    if in1.max_value() <= in2.min_value() {
                        redirect[i] = bin.input1_index;
                        identities_eliminated += 1;
                        continue;
                    } else if in2.max_value() <= in1.min_value() {
                        redirect[i] = bin.input2_index;
                        identities_eliminated += 1;
                        continue;
                    }
                }
                // Max(x, y) where x.min >= y.max → x always wins
                BinaryOperation::Max => {
                    if in1.min_value() >= in2.max_value() {
                        redirect[i] = bin.input1_index;
                        identities_eliminated += 1;
                        continue;
                    } else if in2.min_value() >= in1.max_value() {
                        redirect[i] = bin.input2_index;
                        identities_eliminated += 1;
                        continue;
                    }
                }
                _ => {}
            }
        }

        // 4. Constant folding for all single-input operations
        let folded = match &stack[i] {
            DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(lin)) => stack
                [lin.input_index]
                .as_constant()
                .map(|c| match lin.operation {
                    LinearOperation::Add => c + lin.argument,
                    LinearOperation::Multiply => c * lin.argument,
                }),
            DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(aff)) => stack
                [aff.input_index]
                .as_constant()
                .map(|c| c.mul_add(aff.scale, aff.offset)),
            DensityFunctionComponent::Dependent(DependentDensityFunction::PiecewiseAffine(pa)) => {
                stack[pa.input_index].as_constant().map(|c| {
                    let scale = if c < 0.0 { pa.neg_scale } else { pa.pos_scale };
                    c.mul_add(scale, pa.offset)
                })
            }
            DensityFunctionComponent::Dependent(DependentDensityFunction::Unary(u)) => stack
                [u.input_index]
                .as_constant()
                .map(|c| u.operation.apply(c)),
            DensityFunctionComponent::Dependent(DependentDensityFunction::Clamp(cl)) => stack
                [cl.input_index]
                .as_constant()
                .map(|c| c.clamp(cl.min_value, cl.max_value)),
            _ => None,
        };
        if let Some(constant) = folded {
            stack[i] = DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(
                constant,
            ));
            constants_folded += 1;
            continue;
        }

        // 5. Affine fusion
        let fused = match &stack[i] {
            DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(lin)) => {
                let input = &stack[lin.input_index];
                match (&lin.operation, input) {
                    // Linear::Add(x, a) where x is Linear::Add(y, b) → Affine(y, 1.0, a+b)
                    (
                        LinearOperation::Add,
                        DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(
                            inner,
                        )),
                    ) if inner.operation == LinearOperation::Add => {
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            1.0,
                            lin.argument + inner.argument,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale: 1.0,
                                offset: lin.argument + inner.argument,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Linear::Mul(x, a) where x is Linear::Mul(y, b) → Affine(y, a*b, 0.0)
                    (
                        LinearOperation::Multiply,
                        DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(
                            inner,
                        )),
                    ) if inner.operation == LinearOperation::Multiply => {
                        let scale = lin.argument * inner.argument;
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            scale,
                            0.0,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale,
                                offset: 0.0,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Linear::Mul(x, a) where x is Linear::Add(y, b) → Affine(y, a, b*a)
                    (
                        LinearOperation::Multiply,
                        DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(
                            inner,
                        )),
                    ) if inner.operation == LinearOperation::Add => {
                        let offset = inner.argument * lin.argument;
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            lin.argument,
                            offset,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale: lin.argument,
                                offset,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Linear::Add(x, a) where x is Linear::Mul(y, b) → Affine(y, b, a)
                    (
                        LinearOperation::Add,
                        DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(
                            inner,
                        )),
                    ) if inner.operation == LinearOperation::Multiply => {
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            inner.argument,
                            lin.argument,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale: inner.argument,
                                offset: lin.argument,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Linear::Add(x, a) where x is Affine(y, s, o) → Affine(y, s, o+a)
                    (
                        LinearOperation::Add,
                        DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(
                            inner,
                        )),
                    ) => {
                        let offset = inner.offset + lin.argument;
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            inner.scale,
                            offset,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale: inner.scale,
                                offset,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Linear::Mul(x, a) where x is Affine(y, s, o) → Affine(y, s*a, o*a)
                    (
                        LinearOperation::Multiply,
                        DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(
                            inner,
                        )),
                    ) => {
                        let scale = inner.scale * lin.argument;
                        let offset = inner.offset * lin.argument;
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            scale,
                            offset,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale,
                                offset,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    _ => None,
                }
            }
            DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(aff)) => {
                let input = &stack[aff.input_index];
                match input {
                    // Affine(x, s2, o2) where x is Affine(y, s1, o1) → Affine(y, s1*s2, o1*s2+o2)
                    DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(
                        inner,
                    )) => {
                        let scale = inner.scale * aff.scale;
                        let offset = inner.offset.mul_add(aff.scale, aff.offset);
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            scale,
                            offset,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale,
                                offset,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Affine(x, s2, o2) where x is Linear::Add(y, b) → Affine(y, s2, b*s2+o2)
                    DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(
                        inner,
                    )) if inner.operation == LinearOperation::Add => {
                        let offset = inner.argument.mul_add(aff.scale, aff.offset);
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            aff.scale,
                            offset,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale: aff.scale,
                                offset,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    // Affine(x, s2, o2) where x is Linear::Mul(y, b) → Affine(y, b*s2, o2)
                    DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(
                        inner,
                    )) if inner.operation == LinearOperation::Multiply => {
                        let scale = inner.argument * aff.scale;
                        let (min_value, max_value) = Affine::compute_range(
                            stack[inner.input_index].min_value(),
                            stack[inner.input_index].max_value(),
                            scale,
                            aff.offset,
                        );
                        Some(DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: inner.input_index,
                                scale,
                                offset: aff.offset,
                                min_value,
                                max_value,
                            }),
                        ))
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        if let Some(replacement) = fused {
            stack[i] = replacement;
            affine_fusions += 1;
            // Don't continue — fall through to identity/zero check below
        }

        // 5b. Convert remaining standalone Linear to Affine (removes tracing span, uses FMA)
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(lin)) =
            &stack[i]
        {
            let (scale, offset) = match lin.operation {
                LinearOperation::Add => (1.0, lin.argument),
                LinearOperation::Multiply => (lin.argument, 0.0),
            };
            let (min_value, max_value) = Affine::compute_range(
                stack[lin.input_index].min_value(),
                stack[lin.input_index].max_value(),
                scale,
                offset,
            );
            stack[i] =
                DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(Affine {
                    input_index: lin.input_index,
                    scale,
                    offset,
                    min_value,
                    max_value,
                }));
        }

        // 6. Identity/zero elimination
        match &stack[i] {
            DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(aff)) => {
                if aff.scale == 1.0 && aff.offset == 0.0 {
                    // Identity
                    redirect[i] = aff.input_index;
                    identities_eliminated += 1;
                    continue;
                }
                if aff.scale == 0.0 {
                    // Constant
                    stack[i] = DensityFunctionComponent::Independent(
                        IndependentDensityFunction::Constant(aff.offset),
                    );
                    constants_folded += 1;
                    continue;
                }
            }
            DensityFunctionComponent::Dependent(DependentDensityFunction::Linear(lin)) => {
                match lin.operation {
                    LinearOperation::Add if lin.argument == 0.0 => {
                        redirect[i] = lin.input_index;
                        identities_eliminated += 1;
                        continue;
                    }
                    LinearOperation::Multiply if lin.argument == 1.0 => {
                        redirect[i] = lin.input_index;
                        identities_eliminated += 1;
                        continue;
                    }
                    LinearOperation::Multiply if lin.argument == 0.0 => {
                        stack[i] = DensityFunctionComponent::Independent(
                            IndependentDensityFunction::Constant(0.0),
                        );
                        constants_folded += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // 7. Clamp of in-range elimination
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::Clamp(clamp)) =
            &stack[i]
        {
            let input = &stack[clamp.input_index];
            if input.min_value() >= clamp.min_value && input.max_value() <= clamp.max_value {
                redirect[i] = clamp.input_index;
                identities_eliminated += 1;
            }
        }

        // 8. RangeChoice range-based elimination: if the input's static range proves
        //    it always falls in or always falls out, redirect to the known branch.
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::RangeChoice(rc)) =
            &stack[i]
        {
            let input = &stack[rc.input_index];
            if input.max_value() < rc.min_inclusion_value
                || input.min_value() >= rc.max_exclusion_value
            {
                // Always out-of-range
                redirect[i] = rc.when_out_index;
                identities_eliminated += 1;
                continue;
            }
            if input.min_value() >= rc.min_inclusion_value
                && input.max_value() < rc.max_exclusion_value
            {
                // Always in-range
                redirect[i] = rc.when_in_index;
                identities_eliminated += 1;
                continue;
            }
        }

        // 9. Unary→Affine fusion: Affine(Unary::QuarterNegative/HalfNegative(x)) → PiecewiseAffine
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(aff)) =
            &stack[i]
        {
            if let DensityFunctionComponent::Dependent(DependentDensityFunction::Unary(u)) =
                &stack[aff.input_index]
            {
                let pwa = match u.operation {
                    // QuarterNegative(x) = if x<0 { 0.25*x } else { x }
                    // Affine(QuarterNeg(x), s, o) = if x<0 { 0.25*s*x + o } else { s*x + o }
                    UnaryOperation::QuarterNegative => Some((aff.scale * 0.25, aff.scale)),
                    // HalfNegative(x) = if x<0 { 0.5*x } else { x }
                    // Affine(HalfNeg(x), s, o) = if x<0 { 0.5*s*x + o } else { s*x + o }
                    UnaryOperation::HalfNegative => Some((aff.scale * 0.5, aff.scale)),
                    _ => None,
                };
                if let Some((neg_scale, pos_scale)) = pwa {
                    let (min_value, max_value) = PiecewiseAffine::compute_range(
                        stack[u.input_index].min_value(),
                        stack[u.input_index].max_value(),
                        neg_scale,
                        pos_scale,
                        aff.offset,
                    );
                    stack[i] = DensityFunctionComponent::Dependent(
                        DependentDensityFunction::PiecewiseAffine(PiecewiseAffine {
                            input_index: u.input_index,
                            neg_scale,
                            pos_scale,
                            offset: aff.offset,
                            min_value,
                            max_value,
                        }),
                    );
                    piecewise_affine_fusions += 1;
                }
            }
        }

        // 10. Binary same-index identity: Min(x,x)→x, Max(x,x)→x, Add(x,x)→2*x, Mul(x,x)→x²
        if let DensityFunctionComponent::Dependent(DependentDensityFunction::Binary(bin)) =
            &stack[i]
        {
            if bin.input1_index == bin.input2_index {
                match bin.operation {
                    BinaryOperation::Min | BinaryOperation::Max => {
                        redirect[i] = bin.input1_index;
                        identities_eliminated += 1;
                        continue;
                    }
                    BinaryOperation::Add => {
                        // Add(x, x) = 2*x
                        let (min_value, max_value) = Affine::compute_range(
                            stack[bin.input1_index].min_value(),
                            stack[bin.input1_index].max_value(),
                            2.0,
                            0.0,
                        );
                        stack[i] = DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Affine(Affine {
                                input_index: bin.input1_index,
                                scale: 2.0,
                                offset: 0.0,
                                min_value,
                                max_value,
                            }),
                        );
                        binary_demotions += 1;
                    }
                    BinaryOperation::Multiply => {
                        // Mul(x, x) = x²
                        let in_min = stack[bin.input1_index].min_value();
                        let in_max = stack[bin.input1_index].max_value();
                        let (min_value, max_value) = if in_min >= 0.0 {
                            (in_min * in_min, in_max * in_max)
                        } else if in_max <= 0.0 {
                            (in_max * in_max, in_min * in_min)
                        } else {
                            (0.0, (in_min * in_min).max(in_max * in_max))
                        };
                        stack[i] = DensityFunctionComponent::Dependent(
                            DependentDensityFunction::Unary(Unary {
                                input_index: bin.input1_index,
                                operation: UnaryOperation::Square,
                                min_value,
                                max_value,
                            }),
                        );
                        binary_demotions += 1;
                    }
                }
            }
        }

        // 11. Slide fusion: detect Affine(+c) ← Mul(ygrad2, Affine(+b) ← Mul(ygrad1, Affine(+a, input)))
        //     Fuses the 5-node chain into a single Slide operation.
        if let Some(slide) = try_build_slide(i, &stack) {
            stack[i] = DensityFunctionComponent::Dependent(DependentDensityFunction::Slide(slide));
            slide_fusions += 1;
        }
    }

    // Phase 2: Rewrite all indices using redirect table
    // Transitively resolve the redirect table
    for i in 0..n {
        let mut target = redirect[i];
        while redirect[target] != target {
            target = redirect[target];
        }
        redirect[i] = target;
    }

    for entry in stack.iter_mut() {
        entry.rewrite_indices(&redirect);
    }
    for root in roots.iter_mut() {
        *root = redirect[*root];
    }

    // Phase 3: Flatten splines to lookup tables
    let mut splines_flattened = 0usize;
    // let grid_size = 128;
    // for i in 0..stack.len() {
    //     if let DensityFunctionComponent::Dependent(DependentDensityFunction::Spline(_)) = &stack[i]
    //     {
    //         if let Some(flat) = try_flatten_spline(i, stack, grid_size) {
    //             stack[i] = DensityFunctionComponent::Dependent(
    //                 DependentDensityFunction::FlattenedSpline(flat),
    //             );
    //             splines_flattened += 1;
    //         }
    //     }
    // }

    info!(
        stack_size = n,
        affine_fusions,
        piecewise_affine_fusions,
        constants_folded,
        caches_eliminated,
        identities_eliminated,
        binary_demotions,
        slide_fusions,
        splines_flattened,
        "Density function stack optimized"
    );
}

/// Compute which stack entries need recomputation per block (Y changes)
/// vs once per column (X,Z changes only).
///
/// Forward propagation: a node is per_block if it intrinsically depends on Y
/// OR if any of its inputs is per_block. FlatCache and Cache2d force their
/// output to column-only regardless of inputs.
fn compute_per_block(stack: &[DensityFunctionComponent], _roots: &[usize]) -> Vec<bool> {
    let mut per_block = vec![false; stack.len()];

    for i in 0..stack.len() {
        // Check if intrinsically per_block (uses pos.y directly)
        let intrinsic = match &stack[i] {
            DensityFunctionComponent::Independent(f) => matches!(
                f,
                IndependentDensityFunction::OldBlendedNoise(_)
                    | IndependentDensityFunction::Noise(_)
                    | IndependentDensityFunction::ClampedYGradient(_)
            ),
            DensityFunctionComponent::Dependent(f) => matches!(
                f,
                DependentDensityFunction::Slide(_)
                    | DependentDensityFunction::ShiftedNoise(_)
                    | DependentDensityFunction::WeirdScaled(_)
                    | DependentDensityFunction::FindTopSurface(_)
            ),
            DensityFunctionComponent::Wrapper(_) => false,
        };

        if intrinsic {
            per_block[i] = true;
        } else {
            // per_block if any input is per_block
            stack[i].visit_input_indices(&mut |idx| {
                if per_block[idx] {
                    per_block[i] = true;
                }
            });
        }

        // FlatCache/Cache2d override: force column-only
        if matches!(
            &stack[i],
            DensityFunctionComponent::Wrapper(
                WrapperDensityFunction::FlatCache(_) | WrapperDensityFunction::Cache2d(_)
            )
        ) {
            per_block[i] = false;
        }
    }

    let column_only_count = per_block.iter().filter(|&&b| !b).count();
    let per_block_count = per_block.iter().filter(|&&b| b).count();
    info!(
        column_only_count,
        per_block_count, "Density function per_block analysis"
    );

    per_block
}

/// Reorder the stack into three zones for optimal `evaluate_forward` performance:
///
///   Zone A `[0..column_boundary)`:  column-only entries reachable from final_density
///   Zone B `[column_boundary..fd_boundary)`: per-Y entries reachable from final_density
///   Zone C `[fd_boundary..n)`:               entries not reachable from final_density
///
/// Within each zone, topological order is maintained (children before parents).
/// Returns `(column_boundary, fd_boundary)`.
fn reorder_stack_for_evaluation(
    stack: &mut Vec<DensityFunctionComponent>,
    per_block: &mut Vec<bool>,
    node_labels: &mut Vec<String>,
    roots: &mut [usize],
    final_density_root_idx: usize,
) -> (usize, usize) {
    let n = stack.len();
    let fd_index = roots[final_density_root_idx];

    // Step 1: Compute full reachability from final_density (backward walk)
    let mut fd_reachable = vec![false; n];
    {
        let mut worklist = vec![fd_index];
        fd_reachable[fd_index] = true;
        while let Some(idx) = worklist.pop() {
            stack[idx].visit_input_indices(&mut |input| {
                if !fd_reachable[input] {
                    fd_reachable[input] = true;
                    worklist.push(input);
                }
            });
        }
    }

    // Step 2: Compute per-Y reachability (backward walk, stopping at FlatCache/Cache2d)
    // These are entries whose per-Y values are actually consumed by final_density.
    let mut per_y_reachable = vec![false; n];
    {
        let mut worklist = vec![fd_index];
        per_y_reachable[fd_index] = true;
        while let Some(idx) = worklist.pop() {
            // Stop at FlatCache/Cache2d: their inputs' per-Y values are never used
            let is_cache_boundary = matches!(
                &stack[idx],
                DensityFunctionComponent::Wrapper(
                    WrapperDensityFunction::FlatCache(_) | WrapperDensityFunction::Cache2d(_)
                )
            );
            if !is_cache_boundary {
                stack[idx].visit_input_indices(&mut |input| {
                    if !per_y_reachable[input] {
                        per_y_reachable[input] = true;
                        worklist.push(input);
                    }
                });
            }
        }
    }

    // Step 3: Classify each entry into a zone
    //   Zone A (0): fd_reachable AND NOT (per_y_reachable AND per_block)
    //   Zone B (1): per_y_reachable AND per_block
    //   Zone C (2): NOT fd_reachable
    let mut zone = vec![2u8; n];
    for i in 0..n {
        if fd_reachable[i] {
            if per_y_reachable[i] && per_block[i] {
                zone[i] = 1; // Zone B: per-Y evaluation needed
            } else {
                zone[i] = 0; // Zone A: column-only (or shielded by FlatCache)
            }
        }
    }

    // Safety check: verify no Zone A entry depends on a Zone B entry.
    // This would happen if a FlatCache input is shared with a direct per-Y consumer.
    // In vanilla Minecraft, this never occurs; all FlatCache inputs are exclusive.
    for i in 0..n {
        if zone[i] == 0 {
            stack[i].visit_input_indices(&mut |input| {
                debug_assert!(
                    zone[input] != 1,
                    "Zone A entry {} depends on Zone B entry {} — \
                     FlatCache/Cache2d input is shared with a direct per-Y consumer. \
                     This case requires special handling.",
                    i,
                    input
                );
            });
        }
    }

    // Step 4: Create permutation sorted by (zone, original_index).
    // Since the original stack is in topological order, sorting by original_index
    // within each zone preserves topological order within that zone.
    let mut sorted_indices: Vec<usize> = (0..n).collect();
    sorted_indices.sort_by_key(|&i| (zone[i], i));

    // Step 5: Build old→new index mapping
    let mut old_to_new = vec![0usize; n];
    for (new_idx, &old_idx) in sorted_indices.iter().enumerate() {
        old_to_new[old_idx] = new_idx;
    }

    // Step 6: Apply permutation to stack, per_block, node_labels
    let old_stack: Vec<DensityFunctionComponent> = stack.drain(..).collect();
    let old_per_block: Vec<bool> = per_block.drain(..).collect();
    let old_labels: Vec<String> = node_labels.drain(..).collect();

    for &old_idx in &sorted_indices {
        stack.push(old_stack[old_idx].clone());
        per_block.push(old_per_block[old_idx]);
        node_labels.push(old_labels[old_idx].clone());
    }

    // Step 7: Rewrite all index references using old→new mapping
    for entry in stack.iter_mut() {
        entry.rewrite_indices(&old_to_new);
    }

    // Step 8: Update root indices
    for root in roots.iter_mut() {
        *root = old_to_new[*root];
    }

    // Step 9: Compute zone boundaries
    let zone_a_count = zone.iter().filter(|&&z| z == 0).count();
    let zone_b_count = zone.iter().filter(|&&z| z == 1).count();
    let zone_c_count = zone.iter().filter(|&&z| z == 2).count();
    let column_boundary = zone_a_count;
    let fd_boundary = zone_a_count + zone_b_count;

    info!(
        zone_a_count,
        zone_b_count,
        zone_c_count,
        column_boundary,
        fd_boundary,
        "Stack reordered for evaluation zones"
    );

    // Count per-zone noise evaluations (expensive operations)
    let count_noises = |range: std::ops::Range<usize>| -> usize {
        range
            .filter(|&i| {
                matches!(
                    &stack[i],
                    DensityFunctionComponent::Independent(
                        IndependentDensityFunction::OldBlendedNoise(_)
                            | IndependentDensityFunction::Noise(_)
                            | IndependentDensityFunction::ShiftA(_)
                            | IndependentDensityFunction::ShiftB(_)
                            | IndependentDensityFunction::Shift(_)
                    ) | DensityFunctionComponent::Dependent(
                        DependentDensityFunction::ShiftedNoise(_)
                            | DependentDensityFunction::WeirdScaled(_)
                    )
                )
            })
            .count()
    };
    info!(
        zone_a_noises = count_noises(0..column_boundary),
        zone_b_noises = count_noises(column_boundary..fd_boundary),
        zone_c_noises = count_noises(fd_boundary..n),
        "Noise evaluations per zone"
    );

    (column_boundary, fd_boundary)
}

/// Persistent cache for density function evaluation.
/// Reuse across calls to `final_density` within the same chunk generation
/// to cache column-only values (FlatCache/Cache2d dependency cones).
pub struct DensityCache {
    scratch: Vec<f32>,
    last_x: i32,
    last_z: i32,
    column_valid: bool,
}

/// Pre-populated cache holding Zone A (column-only) results for all 289 (17x17) XZ positions
/// within a chunk column plus the +16 boundary. Eliminates the `column_changed` branch from
/// the per-block hot path when evaluating `final_density`.
///
/// The 17x17 grid covers local coordinates 0..=16 in both X and Z, which is needed because
/// `fill_plane` samples corner positions at block_x+0, block_x+4, ..., block_x+16.
pub struct ChunkColumnCache {
    /// `column_data[xz_idx * zone_a_count + entry_idx]`
    /// where `xz_idx = local_x * GRID_SIDE + local_z` (row-major).
    column_data: Vec<f32>,
    zone_a_count: usize,
    pub(crate) base_block_x: i32,
    pub(crate) base_block_z: i32,
    /// Scratch buffer (len == stack.len()), reused per `final_density_from_column_cache` call.
    pub scratch: Vec<f32>,
}

impl ChunkColumnCache {
    const GRID_SIDE: i32 = 17;

    /// Load pre-computed Zone A values for the given local (x, z) into scratch[0..zone_a_count).
    #[inline]
    pub fn load_column(&mut self, local_x: i32, local_z: i32) {
        let xz_idx = (local_x * Self::GRID_SIDE + local_z) as usize;
        let off = xz_idx * self.zone_a_count;
        self.scratch[..self.zone_a_count]
            .copy_from_slice(&self.column_data[off..off + self.zone_a_count]);
    }
}

/// Compute lazy RangeChoice optimization for Zone B.
///
/// Finds the RangeChoice in Zone B with the most exclusive entries and creates
/// two evaluation lists: one for when the condition is true (when_in branch)
/// and one for when it's false (when_out branch). Entries exclusive to the
/// inactive branch are omitted from the respective list.
#[cfg(feature = "lazy-range-choice")]
fn compute_lazy_range_choice(
    stack: &[DensityFunctionComponent],
    column_boundary: usize,
    final_density_index: usize,
) -> Option<LazyRangeChoice> {
    // Find RangeChoice nodes in Zone B
    let mut best: Option<LazyRangeChoice> = None;
    let mut best_savings = 0usize;

    for rc_idx in column_boundary..=final_density_index {
        let rc = match &stack[rc_idx] {
            DensityFunctionComponent::Dependent(DependentDensityFunction::RangeChoice(rc)) => rc,
            _ => continue,
        };

        // Compute reachable sets from when_in and when_out branches
        let extent = final_density_index + 1;
        let reachable_from_wi = reachable_backwards(rc.when_in_index, stack, extent);
        let reachable_from_wo = reachable_backwards(rc.when_out_index, stack, extent);

        // Compute reachable set from final_density WITHOUT going through this RC's branches.
        // We walk back from final_density but when we encounter this RC node, we only
        // follow the input edge (not when_in or when_out).
        let reachable_without_branches = {
            let mut visited = vec![false; final_density_index + 1];
            visited[final_density_index] = true;
            for i in (column_boundary..=final_density_index).rev() {
                if !visited[i] {
                    continue;
                }
                if i == rc_idx {
                    // Only follow the input edge, not the branch edges
                    if rc.input_index <= final_density_index {
                        visited[rc.input_index] = true;
                    }
                } else {
                    stack[i].visit_input_indices(&mut |dep| {
                        if dep <= final_density_index {
                            visited[dep] = true;
                        }
                    });
                }
            }
            visited
        };

        // Entries exclusive to when_out: reachable from WO, NOT from WI, NOT from other paths
        let wo_exclusive: Vec<usize> = (column_boundary..rc_idx)
            .filter(|&e| {
                reachable_from_wo[e] && !reachable_from_wi[e] && !reachable_without_branches[e]
            })
            .collect();

        // Entries exclusive to when_in: reachable from WI, NOT from WO, NOT from other paths
        let wi_exclusive: Vec<usize> = (column_boundary..rc_idx)
            .filter(|&e| {
                reachable_from_wi[e] && !reachable_from_wo[e] && !reachable_without_branches[e]
            })
            .collect();

        let savings = wo_exclusive.len().max(wi_exclusive.len());
        if savings <= best_savings {
            continue;
        }

        // Build branch-specific lists: Zone B entries AFTER input_index, minus exclusives.
        // The common prefix [column_boundary..=input_index] is always evaluated.
        let input_idx = rc.input_index;
        let branch_when_in: Vec<usize> = ((input_idx + 1)..=final_density_index)
            .filter(|e| !wo_exclusive.contains(e))
            .collect();
        let branch_when_out: Vec<usize> = ((input_idx + 1)..=final_density_index)
            .filter(|e| !wi_exclusive.contains(e))
            .collect();

        eprintln!(
            "  Lazy RangeChoice at [{}]: when_in skips {}, when_out skips {}",
            rc_idx,
            wo_exclusive.len(),
            wi_exclusive.len(),
        );

        best_savings = savings;
        best = Some(LazyRangeChoice {
            input_index: input_idx,
            min_inclusion: rc.min_inclusion_value,
            max_exclusion: rc.max_exclusion_value,
            branch_when_in: branch_when_in.into_boxed_slice(),
            branch_when_out: branch_when_out.into_boxed_slice(),
        });
    }

    best
}

#[cfg(feature = "lazy-range-choice")]
/// Walk backwards from `start` through input edges, returning a reachability bitmap
/// of size `extent`. Entries beyond `start` are always false.
fn reachable_backwards(
    start: usize,
    stack: &[DensityFunctionComponent],
    extent: usize,
) -> Vec<bool> {
    let mut visited = vec![false; extent];
    if start < extent {
        visited[start] = true;
    }
    for i in (0..=start.min(extent - 1)).rev() {
        if !visited[i] {
            continue;
        }
        stack[i].visit_input_indices(&mut |dep| {
            if dep < extent {
                visited[dep] = true;
            }
        });
    }
    visited
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
    let nr = &noise_settings.noise_router;
    let barrier_index = builder.component(&nr.barrier);
    let fluid_level_floodedness_index = builder.component(&nr.fluid_level_floodedness);
    let fluid_level_spread_index = builder.component(&nr.fluid_level_spread);
    let lava_index = builder.component(&nr.lava);
    let temperature_index = builder.component(&nr.temperature);
    let vegetation_index = builder.component(&nr.vegetation);
    let continents_index = builder.component(&nr.continents);
    let erosion_index = builder.component(&nr.erosion);
    let depth_index = builder.component(&nr.depth);
    let ridges_index = builder.component(&nr.ridges);
    let preliminary_surface_level_index = builder.component(&nr.preliminary_surface_level);
    let final_density_index = builder.component(&nr.final_density);
    let vein_toggle_index = builder.component(&nr.vein_toggle);
    let vein_ridged_index = builder.component(&nr.vein_ridged);
    let vein_gap_index = builder.component(&nr.vein_gap);

    let mut roots = [
        barrier_index,
        fluid_level_floodedness_index,
        fluid_level_spread_index,
        lava_index,
        temperature_index,
        vegetation_index,
        continents_index,
        erosion_index,
        depth_index,
        ridges_index,
        preliminary_surface_level_index,
        final_density_index,
        vein_toggle_index,
        vein_ridged_index,
        vein_gap_index,
    ];

    optimize_stack(&mut builder.stack, &mut roots);

    let mut per_block = compute_per_block(&builder.stack, &roots);

    // Build node labels: start with type labels, then overlay reference names
    let mut node_labels: Vec<String> = vec![String::new(); builder.stack.len()];
    for (ident, proto) in builder.functions.iter() {
        if let Some(&idx) = builder.built.get(proto) {
            if idx < node_labels.len() {
                node_labels[idx] = ident.to_string();
            }
        }
    }

    // Reorder the stack into evaluation zones for optimal forward evaluation:
    //   Zone A [0..column_boundary): column-only entries reachable from final_density
    //   Zone B [column_boundary..fd_boundary): per-Y entries for final_density
    //   Zone C [fd_boundary..): entries not reachable from final_density
    let (column_boundary, fd_boundary) = reorder_stack_for_evaluation(
        &mut builder.stack,
        &mut per_block,
        &mut node_labels,
        &mut roots,
        11, // final_density is roots[11]
    );

    let final_density_index = roots[11];

    // Compute lazy RangeChoice optimization for Zone B.
    #[cfg(feature = "lazy-range-choice")]
    let lazy_rc = compute_lazy_range_choice(&builder.stack, column_boundary, final_density_index);

    let router = NoiseRouter {
        barrier_index: roots[0],
        fluid_level_floodedness_index: roots[1],
        fluid_level_spread_index: roots[2],
        lava_index: roots[3],
        temperature_index: roots[4],
        vegetation_index: roots[5],
        continents_index: roots[6],
        erosion_index: roots[7],
        depth_index: roots[8],
        ridges_index: roots[9],
        preliminary_surface_level_index: roots[10],
        final_density_index,
        vein_toggle_index: roots[12],
        vein_ridged_index: roots[13],
        vein_gap_index: roots[14],
        per_block: per_block.into_boxed_slice(),
        column_boundary,
        fd_boundary,
        h_cell_blocks: builder_options.horizontal_cell_block_count,
        v_cell_blocks: builder_options.vertical_cell_block_count,
        stack: Box::from(builder.stack),
        node_labels: node_labels.into_boxed_slice(),
        #[cfg(feature = "lazy-range-choice")]
        lazy_rc,
    };

    router
}

#[cfg(feature = "lazy-range-choice")]
#[derive(Clone, Debug, PartialEq)]
/// Lazy RangeChoice evaluation data.
/// Zone B evaluation is split into a common prefix (up to and including the
/// RangeChoice input) and branch-specific tails. The input is evaluated first,
/// then based on the condition, only the needed branch entries are evaluated.
struct LazyRangeChoice {
    /// Stack index of the RangeChoice's input.
    input_index: usize,
    /// Range bounds for the condition.
    min_inclusion: f32,
    max_exclusion: f32,
    /// Zone B entries AFTER input_index when condition is TRUE (input in range).
    /// Excludes entries only needed by the when_out branch.
    branch_when_in: Box<[usize]>,
    /// Zone B entries AFTER input_index when condition is FALSE (input out of range).
    /// Excludes entries only needed by the when_in branch.
    branch_when_out: Box<[usize]>,
}

pub struct NoiseRouter {
    barrier_index: usize,
    fluid_level_floodedness_index: usize,
    fluid_level_spread_index: usize,
    lava_index: usize,
    temperature_index: usize,
    vegetation_index: usize,
    continents_index: usize,
    erosion_index: usize,
    depth_index: usize,
    ridges_index: usize,
    preliminary_surface_level_index: usize,
    final_density_index: usize,
    vein_toggle_index: usize,
    vein_ridged_index: usize,
    vein_gap_index: usize,
    /// per_block[i] == true means entry i depends on Y and must be recomputed per block.
    /// per_block[i] == false means entry i is column-only (cached across Y changes).
    per_block: Box<[bool]>,
    /// First index of Zone B (per-Y entries for final_density).
    /// Zone A [0..column_boundary): column-only entries reachable from final_density.
    column_boundary: usize,
    /// First index of Zone C (entries not reachable from final_density).
    /// Zone B [column_boundary..fd_boundary): per-Y entries for final_density.
    fd_boundary: usize,
    /// Horizontal cell size in blocks (typically 4).
    h_cell_blocks: usize,
    /// Vertical cell size in blocks (typically 8).
    v_cell_blocks: usize,
    stack: Box<[DensityFunctionComponent]>,
    node_labels: Box<[String]>,
    /// Lazy RangeChoice optimization for Zone B evaluation.
    /// If present, `final_density_from_column_cache` uses branch-specific
    /// evaluation lists to skip entries exclusive to the inactive branch.
    #[cfg(feature = "lazy-range-choice")]
    lazy_rc: Option<LazyRangeChoice>,
}

impl NoiseRouter {
    /// All noise router entries as (name, index) pairs.
    pub fn roots(&self) -> Vec<(&'static str, usize)> {
        vec![
            ("barrier", self.barrier_index),
            (
                "fluid_level_floodedness",
                self.fluid_level_floodedness_index,
            ),
            ("fluid_level_spread", self.fluid_level_spread_index),
            ("lava", self.lava_index),
            ("temperature", self.temperature_index),
            ("vegetation", self.vegetation_index),
            ("continents", self.continents_index),
            ("erosion", self.erosion_index),
            ("depth", self.depth_index),
            ("ridges", self.ridges_index),
            (
                "preliminary_surface_level",
                self.preliminary_surface_level_index,
            ),
            ("final_density", self.final_density_index),
            ("vein_toggle", self.vein_toggle_index),
            ("vein_ridged", self.vein_ridged_index),
            ("vein_gap", self.vein_gap_index),
        ]
    }

    pub fn column_boundary(&self) -> usize {
        self.column_boundary
    }

    pub fn final_density_idx(&self) -> usize {
        self.final_density_index
    }

    /// Evaluate a single stack entry using pre-computed cache values.
    #[inline]
    pub fn sample_entry(&self, index: usize, cache: &[f32], pos: IVec3) -> f32 {
        self.stack[index].sample_cached(cache, &self.stack, pos)
    }

    /// Print Zone B composition to stderr for profiling purposes.
    pub fn print_zone_stats(&self) {
        let zone_a = self.column_boundary;
        let zone_b = self.final_density_index + 1 - self.column_boundary;
        let zone_c = self.stack.len() - self.fd_boundary;
        eprintln!(
            "  Zone A (column-only):  {} entries [0..{})",
            zone_a, self.column_boundary
        );
        eprintln!(
            "  Zone B (per-Y final):  {} entries [{}..={}]",
            zone_b, self.column_boundary, self.final_density_index
        );
        eprintln!(
            "  Zone C (other roots):  {} entries [{}..{})",
            zone_c,
            self.fd_boundary,
            self.stack.len()
        );
        eprintln!("  Total stack: {} entries", self.stack.len());

        // Count Zone B by type
        let mut constants = 0usize;
        let mut old_blended = 0;
        let mut noise = 0;
        let mut shift = 0;
        let mut clamped_y = 0;
        let mut linear = 0;
        let mut affine = 0;
        let mut piecewise = 0;
        let mut slide = 0;
        let mut unary = 0;
        let mut binary = 0;
        let mut shifted_noise = 0;
        let mut weird_scaled = 0;
        let mut clamp = 0;
        let mut range_choice = 0;
        let mut spline = 0;
        let mut flat_spline = 0;
        let mut find_top = 0;
        for i in self.column_boundary..=self.final_density_index {
            match &self.stack[i] {
                DensityFunctionComponent::Independent(f) => match f {
                    IndependentDensityFunction::Constant(_) => constants += 1,
                    IndependentDensityFunction::OldBlendedNoise(_) => old_blended += 1,
                    IndependentDensityFunction::Noise(_) => noise += 1,
                    IndependentDensityFunction::ShiftA(_)
                    | IndependentDensityFunction::ShiftB(_)
                    | IndependentDensityFunction::Shift(_) => shift += 1,
                    IndependentDensityFunction::ClampedYGradient(_) => clamped_y += 1,
                    IndependentDensityFunction::EndIslands => {}
                },
                DensityFunctionComponent::Wrapper(_) => {}
                DensityFunctionComponent::Dependent(f) => match f {
                    DependentDensityFunction::Linear(_) => linear += 1,
                    DependentDensityFunction::Affine(_) => affine += 1,
                    DependentDensityFunction::PiecewiseAffine(_) => piecewise += 1,
                    DependentDensityFunction::Slide(_) => slide += 1,
                    DependentDensityFunction::Unary(_) => unary += 1,
                    DependentDensityFunction::Binary(_) => binary += 1,
                    DependentDensityFunction::ShiftedNoise(_) => shifted_noise += 1,
                    DependentDensityFunction::WeirdScaled(_) => weird_scaled += 1,
                    DependentDensityFunction::Clamp(_) => clamp += 1,
                    DependentDensityFunction::RangeChoice(_) => range_choice += 1,
                    DependentDensityFunction::Spline(_) => spline += 1,
                    DependentDensityFunction::FlattenedSpline(_) => flat_spline += 1,
                    DependentDensityFunction::FindTopSurface(_) => find_top += 1,
                },
            }
        }
        eprintln!("\n  Zone B breakdown:");
        if constants > 0 {
            eprintln!("    Constant:        {}", constants);
        }
        if old_blended > 0 {
            eprintln!("    OldBlendedNoise: {}", old_blended);
        }
        if noise > 0 {
            eprintln!("    Noise:           {}", noise);
        }
        if shift > 0 {
            eprintln!("    Shift:           {}", shift);
        }
        if clamped_y > 0 {
            eprintln!("    ClampedYGrad:    {}", clamped_y);
        }
        if linear > 0 {
            eprintln!("    Linear:          {}", linear);
        }
        if affine > 0 {
            eprintln!("    Affine:          {}", affine);
        }
        if piecewise > 0 {
            eprintln!("    PiecewiseAffine: {}", piecewise);
        }
        if slide > 0 {
            eprintln!("    Slide:           {}", slide);
        }
        if unary > 0 {
            eprintln!("    Unary:           {}", unary);
        }
        if binary > 0 {
            eprintln!("    Binary:          {}", binary);
        }
        if shifted_noise > 0 {
            eprintln!("    ShiftedNoise:    {}", shifted_noise);
        }
        if weird_scaled > 0 {
            eprintln!("    WeirdScaled:     {}", weird_scaled);
        }
        if clamp > 0 {
            eprintln!("    Clamp:           {}", clamp);
        }
        if range_choice > 0 {
            eprintln!("    RangeChoice:     {}", range_choice);
        }
        if spline > 0 {
            eprintln!("    Spline:          {}", spline);
        }
        if flat_spline > 0 {
            eprintln!("    FlattenedSpline: {}", flat_spline);
        }
        if find_top > 0 {
            eprintln!("    FindTopSurface:  {}", find_top);
        }
    }

    /// Create a new DensityCache for use with `final_density`.
    /// Reuse across calls within the same chunk generation.
    pub fn new_cache(&self) -> DensityCache {
        DensityCache {
            scratch: vec![0.0f32; self.stack.len()],
            last_x: i32::MIN,
            last_z: i32::MIN,
            column_valid: false,
        }
    }

    pub fn barrier_index(&self) -> usize {
        self.barrier_index
    }

    pub fn fluid_level_floodedness_index(&self) -> usize {
        self.fluid_level_floodedness_index
    }

    pub fn fluid_level_spread_index(&self) -> usize {
        self.fluid_level_spread_index
    }

    pub fn lava_index(&self) -> usize {
        self.lava_index
    }

    pub fn temperature_index(&self) -> usize {
        self.temperature_index
    }

    pub fn vegetation_index(&self) -> usize {
        self.vegetation_index
    }

    pub fn continents_index(&self) -> usize {
        self.continents_index
    }

    pub fn erosion_index(&self) -> usize {
        self.erosion_index
    }

    pub fn depth_index(&self) -> usize {
        self.depth_index
    }

    pub fn ridges_index(&self) -> usize {
        self.ridges_index
    }

    pub fn preliminary_surface_level_index(&self) -> usize {
        self.preliminary_surface_level_index
    }

    pub fn final_density_index(&self) -> usize {
        self.final_density_index
    }

    pub fn vein_toggle_index(&self) -> usize {
        self.vein_toggle_index
    }

    pub fn vein_ridged_index(&self) -> usize {
        self.vein_ridged_index
    }

    pub fn vein_gap_index(&self) -> usize {
        self.vein_gap_index
    }

    pub fn final_density(&self, pos: IVec3, cache: &mut DensityCache) -> f32 {
        self.evaluate_forward(self.final_density_index, pos, cache)
    }

    /// Evaluate final_density without caching (recursive, for validation/comparison).
    pub fn final_density_uncached(&self, pos: IVec3) -> f32 {
        DensityFunctionComponent::sample_from_stack(&self.stack[..=self.final_density_index], pos)
    }

    /// Verify that evaluate_forward (zone-based cached) matches the simple forward sweep
    /// at multiple positions. Tests both fresh-cache and column-reuse paths.
    /// Returns true if all checks pass.
    pub fn verify_evaluation(&self, positions: &[IVec3]) -> bool {
        let mut ok = true;

        // Test 1: Each position with a fresh cache
        for &pos in positions {
            let mut values = vec![0.0f32; self.final_density_index + 1];
            for i in 0..=self.final_density_index {
                values[i] = self.stack[i].sample_cached(&values, &self.stack, pos);
            }
            let expected = values[self.final_density_index];

            let mut cache = self.new_cache();
            let actual = self.final_density(pos, &mut cache);

            if (expected - actual).abs() > 1e-6 {
                eprintln!(
                    "MISMATCH (fresh) at ({},{},{}): expected={}, actual={}",
                    pos.x, pos.y, pos.z, expected, actual
                );
                ok = false;
            }
        }

        // Test 2: Column-reuse — same XZ, iterate Y with shared cache
        let mut cache = self.new_cache();
        for &base in positions.iter().take(4) {
            for y in (-64..=320).step_by(8) {
                let pos = IVec3::new(base.x, y, base.z);
                let mut values = vec![0.0f32; self.final_density_index + 1];
                for i in 0..=self.final_density_index {
                    values[i] = self.stack[i].sample_cached(&values, &self.stack, pos);
                }
                let expected = values[self.final_density_index];
                let actual = self.final_density(pos, &mut cache);

                if (expected - actual).abs() > 1e-6 {
                    eprintln!(
                        "MISMATCH (column) at ({},{},{}): expected={}, actual={}",
                        pos.x, pos.y, pos.z, expected, actual
                    );
                    ok = false;
                }
            }
        }

        ok
    }

    /// Create a new `ChunkColumnCache` for a 17x17 chunk column grid starting at block (base_block_x, base_block_z).
    /// The 17x17 grid covers local coordinates 0..=16 to include boundary corner positions.
    pub fn new_column_cache(&self, base_block_x: i32, base_block_z: i32) -> ChunkColumnCache {
        let grid_positions = (ChunkColumnCache::GRID_SIDE * ChunkColumnCache::GRID_SIDE) as usize;
        ChunkColumnCache {
            column_data: vec![0.0f32; grid_positions * self.column_boundary],
            zone_a_count: self.column_boundary,
            base_block_x,
            base_block_z,
            scratch: vec![0.0f32; self.stack.len()],
        }
    }

    /// Pre-populate Zone A values at cell corner positions in the chunk column grid.
    /// Only evaluates the (h_cells+1)^2 = 25 corner positions (step by h_cell_blocks),
    /// not every block position. This matches exactly the positions sampled by `fill_plane`.
    pub fn populate_columns(&self, cache: &mut ChunkColumnCache) {
        let zone_a_count = cache.zone_a_count;
        let grid_side = ChunkColumnCache::GRID_SIDE;
        let step = self.h_cell_blocks as i32;
        let corners = (16 / step) + 1; // h_cells + 1
        for cx in 0..corners {
            let local_x = cx * step;
            for cz in 0..corners {
                let local_z = cz * step;
                let y0_pos = IVec3::new(
                    cache.base_block_x + local_x,
                    0,
                    cache.base_block_z + local_z,
                );
                for i in 0..zone_a_count {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, y0_pos);
                }
                let xz_idx = (local_x * grid_side + local_z) as usize;
                let off = xz_idx * zone_a_count;
                cache.column_data[off..off + zone_a_count]
                    .copy_from_slice(&cache.scratch[..zone_a_count]);
            }
        }
    }

    /// Evaluate final_density using a pre-populated column cache.
    /// Zone A values must already be loaded into `cache.scratch` via `load_column`.
    /// Only evaluates Zone B entries (branchless, no column_changed check).
    #[inline]
    pub fn final_density_from_column_cache(&self, pos: IVec3, cache: &mut ChunkColumnCache) -> f32 {
        #[cfg(feature = "lazy-range-choice")]
        if let Some(rc) = &self.lazy_rc {
            // Phase 1: Evaluate the common prefix up to the RangeChoice input.
            for i in self.column_boundary..=rc.input_index {
                cache.scratch[i] = self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
            }

            // Phase 2: Check the RangeChoice condition and select the branch.
            let input_val = cache.scratch[rc.input_index];
            let in_range = input_val >= rc.min_inclusion && input_val < rc.max_exclusion;
            let branch = if in_range {
                &rc.branch_when_in
            } else {
                &rc.branch_when_out
            };

            // Phase 3: Evaluate only the needed branch entries.
            for &i in branch.iter() {
                cache.scratch[i] = self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
            }

            return cache.scratch[self.final_density_index];
        }

        for i in self.column_boundary..=self.final_density_index {
            cache.scratch[i] = self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
        }
        cache.scratch[self.final_density_index]
    }

    /// Verify that `final_density_from_column_cache` matches `final_density` for
    /// all cell corner XZ positions at the given Y values. Returns true if all checks pass.
    pub fn verify_column_cache(&self, base_x: i32, base_z: i32, y_values: &[i32]) -> bool {
        let mut ok = true;
        let mut column_cache = self.new_column_cache(base_x, base_z);
        self.populate_columns(&mut column_cache);

        let step = self.h_cell_blocks as i32;
        let corners = (16 / step) + 1;
        for cx in 0..corners {
            let local_x = cx * step;
            for cz in 0..corners {
                let local_z = cz * step;
                column_cache.load_column(local_x, local_z);
                for &y in y_values {
                    let pos = IVec3::new(base_x + local_x, y, base_z + local_z);

                    let actual = self.final_density_from_column_cache(pos, &mut column_cache);

                    // Reference: simple forward sweep
                    let mut values = vec![0.0f32; self.final_density_index + 1];
                    for i in 0..=self.final_density_index {
                        values[i] = self.stack[i].sample_cached(&values, &self.stack, pos);
                    }
                    let expected = values[self.final_density_index];

                    if (expected - actual).abs() > 1e-6 {
                        eprintln!(
                            "COLUMN CACHE MISMATCH at ({},{},{}): expected={}, actual={}",
                            pos.x, pos.y, pos.z, expected, actual
                        );
                        ok = false;
                    }
                }
                // Reload column since final_density_from_column_cache mutated scratch
                column_cache.load_column(local_x, local_z);
            }
        }
        ok
    }

    /// Generate a DOT graph of the density function computation tree
    /// rooted at `root`, evaluated at `pos`. Each node shows its type,
    /// optional reference name, and computed value.
    pub fn dump_dot_graph(&self, root_name: &str, root: usize, pos: IVec3) -> String {
        // Forward evaluate all entries up to root
        let mut values = vec![0.0f32; root + 1];
        for i in 0..=root {
            values[i] = self.stack[i].sample_cached(&values, &self.stack, pos);
        }

        // Find reachable nodes from root
        let mut reachable = vec![false; root + 1];
        reachable[root] = true;
        for i in (0..=root).rev() {
            if !reachable[i] {
                continue;
            }
            self.stack[i].visit_input_indices(&mut |idx| {
                if idx <= root {
                    reachable[idx] = true;
                }
            });
        }

        let mut dot = String::new();
        dot.push_str(&format!(
            "digraph \"{}\" {{\n",
            root_name.replace('"', "\\\"")
        ));
        dot.push_str("  rankdir=BT;\n");
        dot.push_str("  node [shape=box, style=filled, fontname=\"Helvetica\", fontsize=10];\n");
        dot.push_str(&format!(
            "  label=\"{} at ({}, {}, {})\";\n",
            root_name, pos.x, pos.y, pos.z
        ));
        dot.push_str("  labelloc=t;\n");

        // Add nodes
        for i in 0..=root {
            if !reachable[i] {
                continue;
            }
            let type_label = self.stack[i].type_label();
            let value = values[i];
            let ref_name = &self.node_labels[i];

            let color = if value > 0.5 {
                "#81c784"
            } else if value > 0.0 {
                "#c8e6c9"
            } else if value > -0.5 {
                "#ffcdd2"
            } else {
                "#ef9a9a"
            };

            let full_label = if ref_name.is_empty() {
                format!("[{}] {}\\nval={:.6}", i, type_label, value)
            } else {
                let short_name = ref_name.strip_prefix("minecraft:").unwrap_or(ref_name);
                format!("[{}] {}\\n{}\\nval={:.6}", i, short_name, type_label, value)
            };

            let shape = match &self.stack[i] {
                DensityFunctionComponent::Independent(_) => "ellipse",
                _ => "box",
            };

            let penwidth = if i == root { "3.0" } else { "1.0" };

            dot.push_str(&format!(
                "  n{} [label=\"{}\", fillcolor=\"{}\", shape={}, penwidth={}];\n",
                i, full_label, color, shape, penwidth
            ));
        }

        // Add edges
        for i in 0..=root {
            if !reachable[i] {
                continue;
            }
            self.stack[i].visit_input_indices(&mut |idx| {
                if idx <= root && reachable[idx] {
                    dot.push_str(&format!("  n{} -> n{};\n", idx, i));
                }
            });
        }

        dot.push_str("}\n");
        dot
    }

    /// Dump DOT graphs for all noise router entries.
    pub fn dump_all_roots_dot_graph(&self, pos: IVec3) -> Vec<(String, String)> {
        self.roots()
            .iter()
            .map(|(name, idx)| (name.to_string(), self.dump_dot_graph(name, *idx, pos)))
            .collect()
    }

    /// Colors for each noise router entry (cycled if more than palette size).
    const ROOT_COLORS: &[&str] = &[
        "#ef5350", // barrier - red
        "#42a5f5", // fluid_level_floodedness - blue
        "#66bb6a", // fluid_level_spread - green
        "#ffa726", // lava - orange
        "#ff7043", // temperature - deep orange
        "#9ccc65", // vegetation - light green
        "#ab47bc", // continents - purple
        "#ffca28", // erosion - amber
        "#26c6da", // depth - cyan
        "#ec407a", // ridges - pink
        "#8d6e63", // preliminary_surface_level - brown
        "#29b6f6", // final_density - light blue
        "#78909c", // vein_toggle - blue-grey
        "#7e57c2", // vein_ridged - deep purple
        "#26a69a", // vein_gap - teal
    ];

    /// Generate a single combined DOT graph showing all noise router entries,
    /// how they share nodes, and which entry functions feed into the router.
    pub fn dump_combined_dot_graph(&self, pos: IVec3) -> String {
        let all_roots = self.roots();
        let roots: Vec<(&str, usize, &str)> = all_roots
            .iter()
            .enumerate()
            .map(|(i, (name, idx))| (*name, *idx, Self::ROOT_COLORS[i % Self::ROOT_COLORS.len()]))
            .collect();

        // Find the maximum index needed
        let max_root = roots.iter().map(|(_, idx, _)| *idx).max().unwrap_or(0);

        // Forward evaluate all entries up to max root
        let mut values = vec![0.0f32; max_root + 1];
        for i in 0..=max_root {
            values[i] = self.stack[i].sample_cached(&values, &self.stack, pos);
        }

        // For each node, track which roots reach it
        let mut root_membership: Vec<Vec<usize>> = vec![Vec::new(); max_root + 1];
        for (root_idx, (_, root, _)) in roots.iter().enumerate() {
            let mut reachable = vec![false; *root + 1];
            reachable[*root] = true;
            for i in (0..=*root).rev() {
                if !reachable[i] {
                    continue;
                }
                self.stack[i].visit_input_indices(&mut |idx| {
                    if idx <= *root {
                        reachable[idx] = true;
                    }
                });
            }
            for i in 0..=*root {
                if reachable[i] {
                    root_membership[i].push(root_idx);
                }
            }
        }

        // Any reachable node (belongs to at least one root)
        let any_reachable: Vec<bool> = root_membership
            .iter()
            .map(|members| !members.is_empty())
            .collect();

        let mut dot = String::new();
        dot.push_str("digraph noise_router {\n");
        dot.push_str("  rankdir=BT;\n");
        dot.push_str("  compound=true;\n");
        dot.push_str("  node [shape=box, style=filled, fontname=\"Helvetica\", fontsize=10];\n");
        dot.push_str(&format!(
            "  label=\"Noise Router at ({}, {}, {})\";\n",
            pos.x, pos.y, pos.z
        ));
        dot.push_str("  labelloc=t;\n");
        dot.push_str("  fontsize=14;\n\n");

        // Add router entry nodes at the top
        dot.push_str("  // Router entry points\n");
        dot.push_str("  subgraph cluster_router {\n");
        dot.push_str("    label=\"Noise Router\";\n");
        dot.push_str("    style=dashed;\n");
        dot.push_str("    color=\"#666666\";\n");
        dot.push_str("    fontsize=12;\n");
        for &(name, idx, color) in &roots {
            dot.push_str(&format!(
                "    router_{} [label=\"{}\\nval={:.6}\", fillcolor=\"{}\", shape=doubleoctagon, penwidth=2.0, fontsize=11];\n",
                name, name, values[idx], color
            ));
        }
        dot.push_str("  }\n\n");

        // Connect router entry nodes to their root stack nodes
        for &(name, idx, color) in &roots {
            dot.push_str(&format!(
                "  n{} -> router_{} [color=\"{}\", penwidth=2.0];\n",
                idx, name, color
            ));
        }
        dot.push_str("\n");

        // Add computation nodes
        for i in 0..=max_root {
            if !any_reachable[i] {
                continue;
            }
            let type_label = self.stack[i].type_label();
            let value = values[i];
            let ref_name = &self.node_labels[i];
            let members = &root_membership[i];

            // Color: shared nodes get a special color, others get value-based color
            let is_shared = members.len() > 1;
            let is_root_node = roots.iter().any(|(_, idx, _)| *idx == i);

            let color = if is_root_node {
                // Root node gets its root color
                roots
                    .iter()
                    .find(|(_, idx, _)| *idx == i)
                    .map(|(_, _, c)| *c)
                    .unwrap_or("#ffffff")
                    .to_string()
            } else if is_shared {
                "#fff9c4".to_string() // Light yellow for shared nodes
            } else if value > 0.5 {
                "#81c784".to_string()
            } else if value > 0.0 {
                "#c8e6c9".to_string()
            } else if value > -0.5 {
                "#ffcdd2".to_string()
            } else {
                "#ef9a9a".to_string()
            };

            let full_label = if ref_name.is_empty() {
                format!("[{}] {}\\nval={:.6}", i, type_label, value)
            } else {
                let short_name = ref_name.strip_prefix("minecraft:").unwrap_or(ref_name);
                format!("[{}] {}\\n{}\\nval={:.6}", i, short_name, type_label, value)
            };

            // Append sharing info for shared nodes
            let full_label = if is_shared {
                let shared_with: Vec<&str> = members.iter().map(|&ri| roots[ri].0).collect();
                format!("{}\\nshared: {}", full_label, shared_with.join(", "))
            } else {
                full_label
            };

            let shape = match &self.stack[i] {
                DensityFunctionComponent::Independent(_) => "ellipse",
                _ => "box",
            };

            let penwidth = if is_root_node {
                "3.0"
            } else if is_shared {
                "2.0"
            } else {
                "1.0"
            };

            dot.push_str(&format!(
                "  n{} [label=\"{}\", fillcolor=\"{}\", shape={}, penwidth={}];\n",
                i, full_label, color, shape, penwidth
            ));
        }

        dot.push_str("\n");

        // Add edges
        for i in 0..=max_root {
            if !any_reachable[i] {
                continue;
            }
            self.stack[i].visit_input_indices(&mut |idx| {
                if idx <= max_root && any_reachable[idx] {
                    dot.push_str(&format!("  n{} -> n{};\n", idx, i));
                }
            });
        }

        dot.push_str("}\n");
        dot
    }

    /// Forward evaluation with column caching and zone-based dispatch.
    ///
    /// The stack is reordered into three zones:
    ///   Zone A `[0..column_boundary)`:  column-only entries for final_density
    ///   Zone B `[column_boundary..fd_boundary)`: per-Y entries for final_density
    ///   Zone C `[fd_boundary..n)`:               other roots (aquifer, veins, etc.)
    ///
    /// For Zone A roots (continents, erosion, ridges, etc.):
    ///   Only the column pass runs; the per-Y loop is empty.
    ///
    /// For Zone B roots (final_density and its per-Y dependencies):
    ///   Column pass evaluates Zone A at Y=0; per-Y pass sweeps Zone B branchlessly.
    ///
    /// For Zone C roots (barrier, temperature, veins, etc.):
    ///   Falls back to the general per_block-checking approach.
    fn evaluate_forward(&self, root: usize, pos: IVec3, cache: &mut DensityCache) -> f32 {
        let column_changed = pos.x != cache.last_x || pos.z != cache.last_z || !cache.column_valid;

        if root < self.column_boundary {
            // Zone A root: column-only (e.g., continents, erosion, ridges)
            if column_changed {
                cache.last_x = pos.x;
                cache.last_z = pos.z;
                let y0_pos = IVec3::new(pos.x, 0, pos.z);
                for i in 0..=root {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, y0_pos);
                }
                cache.column_valid = true;
            }
        } else if root < self.fd_boundary {
            // Zone B root: final_density path
            if column_changed {
                cache.last_x = pos.x;
                cache.last_z = pos.z;
                // Evaluate Zone A (column-only) entries at Y=0.
                // This includes FlatCache inputs evaluated at Y=0 (correct for column caching).
                let y0_pos = IVec3::new(pos.x, 0, pos.z);
                for i in 0..self.column_boundary {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, y0_pos);
                }
                cache.column_valid = true;
            }
            // Evaluate Zone B (per-Y) entries at actual position — branchless.
            // All entries in this range are per_block=true by construction.
            for i in self.column_boundary..=root {
                cache.scratch[i] = self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
            }
        } else {
            // Zone C root: fallback for aquifer, veins, temperature, etc.
            if column_changed {
                cache.last_x = pos.x;
                cache.last_z = pos.z;
                let y0_pos = IVec3::new(pos.x, 0, pos.z);
                for i in 0..=root {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, y0_pos);
                }
                cache.column_valid = true;
            }
            for i in 0..=root {
                if self.per_block[i] {
                    cache.scratch[i] =
                        self.stack[i].sample_cached(&cache.scratch, &self.stack, pos);
                }
            }
        }

        cache.scratch[root]
    }

    /// Create a new `SectionInterpolator` matching this router's cell dimensions.
    pub fn new_section_interpolator(&self) -> SectionInterpolator {
        SectionInterpolator::new(self.h_cell_blocks, self.v_cell_blocks)
    }
}

/// Trilinear interpolator for chunk section noise generation.
///
/// Samples `final_density` only at cell corners and trilinearly interpolates
/// interior block positions. With cell sizes of 4x8x4, this reduces expensive
/// density evaluations from 4,096 to 75 per 16x16x16 section.
pub struct SectionInterpolator {
    h_cell_blocks: usize,
    v_cell_blocks: usize,
    h_cells: usize,
    v_cells: usize,

    /// Y-Z plane of corner densities at the current X plane start.
    /// Indexed: `buf[(z_corner * (v_cells + 1)) + y_corner]`
    start_buf: Vec<f32>,
    /// Y-Z plane of corner densities at the current X plane end.
    end_buf: Vec<f32>,

    /// 8 cell corners after `on_sampled_cell_corners`
    corners: [f32; 8],
    /// 4 values after Y interpolation
    after_y: [f32; 4],
    /// 2 values after X interpolation
    after_x: [f32; 2],
    /// Final interpolated value
    val: f32,

    /// Saved top-Y density values for section-boundary reuse.
    /// Adjacent Y sections share cell corners at their boundary (the top Y-row
    /// of section s equals the bottom Y-row of section s+1). This buffer saves
    /// those values to avoid recomputing them.
    ///
    /// Indexed by `[plane_seq * z_count + z_corner]` where plane_seq is the
    /// sequential fill_plane call index within a section (0..=h_cells) and
    /// z_count = h_cells + 1.
    saved_top_y: Vec<f32>,
    /// True after the first section has been fully processed, meaning
    /// `saved_top_y` contains valid data for the next section.
    section_boundary_valid: bool,
}

impl SectionInterpolator {
    pub fn new(h_cell_blocks: usize, v_cell_blocks: usize) -> Self {
        let h_cells = 16 / h_cell_blocks;
        let v_cells = 16 / v_cell_blocks;
        let plane_size = (h_cells + 1) * (v_cells + 1);
        let z_count = h_cells + 1;
        let num_planes = h_cells + 1; // start plane + h_cells end planes
        Self {
            h_cell_blocks,
            v_cell_blocks,
            h_cells,
            v_cells,
            start_buf: vec![0.0f32; plane_size],
            end_buf: vec![0.0f32; plane_size],
            corners: [0.0f32; 8],
            after_y: [0.0f32; 4],
            after_x: [0.0f32; 2],
            val: 0.0f32,
            saved_top_y: vec![0.0f32; num_planes * z_count],
            section_boundary_valid: false,
        }
    }

    #[inline]
    pub fn h_cells(&self) -> usize {
        self.h_cells
    }

    #[inline]
    pub fn v_cells(&self) -> usize {
        self.v_cells
    }

    #[inline]
    pub fn h_cell_blocks(&self) -> usize {
        self.h_cell_blocks
    }

    #[inline]
    pub fn v_cell_blocks(&self) -> usize {
        self.v_cell_blocks
    }

    /// Evaluate `final_density` at all corner positions on a Y-Z plane for a given X,
    /// storing results into `start_buf` or `end_buf`.
    ///
    /// Uses `DensityCache` which handles Zone A column caching internally via
    /// the `column_changed` check. Corner positions can be at +16 (outside the
    /// 16x16 chunk), so `ChunkColumnCache` cannot be used here.
    pub fn fill_plane(
        &mut self,
        is_start: bool,
        x: i32,
        base_y: i32,
        base_z: i32,
        router: &NoiseRouter,
        cache: &mut DensityCache,
    ) {
        let buf = if is_start {
            &mut self.start_buf
        } else {
            &mut self.end_buf
        };
        let v_stride = self.v_cells + 1;
        for cz in 0..=self.h_cells {
            let z = base_z + (cz * self.h_cell_blocks) as i32;
            for cy in 0..=self.v_cells {
                let y = base_y + (cy * self.v_cell_blocks) as i32;
                let pos = IVec3::new(x, y, z);
                let density = router.final_density(pos, cache);
                buf[cz * v_stride + cy] = density;
            }
        }
    }

    /// Evaluate `final_density` at all corner positions on a Y-Z plane for a given X,
    /// using a pre-populated `ChunkColumnCache` instead of `DensityCache`.
    ///
    /// This eliminates the `column_changed` branch entirely — Zone A values are loaded
    /// from the cache, and only Zone B is evaluated per corner.
    pub fn fill_plane_cached(
        &mut self,
        is_start: bool,
        x: i32,
        base_y: i32,
        base_z: i32,
        router: &NoiseRouter,
        column_cache: &mut ChunkColumnCache,
    ) {
        let buf = if is_start {
            &mut self.start_buf
        } else {
            &mut self.end_buf
        };
        let v_stride = self.v_cells + 1;
        let local_x = x - column_cache.base_block_x;
        for cz in 0..=self.h_cells {
            let z = base_z + (cz * self.h_cell_blocks) as i32;
            let local_z = z - column_cache.base_block_z;
            column_cache.load_column(local_x, local_z);
            for cy in 0..=self.v_cells {
                let y = base_y + (cy * self.v_cell_blocks) as i32;
                let pos = IVec3::new(x, y, z);
                let density = router.final_density_from_column_cache(pos, column_cache);
                buf[cz * v_stride + cy] = density;
            }
        }
    }

    /// Like `fill_plane_cached`, but reuses the top-Y row from the previous section
    /// as the bottom-Y row of this section when `section_boundary_valid` is true.
    ///
    /// `plane_seq` identifies which X-plane is being filled (0 = start plane,
    /// 1..=h_cells = successive end planes). This index is used to look up the
    /// correct saved top-Y row from the previous section.
    ///
    /// After filling, the top-Y row (cy = v_cells) is saved for the next section.
    pub fn fill_plane_cached_reuse(
        &mut self,
        plane_seq: usize,
        is_start: bool,
        x: i32,
        base_y: i32,
        base_z: i32,
        router: &NoiseRouter,
        column_cache: &mut ChunkColumnCache,
    ) {
        let buf = if is_start {
            &mut self.start_buf
        } else {
            &mut self.end_buf
        };
        let v_stride = self.v_cells + 1;
        let local_x = x - column_cache.base_block_x;
        let z_count = self.h_cells + 1;
        let reuse = self.section_boundary_valid;

        for cz in 0..z_count {
            // Restore bottom-Y from saved top-Y of previous section
            if reuse {
                buf[cz * v_stride] = self.saved_top_y[plane_seq * z_count + cz];
            }

            let z = base_z + (cz * self.h_cell_blocks) as i32;
            let local_z = z - column_cache.base_block_z;
            column_cache.load_column(local_x, local_z);

            let cy_start = if reuse { 1 } else { 0 };
            for cy in cy_start..=self.v_cells {
                let y = base_y + (cy * self.v_cell_blocks) as i32;
                let pos = IVec3::new(x, y, z);
                let density = router.final_density_from_column_cache(pos, column_cache);
                buf[cz * v_stride + cy] = density;
            }

            // Save top-Y for next section
            self.saved_top_y[plane_seq * z_count + cz] = buf[cz * v_stride + self.v_cells];
        }
    }

    /// Mark the current section as complete, enabling Y-boundary reuse for the
    /// next section. Call this after all fill_plane calls and interpolation for
    /// a section are done.
    #[inline]
    pub fn end_section(&mut self) {
        self.section_boundary_valid = true;
    }

    /// Invalidate the Y-boundary cache, forcing the next section to compute
    /// all corner values from scratch. Must be called when the next section
    /// is not adjacent to the current one (i.e. there is a gap in Y sections).
    #[inline]
    pub fn reset_section_boundary(&mut self) {
        self.section_boundary_valid = false;
    }

    /// Load the 8 corner densities for a given cell from the start/end buffers.
    /// Corner layout:
    ///   corners[0] = start_buf[z][y]       (x0, y0, z0)
    ///   corners[1] = start_buf[z][y+1]     (x0, y1, z0)
    ///   corners[2] = start_buf[z+1][y]     (x0, y0, z1)
    ///   corners[3] = start_buf[z+1][y+1]   (x0, y1, z1)
    ///   corners[4] = end_buf[z][y]         (x1, y0, z0)
    ///   corners[5] = end_buf[z][y+1]       (x1, y1, z0)
    ///   corners[6] = end_buf[z+1][y]       (x1, y0, z1)
    ///   corners[7] = end_buf[z+1][y+1]     (x1, y1, z1)
    #[inline]
    pub fn on_sampled_cell_corners(&mut self, cell_y: usize, cell_z: usize) {
        let v_stride = self.v_cells + 1;
        let z0 = cell_z * v_stride;
        let z1 = (cell_z + 1) * v_stride;
        self.corners[0] = self.start_buf[z0 + cell_y];
        self.corners[1] = self.start_buf[z0 + cell_y + 1];
        self.corners[2] = self.start_buf[z1 + cell_y];
        self.corners[3] = self.start_buf[z1 + cell_y + 1];
        self.corners[4] = self.end_buf[z0 + cell_y];
        self.corners[5] = self.end_buf[z0 + cell_y + 1];
        self.corners[6] = self.end_buf[z1 + cell_y];
        self.corners[7] = self.end_buf[z1 + cell_y + 1];
    }

    /// Check if all 8 corner densities agree on sign.
    /// Returns `Some(true)` if all positive (solid), `Some(false)` if all <= 0 (air),
    /// or `None` if mixed (requires interpolation).
    #[inline]
    pub fn corners_uniform_sign(&self) -> Option<bool> {
        let c = &self.corners;
        if c[0] > 0.0
            && c[1] > 0.0
            && c[2] > 0.0
            && c[3] > 0.0
            && c[4] > 0.0
            && c[5] > 0.0
            && c[6] > 0.0
            && c[7] > 0.0
        {
            return Some(true);
        }
        if c[0] <= 0.0
            && c[1] <= 0.0
            && c[2] <= 0.0
            && c[3] <= 0.0
            && c[4] <= 0.0
            && c[5] <= 0.0
            && c[6] <= 0.0
            && c[7] <= 0.0
        {
            return Some(false);
        }
        None
    }

    /// Interpolate along Y: 8 corners → 4 values.
    /// `delta` = local_y / v_cell_blocks (0.0 at bottom of cell, 1.0 at top).
    #[inline]
    pub fn interpolate_y(&mut self, delta: f32) {
        self.after_y[0] = self.corners[0].lerp(self.corners[1], delta);
        self.after_y[1] = self.corners[2].lerp(self.corners[3], delta);
        self.after_y[2] = self.corners[4].lerp(self.corners[5], delta);
        self.after_y[3] = self.corners[6].lerp(self.corners[7], delta);
    }

    /// Interpolate along X: 4 values → 2 values.
    /// `delta` = local_x / h_cell_blocks.
    #[inline]
    pub fn interpolate_x(&mut self, delta: f32) {
        self.after_x[0] = self.after_y[0].lerp(self.after_y[2], delta);
        self.after_x[1] = self.after_y[1].lerp(self.after_y[3], delta);
    }

    /// Interpolate along Z: 2 values → 1 value.
    /// `delta` = local_z / h_cell_blocks.
    #[inline]
    pub fn interpolate_z(&mut self, delta: f32) {
        self.val = self.after_x[0].lerp(self.after_x[1], delta);
    }

    /// Swap start and end buffers (the current end becomes the next start).
    #[inline]
    pub fn swap_buffers(&mut self) {
        swap(&mut self.start_buf, &mut self.end_buf);
    }

    /// Get the final interpolated density value.
    #[inline]
    pub fn result(&self) -> f32 {
        self.val
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
    noise_name: String,
    sampler: NoiseSampler,
    xz_scale: f32,
    y_scale: f32,
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
    noise_name: String,
    sampler: NoiseSampler,
}

impl Debug for ShiftA {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftA")
            .field("noise_name", &self.noise_name)
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
    noise_name: String,
    sampler: NoiseSampler,
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
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        self.sampler
            .get(pos.z as f32 * 0.25, pos.x as f32 * 0.25, 0.0)
            * 4.0
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Shift {
    noise_name: String,
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
    EndIslands,
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
            IndependentDensityFunction::EndIslands => -0.84375,
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
            IndependentDensityFunction::EndIslands => 0.5625,
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
            IndependentDensityFunction::EndIslands => {
                // TODO: implement proper end islands noise sampling
                0.0
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum DependentDensityFunction {
    Linear(Linear),
    Affine(Affine),
    PiecewiseAffine(PiecewiseAffine),
    Slide(Slide),
    Unary(Unary),
    Binary(Binary),
    ShiftedNoise(ShiftedNoise),
    WeirdScaled(WeirdScaled),
    Clamp(Clamp),
    RangeChoice(RangeChoice),
    Spline(Spline),
    FlattenedSpline(FlattenedSpline),
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
            DependentDensityFunction::Affine(x) => {
                let _span =
                    info_span!("Affine::sample", scale = x.scale, offset = x.offset).entered();
                x.sample(stack, pos)
            }
            DependentDensityFunction::PiecewiseAffine(x) => x.sample(stack, pos),
            DependentDensityFunction::Slide(x) => x.sample(stack, pos),
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
            DependentDensityFunction::FlattenedSpline(x) => x.sample(stack, pos),
            DependentDensityFunction::FindTopSurface(x) => x.sample(stack, pos),
        }
    }
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
            DependentDensityFunction::WeirdScaled(x) => x.min_value(),
            DependentDensityFunction::Clamp(x) => x.min_value(),
            DependentDensityFunction::RangeChoice(x) => x.min_value(),
            DependentDensityFunction::Spline(x) => x.min_value(),
            DependentDensityFunction::FlattenedSpline(x) => x.min_value(),
            DependentDensityFunction::FindTopSurface(x) => x.min_value(),
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
            DependentDensityFunction::WeirdScaled(x) => x.max_value(),
            DependentDensityFunction::Clamp(x) => x.max_value(),
            DependentDensityFunction::RangeChoice(x) => x.max_value(),
            DependentDensityFunction::Spline(x) => x.max_value(),
            DependentDensityFunction::FlattenedSpline(x) => x.max_value(),
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

#[derive(Clone, Debug, PartialEq)]
struct Affine {
    input_index: usize,
    scale: f32,
    offset: f32,
    min_value: f32,
    max_value: f32,
}

#[derive(Clone, Debug, PartialEq, Copy, Eq)]
enum LinearOperation {
    Add,
    Multiply,
}

impl Affine {
    fn compute_range(input_min: f32, input_max: f32, scale: f32, offset: f32) -> (f32, f32) {
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

impl DensityFunction for Affine {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let density = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        density.mul_add(self.scale, self.offset)
    }
}

/// Piecewise-linear affine: different scales for negative vs non-negative input.
///
/// Replaces patterns like `Affine(Unary::QuarterNegative(x))` or
/// `Affine(Unary::HalfNegative(x))` where the unary damps the negative side.
///
/// Computes: `if x < 0 { x * neg_scale + offset } else { x * pos_scale + offset }`
#[derive(Clone, Debug, PartialEq)]
struct PiecewiseAffine {
    input_index: usize,
    neg_scale: f32,
    pos_scale: f32,
    offset: f32,
    min_value: f32,
    max_value: f32,
}

impl PiecewiseAffine {
    fn compute_range(
        input_min: f32,
        input_max: f32,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
    ) -> (f32, f32) {
        // Negative side: input_min * neg_scale + offset (input_min <= 0)
        // Positive side: input_max * pos_scale + offset (input_max >= 0)
        // At zero: offset
        let mut lo = offset;
        let mut hi = offset;
        if input_min < 0.0 {
            let v = input_min * neg_scale + offset;
            lo = lo.min(v);
            hi = hi.max(v);
        }
        if input_max > 0.0 {
            let v = input_max * pos_scale + offset;
            lo = lo.min(v);
            hi = hi.max(v);
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

impl DensityFunction for PiecewiseAffine {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let x = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        let scale = if x < 0.0 {
            self.neg_scale
        } else {
            self.pos_scale
        };
        x.mul_add(scale, self.offset)
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
struct Slide {
    input_index: usize,

    // First Y-gradient applied (typically "top": 240..256 → 1.0..0.0)
    grad1: ClampedYGradient,
    // Second Y-gradient applied (typically "bottom": -64..-40 → 0.0..1.0)
    grad2: ClampedYGradient,

    // Three affine offsets (all original affines had scale=1.0)
    offset_a: f32, // pre-grad1
    offset_b: f32, // between grad1 and grad2
    offset_c: f32, // post-grad2

    // Pre-computed: offset_a + offset_b + offset_c
    combined_offset: f32,

    // Y range where both gradients saturate to 1.0 (fast path)
    fast_path_min_y: f32,
    fast_path_max_y: f32,

    min_value: f32,
    max_value: f32,
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
    fn saturate_one_range(g: &ClampedYGradient) -> Option<(f32, f32)> {
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

impl DensityFunction for Slide {
    #[inline]
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let input = DensityFunctionComponent::sample_from_stack(&stack[..=self.input_index], pos);
        self.compute(input, pos.y as f32)
    }
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
    noise_name: String,
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
    noise_name: String,
    input_index: usize,
    sampler: NoiseSampler,
    mapper: RarityValueMapper,
}

impl Debug for WeirdScaled {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeirdScaled")
            .field("noise_name", &self.noise_name)
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
        amp * self
            .sampler
            .get(
                pos.x as f32 * coord_mul,
                pos.y as f32 * coord_mul,
                pos.z as f32 * coord_mul,
            )
            .abs()
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

impl SplineValue {
    #[inline]
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            SplineValue::Spline(x) => x.sample_cached(cache, stack, pos),
            SplineValue::Constant(x) => *x,
        }
    }
}

impl Spline {
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let location = cache[self.input_index];

        let locs = &self.locations;
        let idx_gt = Self::upper_bound(locs, location);
        let n_points = locs.len();

        if idx_gt == 0 {
            let v0 = self.values[0].sample_cached(cache, stack, pos);
            let d0 = self.derivatives[0];
            return if d0 == 0.0 {
                v0
            } else {
                d0.mul_add(location - locs[0], v0)
            };
        }

        if idx_gt == n_points {
            let i = n_points - 1;
            let v = self.values[i].sample_cached(cache, stack, pos);
            let d = self.derivatives[i];
            return if d == 0.0 {
                v
            } else {
                d.mul_add(location - locs[i], v)
            };
        }

        let i0 = idx_gt - 1;
        let i1 = idx_gt;

        let v0 = self.values[i0].sample_cached(cache, stack, pos);
        let v1 = self.values[i1].sample_cached(cache, stack, pos);

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

#[derive(Clone, Debug)]
struct FlattenedSpline {
    coord_indices: [usize; 3],
    coord_min: [f32; 3],
    coord_inv_range: [f32; 3],
    grid_sizes: [usize; 3],
    strides: [usize; 3],
    lut: Box<[f32]>,
    min_value: f32,
    max_value: f32,
}

impl PartialEq for FlattenedSpline {
    fn eq(&self, other: &Self) -> bool {
        self.coord_indices == other.coord_indices
            && self.coord_min == other.coord_min
            && self.coord_inv_range == other.coord_inv_range
            && self.grid_sizes == other.grid_sizes
            && self.min_value == other.min_value
            && self.max_value == other.max_value
    }
}

impl FlattenedSpline {
    /// Monotone cubic Hermite interpolation between p1 and p2.
    /// Uses p0 and p3 as neighbors for derivative estimation with
    /// Fritsch-Carlson monotonicity correction to prevent overshoot.
    #[inline(always)]
    fn monotone_cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
        let delta = p2 - p1;

        // Estimate derivatives via central differences
        let mut d1 = (p2 - p0) * 0.5;
        let mut d2 = (p3 - p1) * 0.5;

        // Fritsch-Carlson monotonicity: if delta is ~0, zero both derivatives
        if delta.abs() < 1e-10 {
            d1 = 0.0;
            d2 = 0.0;
        } else {
            // Clamp derivatives to prevent overshoot
            let alpha = d1 / delta;
            let beta = d2 / delta;
            // If derivatives point the wrong way, zero them
            if alpha < 0.0 {
                d1 = 0.0;
            }
            if beta < 0.0 {
                d2 = 0.0;
            }
            // Fritsch-Carlson: constrain to circle of radius 3
            let r2 = alpha * alpha + beta * beta;
            if r2 > 9.0 {
                let s = 3.0 / r2.sqrt();
                d1 = s * alpha * delta;
                d2 = s * beta * delta;
            }
        }

        // Hermite basis evaluation
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;

        h00 * p1 + h10 * d1 + h01 * p2 + h11 * d2
    }

    /// Tricubic monotone Hermite interpolation with per-axis grid sizes.
    #[inline]
    fn evaluate(&self, c0: f32, c1: f32, c2: f32) -> f32 {
        let coords = [c0, c1, c2];
        let mut f = [0.0f32; 3];
        let mut idx = [0usize; 3];
        let mut t = [0.0f32; 3];
        let mut neighbors = [[0usize; 4]; 3]; // [i-1, i, i+1, i+2] per axis

        for d in 0..3 {
            let gs = self.grid_sizes[d];
            let max_idx = (gs - 1) as f32;
            f[d] = ((coords[d] - self.coord_min[d]) * self.coord_inv_range[d]).clamp(0.0, 1.0)
                * max_idx;
            idx[d] = (f[d] as usize).min(gs - 2);
            t[d] = f[d] - idx[d] as f32;
            let last = gs - 1;
            neighbors[d] = [
                idx[d].saturating_sub(1),
                idx[d],
                (idx[d] + 1).min(last),
                (idx[d] + 2).min(last),
            ];
        }

        // Tricubic: interpolate along axis 2, then 1, then 0
        let mut x_vals = [0.0f32; 4];
        for (a, &xi) in neighbors[0].iter().enumerate() {
            let mut y_vals = [0.0f32; 4];
            for (b, &yi) in neighbors[1].iter().enumerate() {
                let base = xi * self.strides[0] + yi * self.strides[1];
                y_vals[b] = Self::monotone_cubic(
                    self.lut[base + neighbors[2][0]],
                    self.lut[base + neighbors[2][1]],
                    self.lut[base + neighbors[2][2]],
                    self.lut[base + neighbors[2][3]],
                    t[2],
                );
            }
            x_vals[a] = Self::monotone_cubic(y_vals[0], y_vals[1], y_vals[2], y_vals[3], t[1]);
        }
        Self::monotone_cubic(x_vals[0], x_vals[1], x_vals[2], x_vals[3], t[0])
    }
}

impl RangeFunction for FlattenedSpline {
    #[inline]
    fn min_value(&self) -> f32 {
        self.min_value
    }

    #[inline]
    fn max_value(&self) -> f32 {
        self.max_value
    }
}

impl DensityFunction for FlattenedSpline {
    fn sample(&self, stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        let c0 = DensityFunctionComponent::sample_from_stack(&stack[..=self.coord_indices[0]], pos);
        let c1 = DensityFunctionComponent::sample_from_stack(&stack[..=self.coord_indices[1]], pos);
        let c2 = DensityFunctionComponent::sample_from_stack(&stack[..=self.coord_indices[2]], pos);
        self.evaluate(c0, c1, c2)
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
        // let input2_density =
        //     DensityFunctionComponent::sample_from_stack(&stack[..=self.input2_index], pos);
        // println!("binary {:?} d1={} d2={}", self.operation, input1_density, input2_density);
        // println!(
        //     "binary {:?} d={:?} arg1.min={:?} arg1.max={:?}",
        //     self.operation,
        //     input1_density,
        //     stack[self.input1_index].min_value(),
        //     stack[self.input1_index].max_value()
        // );
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
    fn type_label(&self) -> String {
        match self {
            DensityFunctionComponent::Independent(f) => match f {
                IndependentDensityFunction::Constant(v) => format!("const({v:.6})"),
                IndependentDensityFunction::OldBlendedNoise(_) => "old_blended_noise".into(),
                IndependentDensityFunction::Noise(n) => {
                    format!(
                        "noise({})\\nxz={:.3} y={:.3}",
                        n.noise_name, n.xz_scale, n.y_scale
                    )
                }
                IndependentDensityFunction::ShiftA(s) => {
                    format!("shift_a({})", s.noise_name)
                }
                IndependentDensityFunction::ShiftB(s) => {
                    format!("shift_b({})", s.noise_name)
                }
                IndependentDensityFunction::Shift(s) => {
                    format!("shift({})", s.noise_name)
                }
                IndependentDensityFunction::ClampedYGradient(g) => {
                    format!(
                        "y_gradient\\n{:.0}..{:.0} -> {:.2}..{:.2}",
                        g.from_y, g.to_y, g.from_value, g.to_value
                    )
                }
                IndependentDensityFunction::EndIslands => "end_islands".into(),
            },
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::Linear(l) => match l.operation {
                    LinearOperation::Add => format!("add({:.6})", l.argument),
                    LinearOperation::Multiply => format!("mul({:.6})", l.argument),
                },
                DependentDensityFunction::Affine(a) => {
                    format!("affine\\ns={:.6} o={:.6}", a.scale, a.offset)
                }
                DependentDensityFunction::PiecewiseAffine(a) => {
                    format!(
                        "piecewise_affine\\nneg={:.6} pos={:.6} o={:.6}",
                        a.neg_scale, a.pos_scale, a.offset
                    )
                }
                DependentDensityFunction::Slide(s) => {
                    format!(
                        "slide\\noffsets={:.6},{:.6},{:.6}\\nfast_y={:.0}..{:.0}",
                        s.offset_a, s.offset_b, s.offset_c, s.fast_path_min_y, s.fast_path_max_y
                    )
                }
                DependentDensityFunction::Unary(u) => format!("{:?}", u.operation),
                DependentDensityFunction::Binary(b) => match b.operation {
                    BinaryOperation::Add => "Add".into(),
                    BinaryOperation::Multiply => "Mul".into(),
                    BinaryOperation::Min => "Min".into(),
                    BinaryOperation::Max => "Max".into(),
                },
                DependentDensityFunction::ShiftedNoise(s) => {
                    format!(
                        "shifted_noise({})\\nxz={:.3} y={:.3}",
                        s.noise_name, s.xz_scale, s.y_scale
                    )
                }
                DependentDensityFunction::WeirdScaled(w) => {
                    format!("weird_scaled({})\\n{:?}", w.noise_name, w.mapper)
                }
                DependentDensityFunction::Clamp(c) => {
                    format!("clamp\\n{:.4}..{:.4}", c.min_value, c.max_value)
                }
                DependentDensityFunction::RangeChoice(r) => {
                    format!(
                        "range_choice\\n{:.4}..{:.4}",
                        r.min_inclusion_value, r.max_exclusion_value
                    )
                }
                DependentDensityFunction::Spline(_) => "spline".into(),
                DependentDensityFunction::FlattenedSpline(f) => {
                    format!(
                        "flattened_spline\\ngrid={}x{}x{}",
                        f.grid_sizes[0], f.grid_sizes[1], f.grid_sizes[2]
                    )
                }
                DependentDensityFunction::FindTopSurface(_) => "find_top_surface".into(),
            },
            DensityFunctionComponent::Wrapper(f) => match f {
                WrapperDensityFunction::BlendDensity(_) => "blend_density".into(),
                WrapperDensityFunction::Interpolated(_) => "interpolated".into(),
                WrapperDensityFunction::FlatCache(_) => "flat_cache".into(),
                WrapperDensityFunction::Cache2d(_) => "cache_2d".into(),
                WrapperDensityFunction::CacheOnce(_) => "cache_once".into(),
                WrapperDensityFunction::CacheAllInCell(_) => "cache_all_in_cell".into(),
            },
        }
    }

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

impl SplineValue {
    fn rewrite_indices(&mut self, redirect: &[usize]) {
        if let SplineValue::Spline(spline) = self {
            spline.rewrite_indices(redirect);
        }
    }
}

impl Spline {
    fn rewrite_indices(&mut self, redirect: &[usize]) {
        self.input_index = redirect[self.input_index];
        for value in self.values.iter_mut() {
            value.rewrite_indices(redirect);
        }
    }

    fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
        f(self.input_index);
        for value in self.values.iter() {
            if let SplineValue::Spline(nested) = value {
                nested.visit_input_indices(f);
            }
        }
    }
}

impl DensityFunctionComponent {
    fn rewrite_indices(&mut self, redirect: &[usize]) {
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
                DependentDensityFunction::WeirdScaled(x) => {
                    x.input_index = redirect[x.input_index];
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
                DependentDensityFunction::FlattenedSpline(x) => {
                    for idx in x.coord_indices.iter_mut() {
                        *idx = redirect[*idx];
                    }
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    x.density_index = redirect[x.density_index];
                    x.upper_bound_index = redirect[x.upper_bound_index];
                }
            },
            DensityFunctionComponent::Wrapper(wrapper) => match wrapper {
                WrapperDensityFunction::BlendDensity(x) => {
                    x.input_index = redirect[x.input_index];
                }
                WrapperDensityFunction::Interpolated(x) => {
                    x.input_index = redirect[x.input_index];
                }
                WrapperDensityFunction::FlatCache(x) => {
                    x.input_index = redirect[x.input_index];
                }
                WrapperDensityFunction::Cache2d(x) => {
                    x.input_index = redirect[x.input_index];
                }
                WrapperDensityFunction::CacheOnce(x) => {
                    x.input_index = redirect[x.input_index];
                }
                WrapperDensityFunction::CacheAllInCell(x) => {
                    x.input_index = redirect[x.input_index];
                }
            },
        }
    }

    fn visit_input_indices(&self, f: &mut impl FnMut(usize)) {
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
                DependentDensityFunction::WeirdScaled(x) => f(x.input_index),
                DependentDensityFunction::Clamp(x) => f(x.input_index),
                DependentDensityFunction::RangeChoice(x) => {
                    f(x.input_index);
                    f(x.when_in_index);
                    f(x.when_out_index);
                }
                DependentDensityFunction::Spline(x) => x.visit_input_indices(f),
                DependentDensityFunction::FlattenedSpline(x) => {
                    for &idx in &x.coord_indices {
                        f(idx);
                    }
                }
                DependentDensityFunction::FindTopSurface(x) => {
                    f(x.density_index);
                    f(x.upper_bound_index);
                }
            },
            DensityFunctionComponent::Wrapper(wrapper) => match wrapper {
                WrapperDensityFunction::BlendDensity(x) => f(x.input_index),
                WrapperDensityFunction::Interpolated(x) => f(x.input_index),
                WrapperDensityFunction::FlatCache(x) => f(x.input_index),
                WrapperDensityFunction::Cache2d(x) => f(x.input_index),
                WrapperDensityFunction::CacheOnce(x) => f(x.input_index),
                WrapperDensityFunction::CacheAllInCell(x) => f(x.input_index),
            },
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

    fn debug(stack: &[DensityFunctionComponent], index: usize, pos: IVec3) -> f32 {
        Self::sample_from_stack(&stack[..=index], pos)
    }

    /// Evaluate using pre-computed cache (forward evaluation).
    /// All entries at indices < this entry's position are already computed in `cache`.
    /// No tracing spans — this is the optimized hot path.
    #[inline]
    fn sample_cached(&self, cache: &[f32], stack: &[DensityFunctionComponent], pos: IVec3) -> f32 {
        match self {
            DensityFunctionComponent::Independent(f) => match f {
                IndependentDensityFunction::Constant(x) => *x,
                IndependentDensityFunction::OldBlendedNoise(x) => x.sample(&[], pos),
                IndependentDensityFunction::Noise(x) => x.sample(&[], pos),
                IndependentDensityFunction::ShiftA(x) => x.sample(&[], pos),
                IndependentDensityFunction::ShiftB(x) => x.sample(&[], pos),
                IndependentDensityFunction::Shift(x) => x.sample(&[], pos),
                IndependentDensityFunction::ClampedYGradient(x) => x.sample(&[], pos),
                IndependentDensityFunction::EndIslands => 0.0,
            },
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::Linear(x) => {
                    let input = cache[x.input_index];
                    match x.operation {
                        LinearOperation::Add => input + x.argument,
                        LinearOperation::Multiply => input * x.argument,
                    }
                }
                DependentDensityFunction::Affine(x) => {
                    cache[x.input_index].mul_add(x.scale, x.offset)
                }
                DependentDensityFunction::PiecewiseAffine(x) => {
                    let input = cache[x.input_index];
                    let scale = if input < 0.0 {
                        x.neg_scale
                    } else {
                        x.pos_scale
                    };
                    input.mul_add(scale, x.offset)
                }
                DependentDensityFunction::Slide(x) => x.compute(cache[x.input_index], pos.y as f32),
                DependentDensityFunction::Unary(x) => x.operation.apply(cache[x.input_index]),
                DependentDensityFunction::Binary(x) => {
                    let a = cache[x.input1_index];
                    let b = cache[x.input2_index];
                    match x.operation {
                        BinaryOperation::Add => a + b,
                        BinaryOperation::Multiply => a * b,
                        BinaryOperation::Min => a.min(b),
                        BinaryOperation::Max => a.max(b),
                    }
                }
                DependentDensityFunction::ShiftedNoise(x) => x.sampler.get(
                    pos.x as f32 * x.xz_scale + cache[x.input_x_index],
                    pos.y as f32 * x.y_scale + cache[x.input_y_index],
                    pos.z as f32 * x.xz_scale + cache[x.input_z_index],
                ),
                DependentDensityFunction::WeirdScaled(x) => {
                    let density = cache[x.input_index];
                    let (amp, coord_mul) = match x.mapper {
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
                    amp * x
                        .sampler
                        .get(
                            pos.x as f32 * coord_mul,
                            pos.y as f32 * coord_mul,
                            pos.z as f32 * coord_mul,
                        )
                        .abs()
                }
                DependentDensityFunction::Clamp(x) => {
                    cache[x.input_index].clamp(x.min_value, x.max_value)
                }
                DependentDensityFunction::RangeChoice(x) => {
                    let input = cache[x.input_index];
                    if input >= x.min_inclusion_value && input < x.max_exclusion_value {
                        cache[x.when_in_index]
                    } else {
                        cache[x.when_out_index]
                    }
                }
                DependentDensityFunction::Spline(x) => x.sample_cached(cache, stack, pos),
                DependentDensityFunction::FlattenedSpline(x) => x.evaluate(
                    cache[x.coord_indices[0]],
                    cache[x.coord_indices[1]],
                    cache[x.coord_indices[2]],
                ),
                DependentDensityFunction::FindTopSurface(x) => {
                    let top_y =
                        (cache[x.upper_bound_index] / x.cell_height).floor() * x.cell_height;
                    if top_y <= x.lower_bound {
                        x.lower_bound
                    } else {
                        // Must evaluate density at different Y positions — fall back to recursive
                        let mut current_y = top_y;
                        loop {
                            let sample_pos = IVec3::new(pos.x, current_y as i32, pos.z);
                            let density = DensityFunctionComponent::sample_from_stack(
                                &stack[..=x.density_index],
                                sample_pos,
                            );
                            if density > 0.0 || current_y <= x.lower_bound {
                                return current_y;
                            }
                            current_y -= x.cell_height;
                        }
                    }
                }
            },
            DensityFunctionComponent::Wrapper(f) => match f {
                WrapperDensityFunction::BlendDensity(x) => cache[x.input_index],
                WrapperDensityFunction::Interpolated(x) => cache[x.input_index],
                WrapperDensityFunction::FlatCache(x) => cache[x.input_index],
                WrapperDensityFunction::Cache2d(x) => cache[x.input_index],
                WrapperDensityFunction::CacheOnce(x) => cache[x.input_index],
                WrapperDensityFunction::CacheAllInCell(x) => cache[x.input_index],
            },
        }
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
        let noise_name = Self::noise_name(noise_holder);
        let sampler = self.noise_sampler(noise_holder);
        let proto = ProtoDensityFunction::Noise {
            noise: noise_holder.clone(),
            xz_scale: xz_scale.into(),
            y_scale: y_scale.into(),
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Independent(IndependentDensityFunction::Noise(Noise {
                noise_name,
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
        let noise_name = Self::noise_name(noise);
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
                    noise_name,
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
        let noise_name = Self::noise_name(noise);
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
                    noise_name,
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
        let noise_name = Self::noise_name(function);
        let sampler = self.noise_sampler(function);
        self.register_component(
            ProtoDensityFunction::ShiftA {
                argument: function.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::ShiftA(ShiftA {
                noise_name,
                sampler,
            })),
        );
    }

    fn visit_shift_b(&mut self, function: &NoiseHolder) {
        let noise_name = Self::noise_name(function);
        let sampler = self.noise_sampler(function);
        self.register_component(
            ProtoDensityFunction::ShiftB {
                argument: function.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::ShiftB(ShiftB {
                noise_name,
                sampler,
            })),
        );
    }

    fn visit_shift(&mut self, argument: &NoiseHolder) {
        let noise_name = Self::noise_name(argument);
        let sampler = self.noise_sampler(argument);
        self.register_component(
            ProtoDensityFunction::Shift {
                argument: argument.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::Shift(Shift {
                noise_name,
                sampler,
            })),
        );
    }

    fn visit_end_islands(&mut self) {
        self.register_component(
            ProtoDensityFunction::EndIslands,
            DensityFunctionComponent::Independent(IndependentDensityFunction::EndIslands),
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

    fn noise_name(holder: &NoiseHolder) -> String {
        match holder {
            NoiseHolder::Reference(x) => x
                .as_str()
                .strip_prefix("minecraft:")
                .unwrap_or(x.as_str())
                .to_string(),
            NoiseHolder::Owned(_) => "inline".to_string(),
        }
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
