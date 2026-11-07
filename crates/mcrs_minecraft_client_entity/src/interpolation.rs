use std::collections::VecDeque;

use crate::angle;
use crate::path::{PacketPath, PathStep};
use crate::placement::{Placement, Rotation};

/// A change of position from another cause smaller than this, squared, does not move the path.
const MIN_FOLLOW_DISTANCE_SQUARED: f64 = 1e-5;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    end: Placement,
    weight: f32,
}

/// What a start of an interpolation did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Start {
    /// The active interpolation already ended at the target.
    Unchanged,
    /// The path has new segments.
    Extended,
    /// The interpolation length is 0: the entity jumps to this placement and the path is empty.
    Snap(Placement),
}

/// The path that a client entity still travels towards the placements that the server sent.
///
/// Each start gives the remaining path `length` ticks. Each tick moves the entity by an even
/// share of the remaining weight, so a segment can end in the middle of a tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Interpolation {
    /// Where the interpolation put the entity in its latest tick.
    point: Placement,
    segments: VecDeque<Segment>,
    remaining_time: f32,
    total_weight: f32,
    /// The end point of the latest start. It stays when a start empties the path.
    target: Placement,
    /// Where the entity was at the end of the latest start or step. A step measures the
    /// movement from other causes from here.
    settled: Placement,
}

impl Interpolation {
    /// Whether ticks remain in the interpolation.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.remaining_time > 0.0
    }

    /// Whether segments of the path remain.
    #[must_use]
    pub fn has_path(&self) -> bool {
        !self.segments.is_empty()
    }

    fn clear(&mut self) {
        self.segments.clear();
        self.remaining_time = 0.0;
        self.total_weight = 0.0;
    }

    /// Adds the movement of one packet to the path. `current` is where the entity is and
    /// `length` the interpolation length of its type, in ticks. A missing path or rotation
    /// keeps that of the target of an active interpolation, or else of `current`.
    pub fn start(
        &mut self,
        current: Placement,
        path: Option<PacketPath>,
        rotation: Option<Rotation>,
        length: u32,
    ) -> Start {
        let reference = if self.is_active() {
            self.target
        } else {
            current
        };
        let path = path.unwrap_or(PacketPath::Linear(reference.position));
        let rotation = rotation.unwrap_or(reference.rotation());
        let end = Placement::new(
            path.end().unwrap_or(reference.position),
            rotation.yaw,
            rotation.pitch,
        );
        if length == 0 {
            self.clear();
            self.settled = end;
            return Start::Snap(end);
        }
        if self.is_active() && self.target == end {
            return Start::Unchanged;
        }
        self.settled = current;
        if let PacketPath::Stepped {
            start_immediately: true,
            ..
        } = path
        {
            self.clear();
        }
        if !self.is_active() {
            self.point = current;
        }
        let length = length as f32;
        match path {
            PacketPath::Stepped { steps, .. } if end.position != self.target.position => {
                self.push_steps(&steps, rotation);
            }
            _ => self.push(end, length),
        }
        self.target = end;
        self.remaining_time = length;
        Start::Extended
    }

    fn push(&mut self, end: Placement, weight: f32) {
        self.segments.push_back(Segment { end, weight });
        self.total_weight += weight;
    }

    /// One segment for each step. The rotation turns from the target of the previous start in
    /// proportion to the ticks that the steps reach.
    fn push_steps(&mut self, steps: &[PathStep], rotation: Rotation) {
        let from = self.target.rotation();
        let turns = rotation != from;
        let total: u64 = steps.iter().map(|step| u64::from(step.ticks)).sum();
        let mut reached: u64 = 0;
        for step in steps {
            reached = reached.saturating_add(u64::from(step.ticks));
            let rotation = if turns {
                // The share of a path with no ticks is undefined; the full turn is our choice.
                let share = if total == 0 {
                    1.0
                } else {
                    reached as f32 / total as f32
                };
                Rotation {
                    yaw: angle::alerp(share, from.yaw, rotation.yaw),
                    pitch: angle::lerp(share, from.pitch, rotation.pitch),
                }
            } else {
                rotation
            };
            self.push(
                Placement::new(step.position, rotation.yaw, rotation.pitch),
                step.ticks as f32,
            );
        }
    }

    /// Moves the entity at `current` one tick along the path, and gives its new placement.
    /// `tick_speed` is the factor of a server that runs faster than 20 ticks a second.
    pub fn step(&mut self, current: Placement, tick_speed: f32) -> Placement {
        if !self.is_active() {
            self.segments.clear();
            self.total_weight = 0.0;
            self.settled = current;
            return current;
        }
        self.follow(current);
        let mut distance = tick_speed * self.total_weight / self.remaining_time;
        self.total_weight -= distance;
        self.remaining_time -= tick_speed;
        while let Some(segment) = self.segments.front_mut() {
            if distance < segment.weight {
                let fraction = distance / segment.weight;
                if fraction > 0.0 {
                    self.point = self.point.towards(&segment.end, fraction);
                }
                segment.weight -= distance;
                break;
            }
            distance -= segment.weight;
            self.point = segment.end;
            self.segments.pop_front();
        }
        if self.segments.is_empty() {
            self.clear();
        }
        self.settled = self.point;
        self.point
    }

    /// Moves the target, the rest of the path and the point of the interpolation with the
    /// entity when another cause moved or turned it since the latest start or step. A client
    /// with no block collision shapes finds no collision at the moved target.
    fn follow(&mut self, current: Placement) {
        let shift = current.position - self.settled.position;
        let moves = shift.length_squared() > MIN_FOLLOW_DISTANCE_SQUARED;
        let yaw_change = current.yaw - self.settled.yaw;
        let pitch_change = current.pitch - self.settled.pitch;
        let points = self
            .segments
            .iter_mut()
            .map(|segment| &mut segment.end)
            .chain([&mut self.point, &mut self.target]);
        for point in points {
            if moves {
                point.position += shift;
            }
            point.yaw += yaw_change;
            point.pitch += pitch_change;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn at(z: f64) -> Placement {
        Placement::new(DVec3::new(0.0, 0.0, z), 0.0, 0.0)
    }

    fn linear(z: f64) -> Option<PacketPath> {
        Some(PacketPath::Linear(DVec3::new(0.0, 0.0, z)))
    }

    fn stepped(steps: &[(f64, u32)], start_immediately: bool) -> Option<PacketPath> {
        let steps = steps
            .iter()
            .map(|&(z, ticks)| PathStep {
                position: DVec3::new(0.0, 0.0, z),
                ticks,
            })
            .collect();
        Some(PacketPath::Stepped {
            steps,
            start_immediately,
        })
    }

    fn walk(
        interpolation: &mut Interpolation,
        mut current: Placement,
        ticks: usize,
    ) -> Vec<Placement> {
        let mut placements = Vec::with_capacity(ticks);
        for _ in 0..ticks {
            current = interpolation.step(current, 1.0);
            placements.push(current);
        }
        placements
    }

    fn zs(placements: &[Placement]) -> Vec<f64> {
        placements
            .iter()
            .map(|placement| placement.position.z)
            .collect()
    }

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-5,
                "{actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn a_linear_move_takes_the_interpolation_length_in_equal_steps() {
        let mut interpolation = Interpolation::default();
        let start = interpolation.start(at(0.0), linear(3.0), None, 3);
        assert_eq!(start, Start::Extended);
        assert_close(
            &zs(&walk(&mut interpolation, at(0.0), 4)),
            &[1.0, 2.0, 3.0, 3.0],
        );
        assert!(!interpolation.is_active());
        assert!(!interpolation.has_path());
    }

    #[test]
    fn each_tick_moves_an_even_share_of_the_weight_across_segments() {
        let mut interpolation = Interpolation::default();
        interpolation.start(
            at(0.0),
            stepped(&[(1.0, 1), (3.0, 2), (4.0, 3)], false),
            None,
            3,
        );
        // A weight of 6 over 3 ticks: 2 for each tick, so a tick ends inside a segment.
        assert_close(
            &zs(&walk(&mut interpolation, at(0.0), 3)),
            &[2.0, 3.0 + 1.0 / 3.0, 4.0],
        );
        assert!(!interpolation.has_path());
    }

    #[test]
    fn a_start_during_a_path_spreads_the_rest_and_the_new_segment_over_the_length() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(3.0), None, 3);
        let current = interpolation.step(at(0.0), 1.0);
        assert_close(&[current.position.z], &[1.0]);
        interpolation.start(current, linear(6.0), None, 3);
        // A weight of 2 is left on the first segment and 3 on the new one: 5/3 for each tick.
        assert_close(
            &zs(&walk(&mut interpolation, current, 3)),
            &[8.0 / 3.0, 13.0 / 3.0, 6.0],
        );
    }

    #[test]
    fn start_immediately_empties_the_path_but_keeps_the_target() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(30.0), None, 3);
        let turn = Some(Rotation {
            yaw: 90.0,
            pitch: 0.0,
        });
        interpolation.start(at(0.0), stepped(&[(10.0, 1), (30.0, 1)], true), turn, 3);
        // The end is the old target, so one segment of the full length replaces the steps.
        let placements = walk(&mut interpolation, at(0.0), 3);
        assert_close(&zs(&placements), &[10.0, 20.0, 30.0]);
        let yaws: Vec<f64> = placements
            .iter()
            .map(|placement| f64::from(placement.yaw))
            .collect();
        assert_close(&yaws, &[30.0, 60.0, 90.0]);
    }

    #[test]
    fn start_immediately_starts_from_where_the_entity_is() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(30.0), None, 3);
        interpolation.start(at(0.0), stepped(&[(-3.0, 3)], true), None, 3);
        assert_close(
            &zs(&walk(&mut interpolation, at(0.0), 3)),
            &[-1.0, -2.0, -3.0],
        );
    }

    #[test]
    fn stepped_rotation_turns_in_proportion_to_the_ticks() {
        let mut interpolation = Interpolation::default();
        interpolation.start(
            at(0.0),
            stepped(&[(1.0, 1), (2.0, 3)], false),
            Some(Rotation {
                yaw: 200.0,
                pitch: 40.0,
            }),
            4,
        );
        let first = interpolation.step(at(0.0), 1.0);
        assert_eq!(
            first.yaw, -40.0,
            "a quarter of the turn to 200, the short way"
        );
        assert_eq!(first.pitch, 10.0);
    }

    #[test]
    fn steps_with_no_ticks_take_the_full_turn_at_once() {
        let mut interpolation = Interpolation::default();
        interpolation.start(
            at(0.0),
            stepped(&[(1.0, 0), (2.0, 0)], false),
            Some(Rotation {
                yaw: 90.0,
                pitch: 0.0,
            }),
            3,
        );
        let first = interpolation.step(at(0.0), 1.0);
        assert_eq!(first, Placement::new(DVec3::new(0.0, 0.0, 2.0), 90.0, 0.0));
        assert!(!interpolation.is_active());
    }

    #[test]
    fn an_unchanged_target_does_not_restart() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(3.0), None, 3);
        let current = interpolation.step(at(0.0), 1.0);
        assert_eq!(
            interpolation.start(current, linear(3.0), None, 3),
            Start::Unchanged
        );
        assert_close(&zs(&walk(&mut interpolation, current, 2)), &[2.0, 3.0]);
    }

    #[test]
    fn the_path_follows_a_move_from_another_cause() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(3.0), None, 3);
        let current = interpolation.step(at(0.0), 1.0);
        let lifted = Placement::new(current.position + DVec3::Y * 10.0, 0.0, 0.0);
        let next = interpolation.step(lifted, 1.0);
        assert_close(&[next.position.y, next.position.z], &[10.0, 2.0]);
        let nudged = Placement::new(next.position + DVec3::Y * 1e-3, 0.0, 0.0);
        let last = interpolation.step(nudged, 1.0);
        assert_eq!(
            last.position,
            DVec3::new(0.0, 10.0, 3.0),
            "a change below the threshold does not move the path"
        );
    }

    #[test]
    fn a_move_before_a_start_is_not_followed() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(3.0), None, 3);
        let current = interpolation.step(at(0.0), 1.0);
        let lifted = Placement::new(current.position + DVec3::Y * 5.0, 0.0, 0.0);
        interpolation.start(lifted, linear(6.0), None, 3);
        let placements = walk(&mut interpolation, lifted, 3);
        assert_close(&zs(&placements), &[8.0 / 3.0, 13.0 / 3.0, 6.0]);
        assert_eq!(
            placements[0].position.y, 0.0,
            "the start measures later moves from the lifted position, so the lift is not followed"
        );
    }

    #[test]
    fn a_followed_move_also_moves_the_target_of_the_next_start() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(3.0), None, 3);
        let current = interpolation.step(at(0.0), 1.0);
        let lifted = Placement::new(current.position + DVec3::Y * 10.0, 0.0, 0.0);
        let current = interpolation.step(lifted, 1.0);
        let turn = Some(Rotation {
            yaw: 90.0,
            pitch: 0.0,
        });
        interpolation.start(current, None, turn, 3);
        let placements = walk(&mut interpolation, current, 3);
        let heights: Vec<f64> = placements
            .iter()
            .map(|placement| placement.position.y)
            .collect();
        assert_close(&heights, &[10.0, 10.0, 10.0]);
        assert_eq!(
            placements[2],
            Placement::new(DVec3::new(0.0, 10.0, 3.0), 90.0, 0.0)
        );
    }

    #[test]
    fn a_turn_from_another_cause_turns_the_rest_of_the_path() {
        let mut interpolation = Interpolation::default();
        interpolation.start(at(0.0), linear(3.0), None, 3);
        let current = interpolation.step(at(0.0), 1.0);
        let turned = Placement::new(current.position, 0.5, -0.25);
        let placements = walk(&mut interpolation, turned, 2);
        assert_eq!(
            placements[1],
            Placement::new(DVec3::new(0.0, 0.0, 3.0), 0.5, -0.25),
            "a turn has no threshold"
        );
    }

    #[test]
    fn length_zero_snaps() {
        let mut interpolation = Interpolation::default();
        let start = interpolation.start(at(0.0), linear(3.0), None, 0);
        assert_eq!(start, Start::Snap(at(3.0)));
        assert!(!interpolation.is_active());
    }
}
