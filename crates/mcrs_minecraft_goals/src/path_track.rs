use glam::DVec3;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::PathIntent;
use mcrs_minecraft_brain::Progress;
use mcrs_minecraft_brain::Ticket;
use mcrs_minecraft_brain::Turn;
use mcrs_voxel::BlockPos;

use crate::GoalWorld;

/// The one path of a mob as its brain sees it: the ticket of the last path intent and whether
/// that intent gave a route. Each goal reads and writes this path (GOAL-NAV-005).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PathTrack {
    issued: Ticket,
    route: Route,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum Route {
    #[default]
    None,
    Walking,
}

impl PathTrack {
    /// The mob has a path that is not done: the last intent has a route, and it is pending or the
    /// host still runs it.
    #[must_use]
    pub fn is_running(&self, view: &MobView) -> bool {
        match self.route {
            Route::None => false,
            Route::Walking => {
                view.path.ticket != self.issued || view.path.progress == Progress::Running
            }
        }
    }

    /// A request to the point `target` (GOAL-NAV-001); no route gives no path.
    pub fn request<W: GoalWorld + ?Sized>(
        &mut self,
        turn: &Turn<'_, W>,
        target: DVec3,
        speed_modifier: f32,
        intents: &mut Intents,
    ) {
        self.issued = self.issued.next();
        let ticket = self.issued;
        intents.path = Some(
            match turn.world.route(turn.view.position, BlockPos::from(target)) {
                Some(end) => {
                    self.route = Route::Walking;
                    PathIntent::Walk {
                        ticket,
                        target: end,
                        speed_modifier,
                    }
                }
                None => {
                    self.route = Route::None;
                    PathIntent::Clear { ticket }
                }
            },
        );
    }

    pub fn clear(&mut self, intents: &mut Intents) {
        self.issued = self.issued.next();
        self.route = Route::None;
        intents.path = Some(PathIntent::Clear {
            ticket: self.issued,
        });
    }
}
