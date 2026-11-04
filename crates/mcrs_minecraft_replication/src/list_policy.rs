use glam::DVec3;
use mcrs_engine_muxer::EntityId;

/// A player that a [`ListPolicy`] can list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub player: EntityId,
    pub position: DVec3,
}

/// What a [`ListPolicy`] chooses from.
#[derive(Clone, Copy, Debug)]
pub struct ListRequest<'a> {
    pub viewer: Candidate,
    /// Every player of the world, the viewer included, sorted by player.
    pub candidates: &'a [Candidate],
    pub limit: usize,
}

/// Chooses the players that the list of a viewer shows besides the viewer: at most `limit`
/// candidates other than the viewer, written to the vector, which starts empty.
#[derive(Clone, Copy, Debug)]
pub struct ListPolicy(pub fn(&ListRequest<'_>, &mut Vec<EntityId>));

impl Default for ListPolicy {
    fn default() -> Self {
        Self(nearest)
    }
}

struct Ranked {
    distance: f64,
    player: EntityId,
}

/// The policy that lists the nearest players. Equal distances go to the lower player.
pub fn nearest(request: &ListRequest<'_>, listed: &mut Vec<EntityId>) {
    let mut ranked: Vec<Ranked> = request
        .candidates
        .iter()
        .filter(|candidate| candidate.player != request.viewer.player)
        .map(|candidate| Ranked {
            distance: candidate.position.distance_squared(request.viewer.position),
            player: candidate.player,
        })
        .collect();
    let order = |first: &Ranked, second: &Ranked| {
        first
            .distance
            .total_cmp(&second.distance)
            .then(first.player.cmp(&second.player))
    };
    if ranked.len() > request.limit {
        ranked.select_nth_unstable_by(request.limit, order);
        ranked.truncate(request.limit);
    }
    listed.extend(ranked.iter().map(|ranked| ranked.player));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(index: u64, x: f64) -> Candidate {
        Candidate {
            player: EntityId(index),
            position: DVec3::new(x, 64.0, 0.0),
        }
    }

    #[test]
    fn nearest_lists_the_closest_players_other_than_the_viewer() {
        let candidates = [
            candidate(1, 0.0),
            candidate(2, 50.0),
            candidate(3, -2.0),
            candidate(4, 5.0),
        ];
        let mut listed = Vec::new();

        nearest(
            &ListRequest {
                viewer: candidates[0],
                candidates: &candidates,
                limit: 2,
            },
            &mut listed,
        );

        listed.sort_unstable();
        assert_eq!(listed, [EntityId(3), EntityId(4)]);
    }
}
