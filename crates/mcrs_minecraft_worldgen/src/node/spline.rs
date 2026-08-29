use crate::interval::Interval;
use crate::jmath::{jmax, jmin, lerp};
use crate::kernel::{Runs, at, each_column};
use crate::volume::Volume;

#[derive(Clone, Debug)]
pub enum SplineValue {
    Constant(f32),
    Multipoint(Box<Multipoint>),
}

#[derive(Clone, Debug)]
pub struct Multipoint {
    /// Index into the `coords` list of the owning `Node::Spline`, assigned by
    /// the compiler's pre-order walk: a multipoint's own coordinate first, then
    /// each point's sub-spline in list order, deduplicated by structural
    /// equality so equal coordinates share one index and one graph node.
    pub coord: usize,
    pub locations: Box<[f32]>,
    pub derivatives: Box<[f32]>,
    pub values: Box<[SplineValue]>,
}

#[derive(Clone, Debug)]
pub struct CompiledSpline {
    pub root: SplineValue,
}

impl CompiledSpline {
    pub fn eval<'a>(&self, out: &mut [f32], coords: &dyn Fn(usize) -> Runs<'a>, ext: &Volume) {
        each_column(out, ext, |run, ix, iz| {
            let read = |k: usize| coords(k).col(ix, iz);
            for (i, slot) in run.iter_mut().enumerate() {
                *slot = sample(&self.root, &read, i);
            }
        });
    }

    /// The interval of the whole spline given the interval of each coordinate,
    /// indexed the same way as `eval`'s `read`.
    pub fn range(&self, coord_ranges: &[Interval]) -> Interval {
        value_range(&self.root, coord_ranges)
    }
}

fn sample<'a>(value: &SplineValue, read: &dyn Fn(usize) -> &'a [f32], i: usize) -> f32 {
    let m = match value {
        SplineValue::Constant(v) => return *v,
        SplineValue::Multipoint(m) => m,
    };
    let input = at(read(m.coord), i);
    let last = m.locations.len() - 1;
    let start = find_interval_start(&m.locations, input);
    if start < 0 {
        let value = sample(&m.values[0], read, i);
        return linear_extend(input, &m.locations, value, &m.derivatives, 0);
    }
    let start = start as usize;
    if start == last {
        let value = sample(&m.values[last], read, i);
        return linear_extend(input, &m.locations, value, &m.derivatives, last);
    }
    let x1 = m.locations[start];
    let x2 = m.locations[start + 1];
    let t = (input - x1) / (x2 - x1);
    let y1 = sample(&m.values[start], read, i);
    let y2 = sample(&m.values[start + 1], read, i);
    let d1 = m.derivatives[start];
    let d2 = m.derivatives[start + 1];
    let a = d1 * (x2 - x1) - (y2 - y1);
    let b = -d2 * (x2 - x1) + (y2 - y1);
    lerp(t, y1, y2) + t * (1.0 - t) * lerp(t, a, b)
}

/// `Mth.binarySearch(0, len, i -> input < locations[i]) - 1`, transcribed rather
/// than expressed as `partition_point`: the codec accepts unsorted locations, and
/// two binary searches over an unsorted array need not agree on the index.
fn find_interval_start(locations: &[f32], input: f32) -> isize {
    let mut from = 0usize;
    let mut len = locations.len();
    while len > 0 {
        let half = len / 2;
        let middle = from + half;
        if input < locations[middle] {
            len = half;
        } else {
            from = middle + 1;
            len -= half + 1;
        }
    }
    from as isize - 1
}

/// The zero-derivative case returns `value` untouched instead of computing
/// `value + 0.0 * (input - location)`, which is what keeps an infinite input
/// from poisoning a flat extension. `-0.0` takes it too.
fn linear_extend(
    input: f32,
    locations: &[f32],
    value: f32,
    derivatives: &[f32],
    index: usize,
) -> f32 {
    let derivative = derivatives[index];
    if derivative == 0.0 {
        value
    } else {
        value + derivative * (input - locations[index])
    }
}

fn value_range(value: &SplineValue, coord_ranges: &[Interval]) -> Interval {
    match value {
        SplineValue::Constant(v) => Interval::exact(*v),
        SplineValue::Multipoint(m) => {
            let values: Vec<Interval> = m
                .values
                .iter()
                .map(|v| value_range(v, coord_ranges))
                .collect();
            multipoint_range(coord_ranges[m.coord], &m.locations, &m.derivatives, &values)
        }
    }
}

/// `CubicSpline.Multipoint.range()`. `value_ranges` holds the range of each point
/// value, in point order; the caller owns the recursion over the sub-splines.
pub fn multipoint_range(
    input: Interval,
    locations: &[f32],
    derivatives: &[f32],
    value_ranges: &[Interval],
) -> Interval {
    debug_assert!(!locations.is_empty());
    debug_assert_eq!(locations.len(), derivatives.len());
    debug_assert_eq!(locations.len(), value_ranges.len());
    if input.is_nai() {
        return input;
    }
    let last = locations.len() - 1;
    let mut min_value = f32::INFINITY;
    let mut max_value = f32::NEG_INFINITY;

    if input.min() < locations[0] {
        let first = value_ranges[0];
        let edge1 = linear_extend(input.min(), locations, first.min(), derivatives, 0);
        let edge2 = linear_extend(input.min(), locations, first.max(), derivatives, 0);
        min_value = jmin(min_value, jmin(edge1, edge2));
        max_value = jmax(max_value, jmax(edge1, edge2));
    }
    if input.max() > locations[last] {
        let end = value_ranges[last];
        let edge1 = linear_extend(input.max(), locations, end.min(), derivatives, last);
        let edge2 = linear_extend(input.max(), locations, end.max(), derivatives, last);
        min_value = jmin(min_value, jmin(edge1, edge2));
        max_value = jmax(max_value, jmax(edge1, edge2));
    }
    for range in value_ranges {
        min_value = jmin(min_value, range.min());
        max_value = jmax(max_value, range.max());
    }
    for i in 0..last {
        let d1 = derivatives[i];
        let d2 = derivatives[i + 1];
        // Skipping this guard widens the interval on every flat segment of the
        // shipped continentalness and erosion splines.
        if d1 == 0.0 && d2 == 0.0 {
            continue;
        }
        let x_diff = locations[i + 1] - locations[i];
        let (min1, max1) = (value_ranges[i].min(), value_ranges[i].max());
        let (min2, max2) = (value_ranges[i + 1].min(), value_ranges[i + 1].max());
        let p1 = d1 * x_diff;
        let p2 = d2 * x_diff;
        let min_lerp1 = jmin(min1, min2);
        let max_lerp1 = jmax(max1, max2);
        let min_lerp2 = jmin(p1 - max2 + min1, -p2 + min2 - max1);
        let max_lerp2 = jmax(p1 - min2 + max1, -p2 + max2 - min1);
        min_value = jmin(min_value, min_lerp1 + 0.25 * min_lerp2);
        max_value = jmax(max_value, max_lerp1 + 0.25 * max_lerp2);
    }

    // Vanilla throws instead; a NaI point value or an infinity cancelling in an
    // extension leaves nothing known, which is what NaI means.
    if min_value.is_nan() || max_value.is_nan() {
        return Interval::NAI;
    }
    Interval::of(min_value, max_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strata::{AXIS_Y, NO_AXES};
    use bevy_math::IVec3;
    use std::cell::RefCell;

    fn multipoint(
        coord: usize,
        locations: &[f32],
        derivatives: &[f32],
        values: Vec<SplineValue>,
    ) -> SplineValue {
        SplineValue::Multipoint(Box::new(Multipoint {
            coord,
            locations: locations.into(),
            derivatives: derivatives.into(),
            values: values.into_boxed_slice(),
        }))
    }

    fn constants(values: &[f32]) -> Vec<SplineValue> {
        values.iter().map(|v| SplineValue::Constant(*v)).collect()
    }

    fn eval(root: SplineValue, coords: &[&[f32]], len: usize) -> Vec<f32> {
        let spline = CompiledSpline { root };
        let ext = Volume::new(IVec3::new(1, len as i32, 1), IVec3::ZERO, IVec3::ONE);
        let mut out = vec![0.0; len];
        spline.eval(
            &mut out,
            &|k| {
                let run = coords[k];
                let axes = if run.len() == 1 { NO_AXES } else { AXIS_Y };
                Runs::new(run, axes, &ext)
            },
            &ext,
        );
        out
    }

    fn eval1(root: SplineValue, coord: f32) -> f32 {
        eval(root, &[&[coord]], 1)[0]
    }

    #[test]
    fn the_interior_segment_is_the_cubic_not_a_lerp() {
        let root = multipoint(0, &[0.0, 1.0], &[1.0, -1.0], constants(&[0.0, 1.0]));
        assert_eq!(eval1(root, 0.5), 0.75);
    }

    #[test]
    fn outside_the_endpoints_the_spline_extends_along_the_derivative() {
        let sloped = multipoint(0, &[0.0, 1.0], &[2.0, 3.0], constants(&[3.0, 7.0]));
        assert_eq!(eval1(sloped.clone(), -4.0), 3.0 + 2.0 * -4.0);
        assert_eq!(eval1(sloped, 5.0), 7.0 + 3.0 * (5.0 - 1.0));
    }

    #[test]
    fn a_flat_extension_survives_an_infinite_coordinate() {
        let flat = multipoint(0, &[0.0, 1.0], &[-0.0, 0.0], constants(&[3.0, 7.0]));
        assert_eq!(
            eval1(flat.clone(), f32::NEG_INFINITY),
            3.0,
            "-0.0 takes the short circuit too"
        );
        assert_eq!(eval1(flat, f32::INFINITY), 7.0);
    }

    #[test]
    fn a_location_is_the_start_of_its_own_segment() {
        let root = multipoint(0, &[0.0, 1.0], &[1.0, -1.0], constants(&[0.0, 1.0]));
        assert_eq!(eval1(root.clone(), 0.0), 0.0);
        assert_eq!(
            eval1(root, 1.0),
            1.0,
            "the last location extends, not lerps"
        );
    }

    #[test]
    fn a_scalar_run_agrees_with_element_zero_of_a_full_column() {
        let nested = multipoint(1, &[-1.0, 1.0], &[0.5, -0.5], constants(&[-2.0, 4.0]));
        let root = multipoint(
            0,
            &[-1.0, 0.0, 1.0],
            &[0.25, 1.0, -0.75],
            vec![
                SplineValue::Constant(1.5),
                nested,
                SplineValue::Constant(-3.0),
            ],
        );

        let a_run: &[f32] = &[0.3, -0.9, 0.75, 1.4];
        let b_scalar: &[f32] = &[0.2];

        let column = eval(root.clone(), &[a_run, b_scalar], 4);
        let scalar = eval(root, &[&[a_run[0]], b_scalar], 1);
        assert_eq!(scalar.len(), 1);
        assert_eq!(scalar[0], column[0]);
    }

    #[test]
    fn a_coordinate_on_an_unreached_branch_is_never_read() {
        let reads = RefCell::new(Vec::new());
        let branch = multipoint(1, &[0.0, 1.0], &[0.0, 0.0], constants(&[9.0, 9.0]));
        let root = multipoint(
            0,
            &[0.0, 1.0],
            &[0.0, 0.0],
            vec![SplineValue::Constant(1.0), branch],
        );
        let spline = CompiledSpline { root };
        let coords: [&[f32]; 2] = [&[-5.0], &[0.5]];
        let ext = Volume::point(IVec3::ZERO);
        let mut out = [0.0; 1];
        spline.eval(
            &mut out,
            &|k| {
                reads.borrow_mut().push(k);
                Runs::new(coords[k], NO_AXES, &ext)
            },
            &ext,
        );
        assert_eq!(out[0], 1.0);
        assert_eq!(reads.into_inner(), vec![0]);
    }

    #[test]
    fn range_widens_only_where_the_spline_can_overshoot() {
        let flat = multipoint(0, &[0.0, 1.0], &[0.0, 0.0], constants(&[0.0, 1.0]));
        let curved = multipoint(0, &[0.0, 1.0], &[1.0, -1.0], constants(&[0.0, 1.0]));
        let inside = [Interval::of(0.0, 1.0)];

        let flat = CompiledSpline { root: flat }.range(&inside);
        assert_eq!((flat.min(), flat.max()), (0.0, 1.0));

        let curved = CompiledSpline { root: curved }.range(&inside);
        assert_eq!((curved.min(), curved.max()), (0.0, 1.5));
    }

    #[test]
    fn range_extends_past_a_location_only_when_strictly_past_it() {
        let root = multipoint(0, &[0.0, 1.0], &[2.0, 2.0], constants(&[0.0, 1.0]));
        let spline = CompiledSpline { root };

        let touching = spline.range(&[Interval::of(0.0, 1.0)]);
        assert_eq!((touching.min(), touching.max()), (-0.25, 1.25));

        let below = spline.range(&[Interval::of(-1.0, 1.0)]);
        assert_eq!((below.min(), below.max()), (-2.0, 1.25));
    }

    #[test]
    fn a_constant_root_needs_no_coordinates() {
        let spline = CompiledSpline {
            root: SplineValue::Constant(2.5),
        };
        let ext = Volume::new(IVec3::new(1, 3, 1), IVec3::ZERO, IVec3::ONE);
        let mut out = [0.0; 3];
        spline.eval(&mut out, &|_| unreachable!(), &ext);
        assert_eq!(out, [2.5; 3]);
        assert_eq!(spline.range(&[]), Interval::exact(2.5));
    }
}
