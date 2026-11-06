use crate::player_list::Member;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use std::collections::BTreeSet;

struct Ranked {
    distance: f64,
    player: EntityId,
}

/// The at most `limit` members of `roster` nearest `position`, `viewer` aside. Equal distances
/// go to the lower player.
pub(crate) fn nearest(
    viewer: EntityId,
    position: DVec3,
    roster: &[Member<'_>],
    limit: usize,
) -> BTreeSet<EntityId> {
    let mut ranked: Vec<Ranked> = roster
        .iter()
        .filter(|member| member.player != viewer)
        .map(|member| Ranked {
            distance: member.position.distance_squared(position),
            player: member.player,
        })
        .collect();
    let order = |first: &Ranked, second: &Ranked| {
        first
            .distance
            .total_cmp(&second.distance)
            .then(first.player.cmp(&second.player))
    };
    if ranked.len() > limit {
        ranked.select_nth_unstable_by(limit, order);
        ranked.truncate(limit);
    }
    ranked.iter().map(|ranked| ranked.player).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list_packets::Profile;
    use mcrs_minecraft_game_mode::GameMode;
    use uuid::Uuid;

    fn member(index: u64, x: f64) -> Member<'static> {
        Member {
            player: EntityId(index),
            position: DVec3::new(x, 64.0, 0.0),
            profile: Profile {
                id: Uuid::from_u128(u128::from(index)),
                username: "player",
                properties: &[],
            },
            game_mode: GameMode::default(),
            latency: 0,
        }
    }

    #[test]
    fn nearest_lists_the_closest_players_other_than_the_viewer() {
        let roster = [
            member(1, 0.0),
            member(2, 50.0),
            member(3, -2.0),
            member(4, 5.0),
        ];

        let listed = nearest(EntityId(1), DVec3::new(0.0, 64.0, 0.0), &roster, 2);

        assert_eq!(listed, BTreeSet::from([EntityId(3), EntityId(4)]));
    }
}
