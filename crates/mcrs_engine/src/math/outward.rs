//! Allocation-free outward iterators that emit coordinates in
//! Manhattan-distance-monotonic shells around a centre point.
//!
//! Origin: Spout's `OutwardIterator` (the one primitive worth porting).
//! Within-shell order is implementation-defined but deterministic for a
//! given `(centre, max_radius)`.

use bevy_math::IVec3;

/// Internal shell-walk state for a 2D shell (radius >= 1). At radius `r`
/// the shell contains exactly `4 * r` points, split across four sides
/// of length `r`.
#[derive(Clone, Copy, Debug)]
struct Shell2D {
    radius: i32,
    side: u8,
    step: i32,
}

/// 2D Manhattan-distance-monotonic outward iterator. Yields `(x, z)`
/// offsets relative to the construction centre. Shells are emitted at
/// radii `0, 1, 2, ..., max_radius`; within a shell the order is
/// deterministic for a given input.
///
/// Allocation-free: state is a fixed handful of `i32`/`u8` fields.
#[derive(Clone, Debug)]
pub struct OutwardIterator2D {
    centre_x: i32,
    centre_z: i32,
    max_radius: i32,
    current_radius: i32,
    yielded_centre: bool,
    shell: Shell2D,
    done: bool,
}

impl OutwardIterator2D {
    /// Construct a new iterator centred at `(centre_x, centre_z)` that
    /// yields every coordinate `(x, z)` with `|x - centre_x| + |z - centre_z| <= max_radius`.
    ///
    /// Negative `max_radius` produces a degenerate iterator that yields
    /// nothing (T-04-01-02 mitigation).
    pub fn new(centre_x: i32, centre_z: i32, max_radius: i32) -> Self {
        let done = max_radius < 0;
        Self {
            centre_x,
            centre_z,
            max_radius,
            current_radius: 0,
            yielded_centre: false,
            shell: Shell2D {
                radius: 1,
                side: 0,
                step: 0,
            },
            done,
        }
    }

    /// Yield the next `(dx, dz)` offset in the current shell (radius >= 1).
    /// Returns `None` when the current shell is exhausted; the caller
    /// advances `current_radius` and resets `shell`.
    fn next_in_shell(&mut self) -> Option<(i32, i32)> {
        let r = self.shell.radius;
        loop {
            if self.shell.side >= 4 {
                return None;
            }
            // Each side has exactly `r` points (step in 0..r).
            if self.shell.step >= r {
                self.shell.side += 1;
                self.shell.step = 0;
                continue;
            }
            let s = self.shell.step;
            let (dx, dz) = match self.shell.side {
                // Side 0: (r, 0) -> (0, r); dx = r - s, dz = s.
                0 => (r - s, s),
                // Side 1: (0, r) -> (-r, 0); dx = -s, dz = r - s.
                1 => (-s, r - s),
                // Side 2: (-r, 0) -> (0, -r); dx = -(r - s), dz = -s.
                2 => (-(r - s), -s),
                // Side 3: (0, -r) -> (r, 0); dx = s, dz = -(r - s).
                _ => (s, -(r - s)),
            };
            self.shell.step += 1;
            return Some((dx, dz));
        }
    }
}

impl Iterator for OutwardIterator2D {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if !self.yielded_centre {
            self.yielded_centre = true;
            if self.max_radius == 0 {
                self.done = true;
            }
            return Some((self.centre_x, self.centre_z));
        }
        loop {
            if self.current_radius >= self.max_radius
                && self.shell.side >= 4
            {
                self.done = true;
                return None;
            }
            if self.shell.side >= 4 {
                self.current_radius = self
                    .current_radius
                    .saturating_add(1);
                if self.current_radius > self.max_radius {
                    self.done = true;
                    return None;
                }
                self.shell = Shell2D {
                    radius: self.current_radius,
                    side: 0,
                    step: 0,
                };
            }
            // Ensure shell.radius matches current_radius after a fresh
            // iterator that hasn't yet advanced into a shell.
            if self.current_radius == 0 {
                self.current_radius = 1;
                self.shell = Shell2D {
                    radius: 1,
                    side: 0,
                    step: 0,
                };
            }
            if let Some((dx, dz)) = self.next_in_shell() {
                let x = self.centre_x.saturating_add(dx);
                let z = self.centre_z.saturating_add(dz);
                return Some((x, z));
            }
        }
    }
}

/// 3D Manhattan-distance-monotonic outward iterator. Yields `IVec3`
/// offsets relative to the construction centre. Octahedral shells
/// `|dx|+|dy|+|dz| == r` are emitted for `r = 0, 1, ..., max_radius`.
///
/// Within a shell, iteration walks `y` from `-r` to `r`; at each `y` an
/// inner 2D ring with sub-radius `r - |y|` is emitted. The 2D ring is
/// emitted by walking its four sides; degenerate `y = ±r` collapses to
/// a single point.
///
/// Allocation-free: state is a fixed handful of `i32`/`u8` fields.
#[derive(Clone, Debug)]
pub struct OutwardIterator3D {
    centre: IVec3,
    max_radius: i32,
    current_radius: i32,
    yielded_centre: bool,
    y_offset: i32,
    inner: Shell2D,
    inner_remaining: bool,
    done: bool,
}

impl OutwardIterator3D {
    /// Construct a new iterator centred at `centre` that yields every
    /// `IVec3` `p` with `(p - centre).x.abs() + .y.abs() + .z.abs() <= max_radius`.
    ///
    /// Negative `max_radius` produces a degenerate iterator that yields
    /// nothing.
    pub fn new(centre: IVec3, max_radius: i32) -> Self {
        let done = max_radius < 0;
        Self {
            centre,
            max_radius,
            current_radius: 0,
            yielded_centre: false,
            y_offset: 0,
            inner: Shell2D {
                radius: 0,
                side: 0,
                step: 0,
            },
            inner_remaining: false,
            done,
        }
    }

    fn start_y_layer(&mut self) {
        let r = self.current_radius;
        let sub = r - self.y_offset.abs();
        if sub == 0 {
            // Degenerate "ring": one point at (0, y, 0).
            self.inner = Shell2D {
                radius: 0,
                side: 0,
                step: 0,
            };
            self.inner_remaining = true;
        } else {
            self.inner = Shell2D {
                radius: sub,
                side: 0,
                step: 0,
            };
            self.inner_remaining = true;
        }
    }

    fn next_in_inner_ring(&mut self) -> Option<(i32, i32)> {
        let r = self.inner.radius;
        if r == 0 {
            if self.inner_remaining {
                self.inner_remaining = false;
                return Some((0, 0));
            }
            return None;
        }
        loop {
            if self.inner.side >= 4 {
                return None;
            }
            if self.inner.step >= r {
                self.inner.side += 1;
                self.inner.step = 0;
                continue;
            }
            let s = self.inner.step;
            let (dx, dz) = match self.inner.side {
                0 => (r - s, s),
                1 => (-s, r - s),
                2 => (-(r - s), -s),
                _ => (s, -(r - s)),
            };
            self.inner.step += 1;
            return Some((dx, dz));
        }
    }
}

impl Iterator for OutwardIterator3D {
    type Item = IVec3;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if !self.yielded_centre {
            self.yielded_centre = true;
            if self.max_radius == 0 {
                self.done = true;
            }
            return Some(self.centre);
        }
        loop {
            // Need to ensure current_radius >= 1.
            if self.current_radius == 0 {
                self.current_radius = 1;
                self.y_offset = -self.current_radius;
                self.start_y_layer();
            }
            // Drain the current 2D ring.
            if let Some((dx, dz)) = self.next_in_inner_ring() {
                let p = self.centre
                    + IVec3::new(dx, self.y_offset, dz);
                return Some(p);
            }
            // Inner ring exhausted: advance y within current shell.
            if self.y_offset < self.current_radius {
                self.y_offset = self.y_offset.saturating_add(1);
                self.start_y_layer();
                continue;
            }
            // Whole shell exhausted: advance radius.
            if self.current_radius >= self.max_radius {
                self.done = true;
                return None;
            }
            self.current_radius = self.current_radius.saturating_add(1);
            self.y_offset = -self.current_radius;
            self.start_y_layer();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manhattan_2d(centre: (i32, i32), point: (i32, i32)) -> i32 {
        (point.0 - centre.0).abs() + (point.1 - centre.1).abs()
    }

    fn manhattan_3d(centre: IVec3, point: IVec3) -> i32 {
        (point.x - centre.x).abs()
            + (point.y - centre.y).abs()
            + (point.z - centre.z).abs()
    }

    #[test]
    fn outward_2d_yields_centre_first() {
        let mut it = OutwardIterator2D::new(5, 7, 3);
        assert_eq!(it.next(), Some((5, 7)));
    }

    #[test]
    fn outward_2d_zero_radius() {
        let mut it = OutwardIterator2D::new(0, 0, 0);
        assert_eq!(it.next(), Some((0, 0)));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn outward_2d_negative_radius_yields_nothing() {
        let mut it = OutwardIterator2D::new(0, 0, -1);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn outward_2d_shell_monotonic() {
        let centre = (0, 0);
        let it = OutwardIterator2D::new(centre.0, centre.1, 5);
        let mut last_r = -1;
        for (x, z) in it {
            let r = manhattan_2d(centre, (x, z));
            assert!(
                r >= last_r,
                "shell out of order: last_r={} r={} at ({}, {})",
                last_r,
                r,
                x,
                z
            );
            last_r = r;
        }
    }

    #[test]
    fn outward_2d_total_count() {
        // Total = 1 + sum_{k=1..=r} (4k) = 1 + 2r(r+1).
        for r in 0..=8 {
            let count = OutwardIterator2D::new(0, 0, r).count() as i32;
            let expected = 1 + 2 * r * (r + 1);
            assert_eq!(
                count, expected,
                "count mismatch at max_radius={}",
                r
            );
        }
    }

    #[test]
    fn outward_2d_no_duplicates_small() {
        use std::collections::HashSet;
        let centre = (3, -2);
        let mut seen = HashSet::new();
        for p in OutwardIterator2D::new(centre.0, centre.1, 4) {
            assert!(seen.insert(p), "duplicate point: {:?}", p);
        }
    }

    #[test]
    fn outward_2d_covers_disc() {
        use std::collections::HashSet;
        let centre = (0, 0);
        let r = 4;
        let mut seen = HashSet::new();
        for p in OutwardIterator2D::new(centre.0, centre.1, r) {
            seen.insert(p);
        }
        // Every (x, z) with |x|+|z| <= r must be present.
        for dx in -r..=r {
            for dz in -r..=r {
                if dx.abs() + dz.abs() <= r {
                    assert!(
                        seen.contains(&(dx, dz)),
                        "missing point ({}, {})",
                        dx,
                        dz
                    );
                }
            }
        }
    }

    #[test]
    fn outward_3d_yields_centre_first() {
        let mut it = OutwardIterator3D::new(IVec3::new(1, 2, 3), 2);
        assert_eq!(it.next(), Some(IVec3::new(1, 2, 3)));
    }

    #[test]
    fn outward_3d_zero_radius() {
        let mut it = OutwardIterator3D::new(IVec3::ZERO, 0);
        assert_eq!(it.next(), Some(IVec3::ZERO));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn outward_3d_negative_radius_yields_nothing() {
        let mut it = OutwardIterator3D::new(IVec3::ZERO, -3);
        assert_eq!(it.next(), None);
    }

    #[test]
    fn outward_3d_shell_monotonic() {
        let centre = IVec3::ZERO;
        let mut last_r = -1;
        for p in OutwardIterator3D::new(centre, 4) {
            let r = manhattan_3d(centre, p);
            assert!(
                r >= last_r,
                "shell out of order: last_r={} r={} at {:?}",
                last_r,
                r,
                p
            );
            last_r = r;
        }
    }

    #[test]
    fn outward_3d_covers_octahedron() {
        use std::collections::HashSet;
        let centre = IVec3::ZERO;
        let r = 3;
        let mut seen = HashSet::new();
        for p in OutwardIterator3D::new(centre, r) {
            assert!(seen.insert(p), "duplicate {:?}", p);
        }
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if dx.abs() + dy.abs() + dz.abs() <= r {
                        assert!(
                            seen.contains(&IVec3::new(dx, dy, dz)),
                            "missing {:?}",
                            IVec3::new(dx, dy, dz)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn outward_2d_saturating_extreme_coords() {
        // Centre at i32::MAX with positive radius should not panic; the
        // saturating arithmetic clamps but never wraps.
        let it = OutwardIterator2D::new(i32::MAX, i32::MAX, 3);
        let _: Vec<_> = it.collect();
    }
}
