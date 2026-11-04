use std::collections::BTreeMap;

use crate::muxer_settings::MuxerSettings;
use crate::subscription::{LAYERS, LayerState, Subscription, layer_ranks};
use crate::tier::Tier;
use crate::tile_key::TileKey;

/// The farthest distance of each tier but the last, in the units of the game.
const REACH: [f64; 3] = [32.0, 64.0, 128.0];

/// A tile leaves a tier only this far behind the distance of the tier.
const REACH_HYSTERESIS: f64 = 8.0;

/// The ticks of a second, in which the budget counts its bytes.
pub(crate) const TICKS_PER_SECOND: u64 = 20;

/// Over the budget, the planner lowers one layer each this many ticks.
const LOWER_EVERY_TICKS: u32 = 10;

/// Below 70% of the budget, the planner raises one layer each this many ticks.
const RAISE_EVERY_TICKS: u32 = 40;

/// A point on X and Z, in the units of the game.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) z: f64,
}

/// What the planner reads of one viewer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewerState<'a> {
    pub(crate) settings: &'a MuxerSettings,
    pub(crate) position: Point,
    pub(crate) subscriptions: &'a BTreeMap<TileKey, Subscription>,
}

/// A tile in the view of a viewer and the number of its subjects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileHeader {
    pub(crate) tile: TileKey,
    pub(crate) count: usize,
}

/// The bytes that a viewer received lately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Measured {
    /// The moving average of the bytes of a tick.
    pub(crate) average: u64,
    /// The ticks in a row with the average above the budget.
    pub(crate) over: u32,
    /// The ticks in a row with the average below 70% of the budget.
    pub(crate) calm: u32,
}

/// The new subscription of a viewer to a tile, or `None` when it receives nothing of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SubscriptionChange {
    pub(crate) tile: TileKey,
    pub(crate) subscription: Option<Subscription>,
}

/// A tile in the order of the plan.
#[derive(Clone, Copy, Debug)]
struct Placed {
    tile: TileKey,
    distance: f64,
    count: usize,
}

/// The subscriptions of a viewer for the next tick: which layers of the tiles in `headers` it
/// receives and at which tier. It reads no clock and no state but its inputs.
///
/// The tiles go in the order of distance. The cap admits layers in that order while the sum of
/// their subjects is K or less, and keeps a layer that it admitted until the sum is more than
/// 1.25 × K. After the first layer that the cap leaves out, the tiles get layer 0 only, at the
/// farthest tier. When the subjects in view are more than K, a layer gets the tier of its order
/// if that is farther than the tier of its distance. Over the budget, the planner lowers the last
/// layer by one tier. Below 70% of the budget, it raises the lowered layer that waited longest.
#[must_use]
pub(crate) fn plan(
    viewer: &ViewerState<'_>,
    headers: &[TileHeader],
    measured: Measured,
) -> Vec<SubscriptionChange> {
    let width = f64::from(viewer.settings.tile_width.get());
    let mut order: Vec<Placed> = headers
        .iter()
        .map(|header| Placed {
            tile: header.tile,
            distance: distance(viewer.position, header.tile, width),
            count: header.count,
        })
        .collect();
    order.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.tile.cmp(&b.tile))
    });

    let mut next: Vec<Planned> = admit(viewer, &order);
    adjust_to_budget(viewer.settings, &mut next, measured);

    let mut changes = Vec::new();
    for planned in &next {
        let subscription =
            Some(planned.subscription).filter(|subscription| subscription.visible_ranks() > 0);
        if viewer.subscriptions.get(&planned.tile).copied() != subscription {
            changes.push(SubscriptionChange {
                tile: planned.tile,
                subscription,
            });
        }
    }
    for tile in viewer.subscriptions.keys() {
        if !headers.iter().any(|header| header.tile == *tile) {
            changes.push(SubscriptionChange {
                tile: *tile,
                subscription: None,
            });
        }
    }
    changes
}

/// The subscription that the plan gives a tile.
#[derive(Clone, Copy, Debug)]
struct Planned {
    tile: TileKey,
    count: usize,
    subscription: Subscription,
}

impl Planned {
    /// The admitted layers that hold subjects, with their index.
    fn filled_layers(&mut self) -> impl DoubleEndedIterator<Item = &mut LayerState> {
        let count = self.count;
        self.subscription
            .layers
            .iter_mut()
            .enumerate()
            .filter(move |(layer, state)| state.tier.is_some() && layer_ranks(*layer).start < count)
            .map(|(_, state)| state)
    }
}

/// The layers that the cap admits, with the tier of each, in the order of `order`.
fn admit(viewer: &ViewerState<'_>, order: &[Placed]) -> Vec<Planned> {
    let cap = viewer.settings.cap as usize;
    let release = cap.saturating_add(cap / 4);
    let crowded = order.iter().map(|placed| placed.count).sum::<usize>() > cap;
    let mut sum = 0usize;
    let mut beyond = false;
    let mut next = Vec::with_capacity(order.len());
    for placed in order {
        let old = viewer.subscriptions.get(&placed.tile);
        let reach = reach(old.map(|old| old.reach), placed.distance);
        let mut subscription = Subscription {
            reach,
            layers: [LayerState::default(); LAYERS],
        };
        let layers = if beyond { 1 } else { LAYERS };
        for (layer, state) in subscription.layers.iter_mut().enumerate().take(layers) {
            let ranks = layer_ranks(layer);
            let size = placed.count.clamp(ranks.start, ranks.end) - ranks.start;
            let was = old
                .and_then(|old| old.layers.get(layer))
                .copied()
                .unwrap_or_default();
            if size == 0 {
                break;
            }
            let limit = if was.tier.is_some() { release } else { cap };
            if sum.saturating_add(size) > limit {
                beyond = true;
                break;
            }
            let natural = if beyond {
                Tier::FARTHEST
            } else if crowded {
                reach.max(order_tier(sum))
            } else {
                reach
            };
            sum = sum.saturating_add(size);
            let lowered = was
                .lowered
                .min(Tier::FARTHEST.index().saturating_sub(natural.index()) as u8);
            *state = LayerState {
                tier: Some(natural.lowered(lowered)),
                lowered,
                waited: if lowered > 0 { was.waited } else { 0 },
            };
        }
        next.push(Planned {
            tile: placed.tile,
            count: placed.count,
            subscription,
        });
    }
    next
}

/// Lowers or raises one layer of `next` by one tier when the bytes call for it, and lets each
/// lowered layer gather priority.
fn adjust_to_budget(settings: &MuxerSettings, next: &mut [Planned], measured: Measured) {
    let budget = u64::from(settings.budget) / TICKS_PER_SECOND;
    for planned in next.iter_mut() {
        let reach = planned.subscription.reach;
        for layer in &mut planned.subscription.layers {
            if layer.lowered > 0 {
                // A near layer comes back first, but a far one still gains and never starves.
                let weight = Tier::FARTHEST.index() + 1 - reach.index();
                layer.waited = layer.waited.saturating_add(weight as u32);
            }
        }
    }
    let over = measured.average > budget;
    let calm = measured.average.saturating_mul(10) < budget.saturating_mul(7);
    if over && measured.over % LOWER_EVERY_TICKS == 1 {
        let last = next
            .iter_mut()
            .rev()
            .flat_map(|planned| planned.filled_layers().rev())
            .find(|layer| layer.tier.is_some_and(|tier| tier < Tier::FARTHEST));
        if let Some(layer) = last {
            layer.lowered = layer.lowered.saturating_add(1);
            layer.tier = layer.tier.map(|tier| tier.lowered(1));
        }
    } else if calm && measured.calm > 0 && measured.calm.is_multiple_of(RAISE_EVERY_TICKS) {
        let mut longest: Option<&mut LayerState> = None;
        for planned in next.iter_mut() {
            for layer in planned.filled_layers() {
                let waited_longer = longest
                    .as_ref()
                    .is_none_or(|longest| layer.waited > longest.waited);
                if layer.lowered > 0 && waited_longer {
                    longest = Some(layer);
                }
            }
        }
        if let Some(layer) = longest {
            layer.lowered -= 1;
            layer.waited = 0;
            layer.tier = layer.tier.map(Tier::raised);
        }
    }
}

/// The tier of a tile at `distance` that had the tier `old`. A tile enters a nearer tier at its
/// distance, and leaves a tier only past its distance and the hysteresis.
fn reach(old: Option<Tier>, distance: f64) -> Tier {
    let by_distance = Tier::ALL
        .into_iter()
        .zip(REACH)
        .find(|(_, reach)| distance <= *reach)
        .map_or(Tier::FARTHEST, |(tier, _)| tier);
    match old {
        Some(old)
            if by_distance > old
                && REACH
                    .get(old.index())
                    .is_none_or(|reach| distance <= reach + REACH_HYSTERESIS) =>
        {
            old
        }
        _ => by_distance,
    }
}

/// The tier of a layer that comes after `before` subjects of a crowd: its order, as the ranks of
/// a layer.
fn order_tier(before: usize) -> Tier {
    Tier::ALL
        .into_iter()
        .find(|tier| before < layer_ranks(tier.index()).end)
        .unwrap_or(Tier::FARTHEST)
}

/// The distance on X and Z from `point` to the nearest point of `tile`.
fn distance(point: Point, tile: TileKey, width: f64) -> f64 {
    let axis = |value: f64, index: i32| {
        let start = f64::from(index) * width;
        let end = start + width;
        (start - value).max(value - end).max(0.0)
    };
    axis(point.x, tile.x).hypot(axis(point.z, tile.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f64 = 32.0;

    fn settings(cap: u32) -> MuxerSettings {
        MuxerSettings {
            cap,
            ..MuxerSettings::default()
        }
    }

    fn tile(x: i32) -> TileKey {
        TileKey::new(x, 0, 0)
    }

    /// A viewer at `x` on the X axis, in the middle of the tiles on Z.
    fn at(x: f64) -> Point {
        Point { x, z: WIDTH / 2.0 }
    }

    /// Applies the changes of one plan to `subscriptions`.
    fn step(
        settings: &MuxerSettings,
        position: Point,
        subscriptions: &mut BTreeMap<TileKey, Subscription>,
        headers: &[TileHeader],
        measured: Measured,
    ) {
        let viewer = ViewerState {
            settings,
            position,
            subscriptions,
        };
        for change in plan(&viewer, headers, measured) {
            match change.subscription {
                Some(subscription) => subscriptions.insert(change.tile, subscription),
                None => subscriptions.remove(&change.tile),
            };
        }
    }

    fn visible(subscriptions: &BTreeMap<TileKey, Subscription>, headers: &[TileHeader]) -> usize {
        headers
            .iter()
            .filter_map(|header| {
                let ranks = subscriptions.get(&header.tile)?.visible_ranks();
                Some(header.count.min(ranks))
            })
            .sum()
    }

    fn tiers(subscriptions: &BTreeMap<TileKey, Subscription>) -> Vec<Option<Tier>> {
        subscriptions
            .values()
            .flat_map(|subscription| subscription.layers.map(|layer| layer.tier))
            .collect()
    }

    #[test]
    fn the_cap_admits_up_to_k_and_removes_only_above_a_quarter_more() {
        let settings = settings(100);
        let mut subscriptions = BTreeMap::new();
        let crowd = |near: usize| {
            [
                TileHeader {
                    tile: tile(0),
                    count: near,
                },
                TileHeader {
                    tile: tile(1),
                    count: 70,
                },
            ]
        };

        step(
            &settings,
            at(16.0),
            &mut subscriptions,
            &crowd(40),
            Measured::default(),
        );
        assert_eq!(
            visible(&subscriptions, &crowd(40)),
            72,
            "40 near, then the first 32 of the far tile: its second layer would pass 100"
        );

        step(
            &settings,
            at(16.0),
            &mut subscriptions,
            &crowd(55),
            Measured::default(),
        );
        assert_eq!(
            visible(&subscriptions, &crowd(55)),
            87,
            "a sum of 87 stays under 125, so the far tile keeps its layer"
        );

        step(
            &settings,
            at(16.0),
            &mut subscriptions,
            &crowd(100),
            Measured::default(),
        );
        assert_eq!(
            visible(&subscriptions, &crowd(100)),
            100,
            "a sum of 132 is more than 125, so the far tile goes"
        );
    }

    #[test]
    fn a_tile_takes_the_tier_of_its_distance_and_leaves_it_only_8_blocks_later() {
        let settings = settings(200);
        let mut subscriptions = BTreeMap::new();
        let headers = [TileHeader {
            tile: tile(2),
            count: 1,
        }];
        let tier_at = |subscriptions: &mut BTreeMap<TileKey, Subscription>, x: f64| {
            step(
                &settings,
                at(x),
                subscriptions,
                &headers,
                Measured::default(),
            );
            tiers(subscriptions).first().copied().flatten()
        };

        assert_eq!(
            tier_at(&mut subscriptions, 64.0 - 20.0),
            Some(Tier::NEAREST)
        );
        assert_eq!(
            tier_at(&mut subscriptions, 64.0 - 36.0),
            Some(Tier::NEAREST),
            "36 blocks is inside the hysteresis of tier 0"
        );
        assert_eq!(tier_at(&mut subscriptions, 64.0 - 41.0), Some(Tier::ALL[1]));
        assert_eq!(
            tier_at(&mut subscriptions, 64.0 - 31.0),
            Some(Tier::NEAREST),
            "a tile enters a nearer tier at its distance"
        );
    }

    #[test]
    fn the_budget_lowers_the_farthest_layer_first_then_raises_it_when_calm() {
        let settings = MuxerSettings {
            budget: 20 * 100,
            ..MuxerSettings::default()
        };
        let mut subscriptions = BTreeMap::new();
        let headers = [
            TileHeader {
                tile: tile(0),
                count: 5,
            },
            TileHeader {
                tile: tile(1),
                count: 5,
            },
        ];
        let position = at(16.0);
        step(
            &settings,
            position,
            &mut subscriptions,
            &headers,
            Measured::default(),
        );
        let near = tiers(&subscriptions);

        let over = Measured {
            average: 150,
            over: 1,
            calm: 0,
        };
        step(&settings, position, &mut subscriptions, &headers, over);
        let lowered = tiers(&subscriptions);
        assert_eq!(
            lowered.get(..3),
            near.get(..3),
            "the near tile keeps its tier"
        );
        assert_eq!(
            lowered.get(3).copied().flatten(),
            near.get(3).copied().flatten().map(|tier| tier.lowered(1)),
            "the first layer of the far tile is its last admitted layer, and goes one tier down"
        );

        for calm in 1..=RAISE_EVERY_TICKS {
            let measured = Measured {
                average: 10,
                over: 0,
                calm,
            };
            step(&settings, position, &mut subscriptions, &headers, measured);
        }
        assert_eq!(tiers(&subscriptions), near, "calm bytes give the tier back");
    }

    #[test]
    fn the_accumulator_raises_a_far_lowered_layer_that_waited_longer_than_a_near_one() {
        let settings = MuxerSettings {
            budget: 20 * 100,
            ..MuxerSettings::default()
        };
        let headers = [
            TileHeader {
                tile: tile(0),
                count: 5,
            },
            TileHeader {
                tile: tile(3),
                count: 5,
            },
        ];
        let position = at(16.0);
        let mut subscriptions = BTreeMap::new();
        step(
            &settings,
            position,
            &mut subscriptions,
            &headers,
            Measured::default(),
        );
        let natural = tiers(&subscriptions);
        let over = Measured {
            average: 150,
            over: 1,
            calm: 0,
        };
        // The far layer goes down first; long after, the near one.
        step(&settings, position, &mut subscriptions, &headers, over);
        for _ in 0..200 {
            step(
                &settings,
                position,
                &mut subscriptions,
                &headers,
                Measured::default(),
            );
        }
        step(&settings, position, &mut subscriptions, &headers, over);
        let far_lowered = |subscriptions: &BTreeMap<TileKey, Subscription>| {
            subscriptions
                .get(&tile(3))
                .is_some_and(|subscription| subscription.layers[0].lowered > 0)
        };
        assert!(
            far_lowered(&subscriptions),
            "the far layer is still lowered"
        );

        let calm = Measured {
            average: 10,
            over: 0,
            calm: RAISE_EVERY_TICKS,
        };
        step(&settings, position, &mut subscriptions, &headers, calm);

        assert!(
            !far_lowered(&subscriptions),
            "the far layer waited longer, so the first raise goes to it and not to the nearer one"
        );
        assert_ne!(
            tiers(&subscriptions),
            natural,
            "the near layer is still lowered"
        );
    }
}
