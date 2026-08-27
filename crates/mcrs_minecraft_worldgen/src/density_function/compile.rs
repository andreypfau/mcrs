use super::*;

pub(super) struct ChunkNoiseFunctionBuilderOptions {
    // Number of blocks per cell per axis
    horizontal_cell_block_count: usize,
    vertical_cell_block_count: usize,

    // The biome coords of this chunk
    pub start_biome_x: i32,
    pub start_biome_z: i32,

    // Number of biome regions per chunk per axis
    pub horizontal_biome_end: usize,
}

fn constant_value(holder: &DensityFunctionHolder) -> Option<f32> {
    match holder {
        DensityFunctionHolder::Value(v) => Some(v.value.0 as f32),
        DensityFunctionHolder::Owned(f) => match &**f {
            ProtoDensityFunction::Constant(v) => Some(v.value.0 as f32),
            _ => None,
        },
        DensityFunctionHolder::Reference(_) => None,
    }
}

fn lower_constant_exponent(
    base: &DensityFunctionHolder,
    exponent: f32,
) -> Option<DensityFunctionHolder> {
    let owned = |function| DensityFunctionHolder::Owned(Box::new(function));
    let argument = SingleArgumentFunction {
        input: base.clone(),
    };
    let magnitude = exponent.abs();
    let lowered = if magnitude == 0.5 {
        owned(ProtoDensityFunction::Sqrt(argument))
    } else if magnitude == 1.0 {
        base.clone()
    } else if magnitude == 2.0 {
        owned(ProtoDensityFunction::Square(argument))
    } else if magnitude == 3.0 {
        owned(ProtoDensityFunction::Cube(argument))
    } else {
        return None;
    };
    Some(if exponent < 0.0 {
        owned(ProtoDensityFunction::Reciprocal(SingleArgumentFunction {
            input: lowered,
        }))
    } else {
        lowered
    })
}

fn lerp_range(alpha: (f32, f32), first: (f32, f32), second: (f32, f32)) -> (f32, f32) {
    let scaled = |alpha: f32, delta: f32| {
        if alpha == 0.0 || delta == 0.0 {
            0.0
        } else {
            alpha * delta
        }
    };
    let mut min_value = f32::INFINITY;
    let mut max_value = f32::NEG_INFINITY;
    for a in [alpha.0, alpha.1] {
        for f in [first.0, first.1] {
            for s in [second.0, second.1] {
                let bound = f + scaled(a, s - f);
                // Opposing infinities cancel to NaN, which no finite pair of bounds
                // describes; only the widest interval stays sound.
                if bound.is_nan() {
                    return (f32::NEG_INFINITY, f32::INFINITY);
                }
                min_value = min_value.min(bound);
                max_value = max_value.max(bound);
            }
        }
    }
    (min_value, max_value)
}

/// Check if a Binary::Multiply has one ClampedYGradient input.
/// Returns (gradient, other_input_index) if found.
pub(super) fn extract_mul_y_grad(
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
pub(super) fn try_build_slide(idx: usize, stack: &[DensityFunctionComponent]) -> Option<Slide> {
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

pub(super) fn optimize_stack(stack: &mut Vec<DensityFunctionComponent>, roots: &mut [usize]) {
    let n = stack.len();
    if n == 0 {
        return;
    }

    let mut redirect: Vec<usize> = (0..n).collect();
    let mut piecewise_affine_fusions = 0usize;
    let mut constants_folded = 0usize;
    let mut identities_eliminated = 0usize;
    let mut binary_demotions = 0usize;
    let mut slide_fusions = 0usize;

    for i in 0..n {
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
                    let result = op.apply(a, b);
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

        // 5. Convert standalone Linear to Affine
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
                    BinaryOperation::Subtract
                    | BinaryOperation::Divide
                    | BinaryOperation::Pow
                    | BinaryOperation::Round(_) => {}
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

    info!(
        stack_size = n,
        piecewise_affine_fusions,
        constants_folded,
        identities_eliminated,
        binary_demotions,
        slide_fusions,
        "Density function stack optimized"
    );
}

/// Which stack entries have to be recomputed as Y changes, and which hold for a
/// whole column.
pub(super) fn compute_per_block(stack: &[DensityFunctionComponent]) -> Vec<bool> {
    compute_domain_axes(stack)
        .into_iter()
        .map(|axes| axes & AXIS_Y != 0)
        .collect()
}

/// Whether the column pass can evaluate an entry: its own value holds for the
/// whole column and so does every value it reads. `slice` and `find_top_surface`
/// are the two nodes that drop an axis their input still varies along, so they
/// are also the only ones that stay per-Y here despite a column-wide value.
pub(super) fn compute_column_ready(stack: &[DensityFunctionComponent]) -> Vec<bool> {
    let axes = compute_domain_axes(stack);
    let mut ready = vec![false; stack.len()];
    for i in 0..stack.len() {
        let mut i_ready = axes[i] & AXIS_Y == 0;
        stack[i].visit_input_indices(&mut |j| i_ready &= ready[j]);
        ready[i] = i_ready;
    }
    ready
}

/// The coordinate axes a stack entry's value can vary along. Every axis outside
/// the mask may be replaced by any value without changing the result.
pub(super) fn compute_domain_axes(stack: &[DensityFunctionComponent]) -> Vec<u8> {
    let mut axes = vec![0u8; stack.len()];

    for i in 0..stack.len() {
        let mut inputs = 0u8;
        stack[i].visit_input_indices(&mut |j| {
            debug_assert!(j < i, "stack entry {i} reads later entry {j}");
            inputs |= axes[j];
        });

        axes[i] = match &stack[i] {
            DensityFunctionComponent::Independent(f) => match f {
                IndependentDensityFunction::Constant(_) => 0,
                IndependentDensityFunction::OldBlendedNoise(_)
                | IndependentDensityFunction::Shift(_)
                | IndependentDensityFunction::DistanceToPoint(_) => ALL_AXES,
                IndependentDensityFunction::Noise(n) => noise_scale_axes(n.xz_scale, n.y_scale),
                IndependentDensityFunction::ShiftA(_)
                | IndependentDensityFunction::ShiftB(_)
                | IndependentDensityFunction::EndOuterIslands(_) => AXIS_X | AXIS_Z,
                IndependentDensityFunction::ClampedYGradient(_) => AXIS_Y,
                IndependentDensityFunction::Gradient(g) => g.axis.bit(),
            },
            DensityFunctionComponent::Dependent(f) => match f {
                DependentDensityFunction::ShiftedNoise(n) => {
                    inputs | noise_scale_axes(n.xz_scale, n.y_scale)
                }
                DependentDensityFunction::Slide(_) => inputs | AXIS_Y,
                DependentDensityFunction::Slice(s) => inputs & !s.axis.bit(),
                // The upper bound is read at the sampled Y, so this is only sound
                // while that bound is itself Y-free; `domain_axes_are_sound` proves it.
                DependentDensityFunction::FindTopSurface(_) => inputs & !AXIS_Y,
                _ => inputs,
            },
            DensityFunctionComponent::Wrapper(_) => inputs,
        };
    }

    axes
}

/// Split the sub-graph feeding `root` at every `interpolated` node: the wrappers
/// themselves and everything above them evaluate per block, while their inputs
/// are only ever read off the cell lattice.
pub(super) fn compute_outer_terms(
    stack: &[DensityFunctionComponent],
    root: usize,
) -> (Vec<usize>, Vec<usize>) {
    let is_interpolated = |i: usize| {
        matches!(
            &stack[i],
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::Interpolated(_))
        )
    };

    let mut reached = vec![false; stack.len()];
    let mut pending = vec![root];
    reached[root] = true;
    while let Some(i) = pending.pop() {
        if is_interpolated(i) {
            continue;
        }
        stack[i].visit_input_indices(&mut |j| {
            if !reached[j] {
                reached[j] = true;
                pending.push(j);
            }
        });
    }

    let mut terms = Vec::new();
    let mut wrappers = Vec::new();
    for i in 0..stack.len() {
        if !reached[i] {
            continue;
        }
        if is_interpolated(i) {
            wrappers.push(i);
        } else {
            terms.push(i);
        }
    }
    (terms, wrappers)
}

/// Reorder the stack into three zones for optimal `evaluate_forward` performance:
///
///   Zone A `[0..column_boundary)`:  column-only entries reachable from final_density
///   Zone B `[column_boundary..fd_boundary)`: per-Y entries reachable from final_density
///   Zone C `[fd_boundary..n)`:               entries not reachable from final_density
///
/// Within each zone, topological order is maintained (children before parents).
/// Returns `(column_boundary, fd_boundary)`.
pub(super) fn reorder_stack_for_evaluation(
    stack: &mut Vec<DensityFunctionComponent>,
    per_block: &mut Vec<bool>,
    node_labels: &mut Vec<String>,
    roots: &mut [usize],
    final_density_root_idx: usize,
) -> (usize, usize) {
    let n = stack.len();
    let fd_index = roots[final_density_root_idx];

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

    // Classify each entry into a zone
    //   Zone A (0): fd_reachable AND the column pass can evaluate it
    //   Zone B (1): fd_reachable AND it has to be re-evaluated as Y moves
    //   Zone C (2): NOT fd_reachable
    let column_ready = compute_column_ready(stack);
    let mut zone = vec![2u8; n];
    for i in 0..n {
        if fd_reachable[i] {
            zone[i] = !column_ready[i] as u8;
        }
    }

    // The stack arrives topologically ordered, so sorting by original index
    // within a zone keeps children ahead of parents inside that zone.
    let mut sorted_indices: Vec<usize> = (0..n).collect();
    sorted_indices.sort_by_key(|&i| (zone[i], i));

    let mut old_to_new = vec![0usize; n];
    for (new_idx, &old_idx) in sorted_indices.iter().enumerate() {
        old_to_new[old_idx] = new_idx;
    }

    let old_stack: Vec<DensityFunctionComponent> = stack.drain(..).collect();
    let old_per_block: Vec<bool> = per_block.drain(..).collect();
    let old_labels: Vec<String> = node_labels.drain(..).collect();

    for &old_idx in &sorted_indices {
        stack.push(old_stack[old_idx].clone());
        per_block.push(old_per_block[old_idx]);
        node_labels.push(old_labels[old_idx].clone());
    }

    for entry in stack.iter_mut() {
        entry.rewrite_indices(&old_to_new);
    }

    for root in roots.iter_mut() {
        *root = old_to_new[*root];
    }

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

pub fn build_functions(
    functions: &BTreeMap<ResourceLocation, ProtoDensityFunction>,
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    noise_settings: &NoiseGeneratorSettings,
    seed: u64,
    default_block_state: VoxelId,
    default_fluid_state: VoxelId,
) -> NoiseRouter {
    let random = RandomSource::new(seed, noise_settings.legacy_random_source);
    let builder_options = ChunkNoiseFunctionBuilderOptions {
        horizontal_cell_block_count: 4,
        vertical_cell_block_count: 8,
        start_biome_x: 0,
        start_biome_z: 0,
        horizontal_biome_end: 4,
    };
    let inline = InlineReference(functions);
    let inlined: BTreeMap<ResourceLocation, ProtoDensityFunction> = functions
        .iter()
        .map(|(id, function)| {
            let function = function.rewrite_children(&inline);
            let axes = function.domain_axes();
            (
                id.clone(),
                function.rewrite_children(&SliceUniformAxes::new(axes)),
            )
        })
        .collect();
    let mut builder = FunctionStackBuilder::new(random, seed, &inlined, noises);
    let nr = &noise_settings.noise_router;
    let slice = SliceUniformAxes::new(ALL_AXES);
    let mut root = |builder: &mut FunctionStackBuilder<'_>, holder: &DensityFunctionHolder| {
        builder.component(&slice.rewrite(&inline.rewrite(holder)))
    };
    let temperature_index = root(&mut builder, &nr.temperature);
    let vegetation_index = root(&mut builder, &nr.vegetation);
    let continents_index = root(&mut builder, &nr.continents);
    let erosion_index = root(&mut builder, &nr.erosion);
    let depth_index = root(&mut builder, &nr.depth);
    let ridges_index = root(&mut builder, &nr.ridges);
    let chunk_surface_level_index = root(&mut builder, &nr.chunk_surface_level);
    let final_density_index = root(&mut builder, &nr.final_density);

    let mut roots = [
        temperature_index,
        vegetation_index,
        continents_index,
        erosion_index,
        depth_index,
        ridges_index,
        chunk_surface_level_index,
        final_density_index,
    ];

    optimize_stack(&mut builder.stack, &mut roots);

    let mut per_block = compute_per_block(&builder.stack);

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
        7, // final_density is roots[7]
    );

    let final_density_index = roots[7];

    // Expose beach and surface octave noises for the Beta surface pass.
    // Only populated when using the Beta (legacy) random source; modern router gets None.
    let (beta_beach_noise, beta_surface_noise, beta_terrain_f64_opt) =
        if noise_settings.legacy_random_source {
            let (_, _, _, beach, surface, _, _) = beta_seed::seed_beta_terrain_f64(seed);
            let f64_noises = beta_terrain_f64::BetaTerrainF64::new(seed);
            (
                Some(Box::new(beach)),
                Some(Box::new(surface)),
                Some(Box::new(f64_noises)),
            )
        } else {
            (None, None, None)
        };

    let (outer_terms, outer_wrappers) = compute_outer_terms(&builder.stack, final_density_index);
    let outer_wrapper_inputs: Vec<usize> = outer_wrappers
        .iter()
        .map(|&i| match &builder.stack[i] {
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::Interpolated(x)) => {
                x.input_index
            }
            _ => unreachable!(),
        })
        .collect();

    // Every value a caller reads back out of Zone B. The schedule may skip any
    // node outside this set, so it is also exactly what the debug verify checks.
    let zone_b_roots: Vec<usize> = std::iter::once(final_density_index)
        .chain(outer_wrapper_inputs.iter().copied())
        .collect();
    let zone_b_schedule = {
        let members: Vec<usize> = (column_boundary..=final_density_index).collect();
        branch_schedule::build(&builder.stack, &members, &zone_b_roots)
    };

    // The lattice the chunk fill walks is the wrappers' own cell geometry, so a
    // datapack whose wrappers disagree would need one lattice per cell size.
    let mut cell_sizes = outer_wrappers.iter().map(|&i| match &builder.stack[i] {
        DensityFunctionComponent::Wrapper(WrapperDensityFunction::Interpolated(x)) => {
            (x.cell_size_xz as usize, x.cell_size_y as usize)
        }
        _ => unreachable!(),
    });
    let (h_cell_blocks, v_cell_blocks) = match cell_sizes.next() {
        Some(first) => {
            assert!(
                cell_sizes.all(|other| other == first),
                "final_density mixes interpolated cell sizes"
            );
            first
        }
        None => (
            builder_options.horizontal_cell_block_count,
            builder_options.vertical_cell_block_count,
        ),
    };

    let router = NoiseRouter {
        temperature_index: roots[0],
        vegetation_index: roots[1],
        continents_index: roots[2],
        erosion_index: roots[3],
        depth_index: roots[4],
        ridges_index: roots[5],
        chunk_surface_level_index: roots[6],
        final_density_index,
        noise_min_y: noise_settings.noise.min_y,
        noise_height: noise_settings.noise.height,
        sea_level: noise_settings.sea_level,
        default_block_state,
        default_fluid_state,
        world_seed: seed,
        beta_beach_noise,
        beta_surface_noise,
        beta_terrain_f64: beta_terrain_f64_opt,
        per_block: per_block.into_boxed_slice(),
        outer_terms: outer_terms.into_boxed_slice(),
        outer_wrappers: outer_wrappers.into_boxed_slice(),
        outer_wrapper_inputs: outer_wrapper_inputs.into_boxed_slice(),
        column_boundary,
        fd_boundary,
        h_cell_blocks,
        v_cell_blocks,
        stack: Box::from(builder.stack),
        node_labels: node_labels.into_boxed_slice(),
        zone_b_schedule,
        zone_b_roots: zone_b_roots.into_boxed_slice(),
    };

    router
}

pub(super) struct FunctionStackBuilder<'a> {
    random: RandomSource,
    world_seed: u64,
    functions: &'a BTreeMap<ResourceLocation, ProtoDensityFunction>,
    noises: &'a BTreeMap<ResourceLocation, NoiseParam>,
    stack: Vec<DensityFunctionComponent>,
    built: HashMap<ProtoDensityFunction, usize>,
}

impl<'a> FunctionStackBuilder<'a> {
    fn new(
        random: RandomSource,
        world_seed: u64,
        functions: &'a BTreeMap<ResourceLocation, ProtoDensityFunction>,
        noises: &'a BTreeMap<ResourceLocation, NoiseParam>,
    ) -> Self {
        Self {
            random,
            world_seed,
            functions,
            noises,
            stack: Vec::new(),
            built: HashMap::new(),
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
        let (input_index) = self.component(&function.input);
        let comp = &self.stack[input_index];
        self.register_component(
            ProtoDensityFunction::BlendDensity(SingleArgumentFunction {
                input: function.input.clone(),
            }),
            comp.clone(),
        );
    }

    fn visit_cache(&mut self, function: &SingleArgumentFunction) {
        let input_index = self.component(&function.input);
        let input = &self.stack[input_index];
        let proto = ProtoDensityFunction::Cache(SingleArgumentFunction {
            input: function.input.clone(),
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
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::Cache(Cache {
                input_index,
                min_value,
                max_value,
            })),
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

    fn visit_reciprocal(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Reciprocal);
    }

    fn visit_negate(&mut self, function: &SingleArgumentFunction) {
        let input_index = self.component(&function.input);
        let input = &self.stack[input_index];
        let (min_value, max_value) =
            Affine::compute_range(input.min_value(), input.max_value(), -1.0, 0.0);
        self.register_component(
            ProtoDensityFunction::Negate(SingleArgumentFunction {
                input: function.input.clone(),
            }),
            DensityFunctionComponent::Dependent(DependentDensityFunction::Affine(Affine {
                input_index,
                scale: -1.0,
                offset: 0.0,
                min_value,
                max_value,
            })),
        );
    }

    fn visit_squeeze(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Squeeze);
    }

    fn visit_sqrt(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Sqrt);
    }

    fn visit_log(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Log);
    }

    fn visit_sign(&mut self, function: &SingleArgumentFunction) {
        self.unary(function, UnaryOperation::Sign);
    }

    fn visit_pow(&mut self, function: &PowFunctionArguments) {
        let proto = ProtoDensityFunction::Pow(function.clone());
        // A constant base keeps the transcendental even when the exponent is also
        // constant, so the exponent lowering only applies to a moving base.
        if constant_value(&function.base).is_none()
            && let Some(lowered) = constant_value(&function.exponent)
                .and_then(|exponent| lower_constant_exponent(&function.base, exponent))
        {
            let index = self.component(&lowered);
            self.built.insert(proto, index);
            return;
        }
        self.binary(
            &function.base,
            &function.exponent,
            proto,
            BinaryOperation::Pow,
        );
    }

    fn visit_round(&mut self, mode: RoundingMode, function: &RoundFunctionArguments) {
        let proto = match mode {
            RoundingMode::Floor => ProtoDensityFunction::Floor(function.clone()),
            RoundingMode::Round => ProtoDensityFunction::Round(function.clone()),
            RoundingMode::Ceil => ProtoDensityFunction::Ceil(function.clone()),
            RoundingMode::Truncate => ProtoDensityFunction::Truncate(function.clone()),
        };
        self.binary(
            &function.input,
            &function.multiple,
            proto,
            BinaryOperation::Round(mode),
        );
    }

    fn visit_add(&mut self, arg: &TwoArgumentFunction) {
        self.binary(
            &arg.left,
            &arg.right,
            ProtoDensityFunction::Add(arg.clone()),
            BinaryOperation::Add,
        );
    }

    fn visit_sub(&mut self, function: &TwoArgumentFunction) {
        self.binary(
            &function.left,
            &function.right,
            ProtoDensityFunction::Sub(function.clone()),
            BinaryOperation::Subtract,
        );
    }

    fn visit_div(&mut self, function: &TwoArgumentFunction) {
        self.binary(
            &function.left,
            &function.right,
            ProtoDensityFunction::Div(function.clone()),
            BinaryOperation::Divide,
        );
    }

    fn visit_mul(&mut self, function: &TwoArgumentFunction) {
        self.binary(
            &function.left,
            &function.right,
            ProtoDensityFunction::Mul(function.clone()),
            BinaryOperation::Multiply,
        );
    }

    fn visit_min(&mut self, function: &TwoArgumentFunction) {
        self.binary(
            &function.left,
            &function.right,
            ProtoDensityFunction::Min(function.clone()),
            BinaryOperation::Min,
        );
    }

    fn visit_max(&mut self, function: &TwoArgumentFunction) {
        self.binary(
            &function.left,
            &function.right,
            ProtoDensityFunction::Max(function.clone()),
            BinaryOperation::Max,
        );
    }

    fn visit_old_blended_noise(
        &mut self,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) {
        // Legacy (Beta) mode seeds low/high/selector from LegacyRandom(world_seed)
        // exactly like ChunkProviderGenerate.java:33-35 and omits the trailing /128
        // (ChunkProviderGenerate.java:280-297 vs BlendedNoise.java:159).
        let (mut random, final_divisor) = if let RandomSource::Legacy(_) = self.random {
            (RandomSource::new(self.world_seed, true), 1.0)
        } else {
            (self.random.clone().fork_hash("minecraft:terrain"), 128.0)
        };
        let blended = OldBlendedNoise::new(
            &mut random,
            xz_scale as f32,
            y_scale as f32,
            xz_factor as f64,
            y_factor as f64,
            smear_scale_multiplier as f32,
            final_divisor,
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

    fn visit_noise(
        &mut self,
        noise_holder: &NoiseHolder,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Option<&DensityFunctionHolder>,
        shift_y: Option<&DensityFunctionHolder>,
        shift_z: Option<&DensityFunctionHolder>,
    ) {
        let noise_name = Self::noise_name(noise_holder);
        let sampler = self.noise_sampler(noise_holder);
        let proto = ProtoDensityFunction::Noise {
            noise: noise_holder.clone(),
            xz_scale: xz_scale.into(),
            y_scale: y_scale.into(),
            shift_x: shift_x.cloned(),
            shift_y: shift_y.cloned(),
            shift_z: shift_z.cloned(),
        };

        if shift_x.is_none() && shift_y.is_none() && shift_z.is_none() {
            self.register_component(
                proto,
                DensityFunctionComponent::Independent(IndependentDensityFunction::Noise(Noise {
                    noise_name,
                    sampler,
                    xz_scale: xz_scale as f64,
                    y_scale: y_scale as f64,
                })),
            );
            return;
        }

        let zero = DensityFunctionHolder::Value(0.0.into());
        let input_x_index = self.component(shift_x.unwrap_or(&zero));
        let input_y_index = self.component(shift_y.unwrap_or(&zero));
        let input_z_index = self.component(shift_z.unwrap_or(&zero));
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::ShiftedNoise(
                ShiftedNoise {
                    noise_name,
                    input_x_index,
                    input_y_index,
                    input_z_index,
                    xz_scale: xz_scale as f64,
                    y_scale: y_scale as f64,
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
            min_inclusive: NoiseValue(min_inclusive),
            max_exclusive: NoiseValue(max_exclusive),
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
                noise: function.clone(),
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
                noise: function.clone(),
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
                noise: argument.clone(),
            },
            DensityFunctionComponent::Independent(IndependentDensityFunction::Shift(Shift {
                noise_name,
                sampler,
            })),
        );
    }

    fn visit_end_outer_islands(&mut self) {
        self.register_component(
            ProtoDensityFunction::EndOuterIslands,
            DensityFunctionComponent::Independent(IndependentDensityFunction::EndOuterIslands(
                EndIslands::new(self.world_seed),
            )),
        );
    }

    fn visit_clamp(&mut self, input: &DensityFunctionHolder, min: f64, max: f64) {
        let (input_index) = self.component(input);
        let input_min = self.stack[input_index].min_value();
        let input_max = self.stack[input_index].max_value();
        let proto = ProtoDensityFunction::Clamp(ClampArguments {
            input: input.clone(),
            min: NoiseValue(min),
            max: NoiseValue(max),
        });
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::Clamp(Clamp {
                input_index,
                min_value: input_min.clamp(min as f32, max as f32),
                max_value: input_max.clamp(min as f32, max as f32),
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

    fn visit_gradient(
        &mut self,
        axis: Axis,
        tiling: TilingMode,
        from_coordinate: i32,
        to_coordinate: i32,
        from_value: f64,
        to_value: f64,
    ) {
        let proto = ProtoDensityFunction::Gradient(GradientArguments {
            axis,
            tiling,
            from_coordinate,
            to_coordinate,
            from_value: NoiseValue(from_value),
            to_value: NoiseValue(to_value),
        });
        let component = if axis == Axis::Y && tiling == TilingMode::ClampToEdge {
            IndependentDensityFunction::ClampedYGradient(ClampedYGradient {
                from_y: from_coordinate as f32,
                to_y: to_coordinate as f32,
                from_value: from_value as f32,
                to_value: to_value as f32,
            })
        } else {
            IndependentDensityFunction::Gradient(Gradient {
                axis,
                tiling,
                from_coordinate,
                to_coordinate,
                from_value: from_value as f32,
                to_value: to_value as f32,
            })
        };
        self.register_component(proto, DensityFunctionComponent::Independent(component));
    }

    fn visit_lerp(
        &mut self,
        alpha: &DensityFunctionHolder,
        first: &DensityFunctionHolder,
        second: &DensityFunctionHolder,
    ) {
        let alpha_index = self.component(alpha);
        let first_index = self.component(first);
        let second_index = self.component(second);
        let (min_value, max_value) = lerp_range(
            (
                self.stack[alpha_index].min_value(),
                self.stack[alpha_index].max_value(),
            ),
            (
                self.stack[first_index].min_value(),
                self.stack[first_index].max_value(),
            ),
            (
                self.stack[second_index].min_value(),
                self.stack[second_index].max_value(),
            ),
        );
        self.register_component(
            ProtoDensityFunction::Lerp {
                alpha: alpha.clone(),
                first: first.clone(),
                second: second.clone(),
            },
            DensityFunctionComponent::Dependent(DependentDensityFunction::Lerp(Lerp {
                alpha_index,
                first_index,
                second_index,
                min_value,
                max_value,
            })),
        );
    }

    fn visit_slice(&mut self, axis: Axis, coordinate: i32, input: &DensityFunctionHolder) {
        let input_index = self.component(input);
        let proto = ProtoDensityFunction::Slice {
            axis,
            coordinate,
            input: input.clone(),
        };
        if input.domain_axes() & axis.bit() == 0 {
            let component = self.stack[input_index].clone();
            self.register_component(proto, component);
            return;
        }
        let min_value = self.stack[input_index].min_value();
        let max_value = self.stack[input_index].max_value();
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::Slice(Slice {
                axis,
                coordinate,
                input_index,
                min_value,
                max_value,
            })),
        );
    }

    fn visit_distance_to_point(&mut self, point: [i32; 3], metric: DistanceMetric) {
        self.register_component(
            ProtoDensityFunction::DistanceToPoint { point, metric },
            DensityFunctionComponent::Independent(IndependentDensityFunction::DistanceToPoint(
                DistanceToPoint {
                    point: IVec3::new(point[0], point[1], point[2]),
                    metric,
                },
            )),
        );
    }

    fn visit_interval_select(
        &mut self,
        input: &DensityFunctionHolder,
        thresholds: &[NoiseValue],
        functions: &[DensityFunctionHolder],
    ) {
        let mut lowered = functions[functions.len() - 1].clone();
        for (threshold, function) in thresholds.iter().zip(functions).rev() {
            lowered = DensityFunctionHolder::Owned(Box::new(ProtoDensityFunction::RangeChoice {
                input: input.clone(),
                min_inclusive: NoiseValue(f64::NEG_INFINITY),
                max_exclusive: *threshold,
                when_in_range: function.clone(),
                when_out_of_range: lowered,
            }));
        }
        let index = self.component(&lowered);
        self.built.insert(
            ProtoDensityFunction::IntervalSelect(IntervalSelectArguments {
                input: input.clone(),
                thresholds: thresholds.to_vec(),
                functions: functions.to_vec(),
            }),
            index,
        );
    }

    fn visit_find_top_surface(
        &mut self,
        density: &DensityFunctionHolder,
        upper_bound: &DensityFunctionHolder,
        lower_bound: i32,
        cell_height: std::num::NonZeroU32,
    ) {
        let (density_index) = self.component(density);
        let (upper_bound_index) = self.component(upper_bound);
        let max_value = self.stack[upper_bound_index]
            .max_value()
            .max(lower_bound as f32);
        let proto = ProtoDensityFunction::FindTopSurface {
            density: density.clone(),
            upper_bound: upper_bound.clone(),
            lower_bound,
            cell_height,
        };
        self.register_component(
            proto,
            DensityFunctionComponent::Dependent(DependentDensityFunction::FindTopSurface(
                FindTopSurface {
                    density_index,
                    upper_bound_index,
                    lower_bound: lower_bound as f32,
                    cell_height: cell_height.get() as f32,
                    max_value,
                },
            )),
        );
    }

    fn visit_interpolated(
        &mut self,
        input: &DensityFunctionHolder,
        cell_size_xz: u32,
        cell_size_y: u32,
    ) {
        let input_index = self.component(input);
        let component = &self.stack[input_index];
        let min_value = component.min_value();
        let max_value = component.max_value();

        self.register_component(
            ProtoDensityFunction::Interpolated {
                input: input.clone(),
                cell_size_xz,
                cell_size_y,
            },
            DensityFunctionComponent::Wrapper(WrapperDensityFunction::Interpolated(Interpolated {
                input_index,
                cell_size_xz,
                cell_size_y,
                min_value,
                max_value,
            })),
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

    fn binary(
        &mut self,
        left: &DensityFunctionHolder,
        right: &DensityFunctionHolder,
        proto: ProtoDensityFunction,
        operation: BinaryOperation,
    ) {
        let (input1_index) = self.component(left);
        let arg1 = &self.stack[input1_index];
        let min1 = arg1.min_value();
        let max1 = arg1.max_value();
        let arg1_constant = arg1.as_constant();

        let (input2_index) = self.component(right);
        let arg2 = &self.stack[input2_index];
        let min2 = arg2.min_value();
        let max2 = arg2.max_value();
        let arg2_constant = arg2.as_constant();

        let (min_value, max_value) = binary_range(operation, (min1, max1), (min2, max2));

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
        let (input_index) = self.component(&arg.input);
        let input = &self.stack[input_index];
        let min = input.min_value();
        let max = input.max_value();
        let proto = match operation {
            UnaryOperation::Abs => ProtoDensityFunction::Abs(arg.clone()),
            UnaryOperation::Square => ProtoDensityFunction::Square(arg.clone()),
            UnaryOperation::Cube => ProtoDensityFunction::Cube(arg.clone()),
            UnaryOperation::HalfNegative => ProtoDensityFunction::HalfNegative(arg.clone()),
            UnaryOperation::QuarterNegative => ProtoDensityFunction::QuarterNegative(arg.clone()),
            UnaryOperation::Reciprocal => ProtoDensityFunction::Reciprocal(arg.clone()),
            UnaryOperation::Squeeze => ProtoDensityFunction::Squeeze(arg.clone()),
            UnaryOperation::Sqrt => ProtoDensityFunction::Sqrt(arg.clone()),
            UnaryOperation::Log => ProtoDensityFunction::Log(arg.clone()),
            UnaryOperation::Sign => ProtoDensityFunction::Sign(arg.clone()),
        };

        let (min_value, max_value) = unary_range(operation, min, max);

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
            NoiseHolder::Owned(x) => Self::from_noise_param(&mut self.random.clone(), "inline", x),
        }
    }

    fn from_noise_param<R: mcrs_minecraft_random::Random>(
        random: &mut R,
        id: &str,
        param: &NoiseParam,
    ) -> NoiseSampler {
        if param.normalize != Normalization::Enabled {
            panic!(
                "Noise {id}: normalize {:?} is not supported",
                param.normalize
            );
        }
        NoiseSampler::from_params(
            random,
            param.base_octave,
            param.octave_amplitudes(),
            param.base_amplitude.0,
        )
    }

    fn create_noise(&mut self, id: &ResourceLocation) -> NoiseSampler {
        match id.as_str() {
            "minecraft:nether/temperature" => {
                return NoiseSampler::new(
                    &mut LegacyRandom::new(self.world_seed),
                    -7,
                    vec![1.0, 1.0],
                );
            }
            "minecraft:nether/vegetation" => {
                return NoiseSampler::new(
                    &mut LegacyRandom::new(self.world_seed.wrapping_add(1)),
                    -7,
                    vec![1.0, 1.0],
                );
            }
            _ => {}
        }

        if let RandomSource::Legacy(_) = &self.random {
            match id.as_str() {
                "minecraft:offset" => {
                    return NoiseSampler::new(
                        &mut self.random.clone().fork_hash("minecraft:offset"),
                        0,
                        vec![0.0],
                    );
                }
                // Beta terrain 2D noises: elements 3 and 4 of the sequential
                // seed_beta_terrain stream, sampled at noise-cell coords with their
                // Java frequency constants (1.121 scale, 200.0 depth). Bounds match
                // sample_xz: |acc| <= A * (2^octaves - 1) with per-octave |s| ~ 2.
                "mcrs:beta/scale" => {
                    let (_, _, _, _, _, scale_noise, _) =
                        beta_seed::seed_beta_terrain(self.world_seed);
                    return NoiseSampler::beta_octave_2d(scale_noise, 1.121, 2048.0);
                }
                "mcrs:beta/depth" => {
                    let (_, _, _, _, _, _, depth_noise) =
                        beta_seed::seed_beta_terrain(self.world_seed);
                    return NoiseSampler::beta_octave_2d(depth_noise, 200.0, 131072.0);
                }
                // Beta climate simplex noises: three independent LegacyRandom streams
                // (WorldChunkManager.java lines 18-20). Frequency constants are
                // id-intrinsic (JSON samples with xz_scale=1.0). Post-processing
                // lives in minecraft:beta/{temperature,vegetation,climate_detail}.
                "mcrs:beta/temperature" => {
                    let (temp_noise, _, _) = beta_seed::seed_beta_climate(self.world_seed);
                    return NoiseSampler::beta_simplex_2d(temp_noise, 0.025, 0.25, 16.0);
                }
                "mcrs:beta/vegetation" => {
                    let (_, rain_noise, _) = beta_seed::seed_beta_climate(self.world_seed);
                    return NoiseSampler::beta_simplex_2d(rain_noise, 0.05, 1.0 / 3.0, 16.0);
                }
                "mcrs:beta/climate_detail" => {
                    let (_, _, detail_noise) = beta_seed::seed_beta_climate(self.world_seed);
                    return NoiseSampler::beta_simplex_2d(detail_noise, 0.25, 1.0 / 1.7, 4.0);
                }
                _ => {}
            }
        }

        let mut random = self.random.clone().fork_hash(id.as_str());
        let noise_param = self.noises.get(id);
        if noise_param.is_none() {
            panic!("Noise not loaded: {}", id);
        }
        Self::from_noise_param(&mut random, id.as_str(), noise_param.unwrap())
    }
}

#[cfg(all(test, feature = "serde"))]
mod arithmetic_node_tests {
    use super::{
        DensityFunctionComponent, DependentDensityFunction, FunctionStackBuilder, UnaryOperation,
    };
    use crate::density_function::proto::{
        DensityFunctionHolder, HashableF64, NoiseParam, Normalization, ProtoDensityFunction,
    };
    use crate::noise::normal_noise::NoiseSampler;
    use crate::spline::RangeFunction;
    use bevy_math::IVec3;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_random::{Random, RandomSource};
    use std::collections::BTreeMap;

    fn build(json: &str) -> (f32, f32, f32) {
        let proto: ProtoDensityFunction =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let functions = BTreeMap::new();
        let noises = BTreeMap::new();
        let mut builder =
            FunctionStackBuilder::new(RandomSource::new(0, false), 0, &functions, &noises);
        let index = builder.component(&DensityFunctionHolder::Owned(Box::new(proto)));
        let value =
            DensityFunctionComponent::sample_from_stack(&builder.stack[..=index], IVec3::ZERO);
        let component = &builder.stack[index];
        (value, component.min_value(), component.max_value())
    }

    fn sample(json: &str) -> f32 {
        build(json).0
    }

    /// A y-gradient standing in for any input with a genuinely moving range;
    /// at y = 0 it evaluates to `from`.
    fn moving(from: f32, to: f32) -> String {
        format!(
            r#"{{"type":"gradient","axis":"y","from_coordinate":0,"to_coordinate":16,"from_value":{from},"to_value":{to}}}"#
        )
    }

    #[test]
    fn abs_and_square_cover_both_sides_of_a_zero_crossing_input() {
        let (_, min, max) = build(&format!(
            r#"{{"type":"abs","input":{}}}"#,
            moving(-5.0, 3.0)
        ));
        assert_eq!((min, max), (0.0, 5.0));
        let (_, min, max) = build(&format!(
            r#"{{"type":"square","input":{}}}"#,
            moving(-5.0, 3.0)
        ));
        assert_eq!((min, max), (0.0, 25.0));
        let (_, min, max) = build(&format!(
            r#"{{"type":"square","input":{}}}"#,
            moving(-4.0, 4.0)
        ));
        assert_eq!((min, max), (0.0, 16.0));
        let (_, min, max) = build(&format!(r#"{{"type":"abs","input":{}}}"#, moving(2.0, 6.0)));
        assert_eq!((min, max), (2.0, 6.0));
        let (_, min, max) = build(&format!(
            r#"{{"type":"square","input":{}}}"#,
            moving(-6.0, -2.0)
        ));
        assert_eq!((min, max), (4.0, 36.0));
    }

    #[test]
    fn clamp_range_is_the_image_of_its_input() {
        let (_, min, max) = build(&format!(
            r#"{{"type":"clamp","input":{},"min":-1.0,"max":1.0}}"#,
            moving(-0.5, 0.5)
        ));
        assert_eq!((min, max), (-0.5, 0.5));
        let (_, min, max) = build(&format!(
            r#"{{"type":"clamp","input":{},"min":-1.0,"max":1.0}}"#,
            moving(-8.0, 0.25)
        ));
        assert_eq!((min, max), (-1.0, 0.25));
    }

    #[test]
    fn pow_raises_base_to_exponent() {
        assert_eq!(
            sample(r#"{"type":"pow","base":2.0,"exponent":10.0}"#),
            1024.0
        );
        assert_eq!(
            sample(r#"{"type":"minecraft:pow","base":9.0,"exponent":0.5}"#),
            3.0
        );
        let (_, min, max) = build(r#"{"type":"pow","base":2.0,"exponent":10.0}"#);
        assert_eq!((min, max), (1024.0, 1024.0));
    }

    #[test]
    fn pow_range_covers_a_moving_base_and_exponent() {
        let (_, min, max) = build(&format!(
            r#"{{"type":"pow","base":{},"exponent":{}}}"#,
            moving(0.5, 2.0),
            moving(-2.0, 2.0)
        ));
        assert_eq!((min, max), (0.25, 4.0));
    }

    #[test]
    fn sqrt_log_and_sign() {
        assert_eq!(sample(r#"{"type":"sqrt","input":16.0}"#), 4.0);
        assert_eq!(sample(r#"{"type":"log","input":1.0}"#), 0.0);
        assert_eq!(sample(r#"{"type":"sign","input":-3.0}"#), -1.0);
        assert_eq!(sample(r#"{"type":"minecraft:sign","input":0.0}"#), 0.0);
        assert_eq!(
            build(&format!(
                r#"{{"type":"sign","input":{}}}"#,
                moving(-4.0, 4.0)
            ))
            .1,
            -1.0
        );
    }

    #[test]
    fn rounding_defaults_to_whole_numbers() {
        assert_eq!(sample(r#"{"type":"floor","input":2.7}"#), 2.0);
        assert_eq!(sample(r#"{"type":"ceil","input":2.1}"#), 3.0);
        assert_eq!(sample(r#"{"type":"truncate","input":-2.7}"#), -2.0);
        // Java's Math.round is half-up, so -2.5 rounds towards zero.
        assert_eq!(sample(r#"{"type":"round","input":-2.5}"#), -2.0);
        assert_eq!(sample(r#"{"type":"round","input":2.5}"#), 3.0);
    }

    #[test]
    fn rounding_snaps_to_a_multiple() {
        assert_eq!(
            sample(r#"{"type":"floor","input":7.0,"multiple":3.0}"#),
            6.0
        );
        let (value, min, max) = build(&format!(
            r#"{{"type":"ceil","input":{},"multiple":4.0}}"#,
            moving(0.0, 10.0)
        ));
        assert_eq!(value, 0.0);
        assert_eq!((min, max), (0.0, 12.0));
    }

    fn nether_climate_params() -> NoiseParam {
        NoiseParam {
            base_octave: -7,
            base_amplitude: HashableF64(0.9494731054427981),
            octave_count: 2,
            normalize: Normalization::Enabled,
            amplitude_modifiers: Vec::new(),
        }
    }

    #[test]
    fn the_nether_climate_noises_are_seeded_the_legacy_way() {
        const SEED: u64 = 845;
        let functions = BTreeMap::new();
        let mut noises = BTreeMap::new();
        for name in [
            "minecraft:nether/temperature",
            "minecraft:nether/vegetation",
        ] {
            noises.insert(
                ResourceLocation::parse(name).unwrap().into(),
                nether_climate_params(),
            );
        }
        let mut builder =
            FunctionStackBuilder::new(RandomSource::new(SEED, true), SEED, &functions, &noises);

        for (name, seed_offset) in [
            ("minecraft:nether/temperature", 0),
            ("minecraft:nether/vegetation", 1),
        ] {
            let id = ResourceLocation::parse(name).unwrap().into();
            let built = builder.create_noise(&id);
            assert_eq!(
                built,
                NoiseSampler::new(
                    &mut LegacyRandom::new(SEED + seed_offset),
                    -7,
                    vec![1.0, 1.0]
                ),
                "{name} must come from a raw legacy stream"
            );
            let param = nether_climate_params();
            assert_ne!(
                built,
                NoiseSampler::from_params(
                    &mut RandomSource::new(SEED, true).fork_hash(name),
                    param.base_octave,
                    param.octave_amplitudes(),
                    param.base_amplitude.0,
                ),
                "{name} must not fall through to the hashed fork"
            );
        }
    }

    fn unary_operation(json: &str) -> Option<UnaryOperation> {
        let proto: ProtoDensityFunction = serde_json::from_str(json).unwrap();
        let functions = BTreeMap::new();
        let noises = BTreeMap::new();
        let mut builder =
            FunctionStackBuilder::new(RandomSource::new(0, false), 0, &functions, &noises);
        let index = builder.component(&DensityFunctionHolder::Owned(Box::new(proto)));
        match &builder.stack[index] {
            DensityFunctionComponent::Dependent(DependentDensityFunction::Unary(unary)) => {
                Some(unary.operation)
            }
            _ => None,
        }
    }

    #[test]
    fn a_constant_exponent_avoids_the_transcendental() {
        let pow = |exponent: &str| {
            format!(
                r#"{{"type":"pow","base":{},"exponent":{exponent}}}"#,
                moving(0.5, 2.0)
            )
        };
        assert_eq!(unary_operation(&pow("0.5")), Some(UnaryOperation::Sqrt));
        assert_eq!(unary_operation(&pow("2.0")), Some(UnaryOperation::Square));
        assert_eq!(unary_operation(&pow("3.0")), Some(UnaryOperation::Cube));
        assert_eq!(
            unary_operation(&pow("-2.0")),
            Some(UnaryOperation::Reciprocal)
        );
        assert_eq!(unary_operation(&pow("4.0")), None);

        assert_eq!(sample(&pow("1.0")), sample(&moving(0.5, 2.0)));
        assert_eq!(
            sample(&pow("3.0")).to_bits(),
            sample(&format!(
                r#"{{"type":"cube","input":{}}}"#,
                moving(0.5, 2.0)
            ))
            .to_bits()
        );
    }

    #[test]
    fn lerp_follows_alpha_outside_the_unit_interval() {
        let lerp = |alpha: String| {
            format!(r#"{{"type":"lerp","alpha":{alpha},"first":0.0,"second":1.0}}"#)
        };
        let (_, min, max) = build(&lerp(moving(0.0, 1.0).to_string()));
        assert_eq!((min, max), (0.0, 1.0));
        let (_, min, max) = build(&lerp(moving(-2.0, 2.0).to_string()));
        assert_eq!((min, max), (-2.0, 2.0));
    }
}
