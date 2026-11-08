use mcrs_minecraft_brain::Applied;
use mcrs_minecraft_brain::BlockPos;
use mcrs_minecraft_brain::PathIntent;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_brain::Progress;

use crate::ControlFlags;
use crate::ControlInput;
use crate::PathEnd;

/// The navigation of a mob with a straight route of one node, its target (PATH-FOL-001 to
/// PATH-FOL-005). A path search later gives a list of nodes in place of the one node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Follower {
    applied: Applied,
    route: Option<Route>,
    ticks: u64,
    stuck: StuckCheck,
    timeout: Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Route {
    node: BlockPos,
    floor: f64,
    speed_modifier: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct StuckCheck {
    tick: u64,
    position: Point,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Timeout {
    node: Option<BlockPos>,
    time: u64,
    timer: u64,
    limit: f64,
}

/// What the follower gives the move control in one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FollowerStep {
    pub wanted: Option<Wanted>,
    pub ended: Option<PathEnd>,
}

/// The wanted position of the move control and the speed modifier of the path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wanted {
    pub position: Point,
    pub speed_modifier: f32,
}

const STUCK_TICKS: u64 = 100;

impl Follower {
    /// The last applied path intent and whether its path runs, for the view of the brain.
    #[must_use]
    pub const fn applied(&self) -> Applied {
        self.applied
    }

    /// The mob has a path that is not done.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.route.is_some()
    }

    /// Applies a path intent of a decision; a new path resets the stuck check and the timeout.
    pub fn apply(&mut self, intent: &PathIntent, position: Point) {
        match *intent {
            PathIntent::Walk {
                ticket,
                target,
                speed_modifier,
            } => {
                self.route = Some(Route {
                    node: target.block(),
                    floor: target.y,
                    speed_modifier,
                });
                self.applied = Applied {
                    ticket,
                    progress: Progress::Running,
                };
                self.stuck = StuckCheck {
                    tick: self.ticks,
                    position,
                };
                self.timeout = Timeout::default();
            }
            PathIntent::Clear { ticket } => {
                self.route = None;
                self.applied = Applied {
                    ticket,
                    progress: Progress::Ended,
                };
            }
        }
    }

    /// One tick of the follower before the move control: the advance at the node, the stuck
    /// check and the timeout, then the wanted position.
    pub fn step(&mut self, input: &ControlInput) -> FollowerStep {
        let Some(route) = self.route else {
            return FollowerStep::default();
        };
        self.ticks = self.ticks.saturating_add(1);
        if self.reached(input, route.node) {
            return self.end(PathEnd::Done);
        }
        if self.ticks.saturating_sub(self.stuck.tick) > STUCK_TICKS {
            let speed = input.speed;
            let scale = if speed >= 1.0 { speed } else { speed * speed };
            let reach = f64::from(scale * STUCK_TICKS as f32 * 0.25);
            let moved = input.position.distance_squared(self.stuck.position);
            self.stuck = StuckCheck {
                tick: self.ticks,
                position: input.position,
            };
            if moved < reach * reach {
                return self.end(PathEnd::Stuck);
            }
        }
        if self.timed_out(input, route.node) {
            self.timeout = Timeout::default();
            return self.end(PathEnd::TimedOut);
        }
        let side = (input.body.width + 1.0) as i32;
        let offset = f64::from(side) * 0.5;
        FollowerStep {
            wanted: Some(Wanted {
                position: Point::new(
                    f64::from(route.node.x) + offset,
                    route.floor,
                    f64::from(route.node.z) + offset,
                ),
                speed_modifier: route.speed_modifier,
            }),
            ended: None,
        }
    }

    fn reached(&self, input: &ControlInput, node: BlockPos) -> bool {
        let flags = input.flags;
        let position = input.position;
        let can_update = flags.contains(ControlFlags::ON_GROUND)
            || flags.contains(ControlFlags::IN_LIQUID)
            || flags.contains(ControlFlags::PASSENGER);
        if !can_update {
            let block = position.block();
            return position.y > f64::from(node.y) && block.x == node.x && block.z == node.z;
        }
        let width = input.body.width;
        let limit = f64::from(if width > 0.75 {
            width / 2.0
        } else {
            0.75 - width / 2.0
        });
        (position.x - (f64::from(node.x) + 0.5)).abs() < limit
            && (position.z - (f64::from(node.z) + 0.5)).abs() < limit
            && (position.y - f64::from(node.y)).abs() < 1.0
    }

    fn timed_out(&mut self, input: &ControlInput, node: BlockPos) -> bool {
        let time = input.tick;
        if self.timeout.node == Some(node) {
            self.timeout.timer = self
                .timeout
                .timer
                .saturating_add(time.saturating_sub(self.timeout.time));
        } else {
            self.timeout.node = Some(node);
            let distance = input.position.distance_squared(node.bottom_centre()).sqrt();
            let speed = f64::from(input.speed);
            self.timeout.limit = if speed > 0.0 {
                distance / speed * 20.0
            } else {
                0.0
            };
        }
        self.timeout.time = time;
        self.timeout.limit > 0.0 && self.timeout.timer as f64 > 3.0 * self.timeout.limit
    }

    fn end(&mut self, end: PathEnd) -> FollowerStep {
        self.route = None;
        self.applied.progress = Progress::Ended;
        FollowerStep {
            wanted: None,
            ended: Some(end),
        }
    }
}
