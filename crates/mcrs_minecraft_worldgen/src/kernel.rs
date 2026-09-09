//! A node is evaluated over its whole stratum, one column at a time, and within
//! a column it holds either a single scalar or a run of `sy` values. These
//! helpers resolve which, once, outside the inner loop, so the loop the compiler
//! sees has no branch and no indirection.

use crate::strata::{AXIS_X, AXIS_Z, Axes, run_len};
use crate::volume::Volume;

/// One node's whole stratum buffer, addressed by the column being evaluated. An
/// axis the stratum drops gets stride zero, so the same lookup broadcasts it
/// over every column that shares the value.
#[derive(Clone, Copy)]
pub struct Runs<'a> {
    data: &'a [f32],
    run: usize,
    stride_x: usize,
    stride_z: usize,
}

impl<'a> Runs<'a> {
    pub fn new(data: &'a [f32], axes: Axes, volume: &Volume) -> Self {
        let size = volume.size();
        let run = run_len(axes, size.y as usize);
        let stride_x = if axes & AXIS_X != 0 { run } else { 0 };
        let plane = if axes & AXIS_X != 0 {
            size.x as usize * run
        } else {
            run
        };
        Self {
            data,
            run,
            stride_x,
            stride_z: if axes & AXIS_Z != 0 { plane } else { 0 },
        }
    }

    #[inline]
    pub fn col(self, ix: usize, iz: usize) -> &'a [f32] {
        let base = ix * self.stride_x + iz * self.stride_z;
        &self.data[base..base + self.run]
    }
}

/// Splits a node's output buffer into its columns, in the order the stratum lays
/// them out: Y fastest, then X, then Z, with a dropped axis contributing one
/// iteration. `ext` is the node's own stratum, so its size *is* the loop bound.
#[inline]
pub fn each_column(out: &mut [f32], ext: &Volume, mut f: impl FnMut(&mut [f32], usize, usize)) {
    let size = ext.size();
    let run = size.y as usize;
    let mut rest = out;
    for iz in 0..size.z as usize {
        for ix in 0..size.x as usize {
            let (head, tail) = rest.split_at_mut(run);
            f(head, ix, iz);
            rest = tail;
        }
    }
    debug_assert!(rest.is_empty(), "the columns did not cover the stratum");
}

#[inline]
fn map1(out: &mut [f32], a: &[f32], f: impl Fn(f32) -> f32) {
    if a.len() == out.len() {
        for (o, &x) in out.iter_mut().zip(a) {
            *o = f(x);
        }
    } else {
        let x = f(a[0]);
        out.fill(x);
    }
}

#[inline]
fn zip2(out: &mut [f32], a: &[f32], b: &[f32], f: impl Fn(f32, f32) -> f32) {
    let n = out.len();
    match (a.len() == n, b.len() == n) {
        (true, true) => {
            for (o, (&x, &y)) in out.iter_mut().zip(a.iter().zip(b)) {
                *o = f(x, y);
            }
        }
        (false, true) => {
            let x = a[0];
            for (o, &y) in out.iter_mut().zip(b) {
                *o = f(x, y);
            }
        }
        (true, false) => {
            let y = b[0];
            for (o, &x) in out.iter_mut().zip(a) {
                *o = f(x, y);
            }
        }
        (false, false) => out.fill(f(a[0], b[0])),
    }
}

#[inline]
fn zip3(out: &mut [f32], a: &[f32], b: &[f32], c: &[f32], f: impl Fn(f32, f32, f32) -> f32) {
    let n = out.len();
    if a.len() == n && b.len() == n && c.len() == n {
        for (i, o) in out.iter_mut().enumerate() {
            *o = f(a[i], b[i], c[i]);
        }
        return;
    }
    // A broadcast run holds one value, so its stride is zero and `i * stride`
    // re-reads element zero for every i. Which operands broadcast is settled
    // here; the loop that runs carries no test of its own.
    let stride = |s: &[f32]| usize::from(s.len() == n);
    let (sa, sb, sc) = (stride(a), stride(b), stride(c));
    for (i, o) in out.iter_mut().enumerate() {
        *o = f(a[i * sa], b[i * sb], c[i * sc]);
    }
}

#[inline]
pub fn map_columns(out: &mut [f32], ext: &Volume, a: Runs<'_>, f: impl Fn(f32) -> f32) {
    each_column(out, ext, |run, ix, iz| map1(run, a.col(ix, iz), &f));
}

#[inline]
pub fn zip2_columns(
    out: &mut [f32],
    ext: &Volume,
    a: Runs<'_>,
    b: Runs<'_>,
    f: impl Fn(f32, f32) -> f32,
) {
    each_column(out, ext, |run, ix, iz| {
        zip2(run, a.col(ix, iz), b.col(ix, iz), &f)
    });
}

#[inline]
pub fn zip3_columns(
    out: &mut [f32],
    ext: &Volume,
    a: Runs<'_>,
    b: Runs<'_>,
    c: Runs<'_>,
    f: impl Fn(f32, f32, f32) -> f32,
) {
    each_column(out, ext, |run, ix, iz| {
        zip3(run, a.col(ix, iz), b.col(ix, iz), c.col(ix, iz), &f)
    });
}

/// Reads element `i` of a run that may be a broadcast scalar. Only for paths
/// that cannot hoist the decision, such as the arm merge in a selection node.
#[inline]
pub fn at(s: &[f32], i: usize) -> f32 {
    if s.len() == 1 { s[0] } else { s[i] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scalar_input_broadcasts_over_the_run() {
        let mut out = [0.0; 4];
        zip2(&mut out, &[10.0], &[1.0, 2.0, 3.0, 4.0], |a, b| a + b);
        assert_eq!(out, [11.0, 12.0, 13.0, 14.0]);

        let mut out = [0.0; 4];
        zip2(&mut out, &[1.0, 2.0, 3.0, 4.0], &[10.0], |a, b| a + b);
        assert_eq!(out, [11.0, 12.0, 13.0, 14.0]);
    }

    #[test]
    fn two_scalars_collapse_to_a_scalar_output() {
        let mut out = [0.0; 1];
        zip2(&mut out, &[3.0], &[4.0], |a, b| a * b);
        assert_eq!(out, [12.0]);
    }

    #[test]
    fn map1_broadcasts_and_evaluates_once() {
        let calls = std::cell::Cell::new(0);
        let mut out = [0.0; 4];
        map1(&mut out, &[2.0], |v| {
            calls.set(calls.get() + 1);
            v * 3.0
        });
        assert_eq!(out, [6.0; 4]);
        assert_eq!(
            calls.get(),
            1,
            "a broadcast scalar is not recomputed per element"
        );
    }

    #[test]
    fn a_dropped_axis_broadcasts_across_every_column_that_shares_it() {
        use crate::strata::{AXIS_X, AXIS_Y, AXIS_Z};
        use bevy_math::IVec3;

        let volume = Volume::dense(IVec3::new(2, 3, 2), IVec3::ZERO);
        let data: Vec<f32> = (0..4).map(|i| i as f32).collect();
        let xz = Runs::new(&data, AXIS_X | AXIS_Z, &volume);
        assert_eq!(xz.col(0, 0), &[0.0]);
        assert_eq!(xz.col(1, 0), &[1.0]);
        assert_eq!(xz.col(0, 1), &[2.0]);
        assert_eq!(xz.col(1, 1), &[3.0]);

        let data: Vec<f32> = (0..3).map(|i| i as f32).collect();
        let y = Runs::new(&data, AXIS_Y, &volume);
        assert_eq!(y.col(0, 0), &[0.0, 1.0, 2.0]);
        assert_eq!(y.col(1, 1), y.col(0, 0), "no X and no Z is one run for all");
    }

    #[test]
    fn each_column_walks_the_stratum_in_layout_order() {
        use crate::strata::{AXIS_X, AXIS_Z, stratum};
        use bevy_math::IVec3;

        let volume = Volume::dense(IVec3::new(2, 3, 2), IVec3::ZERO);
        let ext = stratum(AXIS_X | AXIS_Z, &volume);
        let mut out = vec![0.0; 4];
        let mut seen = Vec::new();
        each_column(&mut out, &ext, |o, ix, iz| {
            seen.push((ix, iz));
            o[0] = (ix + 10 * iz) as f32;
        });
        assert_eq!(seen, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert_eq!(out, vec![0.0, 1.0, 10.0, 11.0]);
    }

    #[test]
    fn zip3_mixes_scalar_and_run_inputs() {
        let mut out = [0.0; 3];
        zip3(
            &mut out,
            &[1.0, 2.0, 3.0],
            &[10.0],
            &[100.0, 200.0, 300.0],
            |a, b, c| a + b + c,
        );
        assert_eq!(out, [111.0, 212.0, 313.0]);
    }
}
