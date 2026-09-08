use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateParameters {
    pub temperature: ParameterRange,
    pub humidity: ParameterRange,
    pub continentalness: ParameterRange,
    pub erosion: ParameterRange,
    pub depth: ParameterRange,
    pub weirdness: ParameterRange,
    pub offset: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterRange {
    Point(f64),
    Range([f64; 2]),
}

/// Climate coordinates are compared as fixed point, not floats: the reference
/// multiplies by ten thousand and truncates, and the search is exact integer
/// arithmetic from there on. The multiply happens in f32 because that is the
/// width the codec reads, and rounding it in f64 would land on a different
/// integer for some values.
pub fn quantize_coord(coord: f32) -> i64 {
    (coord * 10000.0) as i64
}

pub fn unquantize_coord(coord: i64) -> f32 {
    coord as f32 / 10000.0
}

/// One climate coordinate's accepted span, quantized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    pub fn point(value: f32) -> Self {
        Self::span(value, value)
    }

    pub fn span(min: f32, max: f32) -> Self {
        assert!(min <= max, "climate span min > max: {min} > {max}");
        Parameter {
            min: quantize_coord(min),
            max: quantize_coord(max),
        }
    }

    /// The union of two spans, which is how the reference widens a slice to
    /// cover several of its neighbours.
    pub fn union(self, other: Parameter) -> Self {
        Parameter {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// How far `target` lies outside the span, zero when inside it.
    pub fn distance(self, target: i64) -> i64 {
        let above = target - self.max;
        let below = self.min - target;
        if above > 0 { above } else { below.max(0) }
    }
}

impl From<&ParameterRange> for Parameter {
    fn from(range: &ParameterRange) -> Self {
        match *range {
            ParameterRange::Point(value) => Parameter::point(value as f32),
            ParameterRange::Range([min, max]) => Parameter::span(min as f32, max as f32),
        }
    }
}

/// The climate a biome accepts. `offset` is a flat penalty that lets one entry
/// lose a tie to another whose spans fit equally well.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterPoint {
    pub temperature: Parameter,
    pub humidity: Parameter,
    pub continentalness: Parameter,
    pub erosion: Parameter,
    pub depth: Parameter,
    pub weirdness: Parameter,
    pub offset: i64,
}

impl ParameterPoint {
    pub fn fitness(&self, target: TargetPoint) -> i64 {
        let terms = [
            self.temperature.distance(target.temperature),
            self.humidity.distance(target.humidity),
            self.continentalness.distance(target.continentalness),
            self.erosion.distance(target.erosion),
            self.depth.distance(target.depth),
            self.weirdness.distance(target.weirdness),
            self.offset,
        ];
        terms.iter().map(|term| term * term).sum()
    }
}

impl From<&ClimateParameters> for ParameterPoint {
    fn from(parameters: &ClimateParameters) -> Self {
        ParameterPoint {
            temperature: (&parameters.temperature).into(),
            humidity: (&parameters.humidity).into(),
            continentalness: (&parameters.continentalness).into(),
            erosion: (&parameters.erosion).into(),
            depth: (&parameters.depth).into(),
            weirdness: (&parameters.weirdness).into(),
            offset: quantize_coord(parameters.offset as f32),
        }
    }
}

/// The climate actually sampled at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetPoint {
    pub temperature: i64,
    pub humidity: i64,
    pub continentalness: i64,
    pub erosion: i64,
    pub depth: i64,
    pub weirdness: i64,
}

impl TargetPoint {
    pub fn new(
        temperature: f32,
        humidity: f32,
        continentalness: f32,
        erosion: f32,
        depth: f32,
        weirdness: f32,
    ) -> Self {
        TargetPoint {
            temperature: quantize_coord(temperature),
            humidity: quantize_coord(humidity),
            continentalness: quantize_coord(continentalness),
            erosion: quantize_coord(erosion),
            depth: quantize_coord(depth),
            weirdness: quantize_coord(weirdness),
        }
    }
}

/// The seven-dimensional point a search compares against: the six sampled
/// coordinates, plus a zero the offset penalty is measured from.
type Coords = [i64; 7];

impl ParameterPoint {
    fn space(&self) -> [Parameter; 7] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
            Parameter {
                min: self.offset,
                max: self.offset,
            },
        ]
    }
}

impl TargetPoint {
    fn coords(self) -> Coords {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
            0,
        ]
    }
}

fn bound_distance(space: &[Parameter; 7], target: &Coords) -> i64 {
    let mut total = 0;
    for (parameter, coordinate) in space.iter().zip(target) {
        let distance = parameter.distance(*coordinate);
        total += distance * distance;
    }
    total
}

/// An R-tree node over the parameter space. A subtree's own space is the union
/// of its children's, which is what lets a search skip a whole branch.
#[derive(Debug, Clone)]
enum Node<T> {
    Leaf {
        space: [Parameter; 7],
        value: T,
    },
    SubTree {
        space: [Parameter; 7],
        children: Vec<Node<T>>,
    },
}

impl<T> Node<T> {
    fn space(&self) -> &[Parameter; 7] {
        match self {
            Node::Leaf { space, .. } | Node::SubTree { space, .. } => space,
        }
    }

    /// The nearest leaf, given a candidate to beat. A branch whose own bound is
    /// already further than the candidate is not entered at all.
    fn search(&self, target: &Coords, best: Option<(T, i64)>) -> Option<(T, i64)>
    where
        T: Copy,
    {
        match self {
            Node::Leaf { space, value } => Some((*value, bound_distance(space, target))),
            Node::SubTree { children, .. } => {
                let mut best = best;
                for child in children {
                    let nearest = best.map_or(i64::MAX, |(_, distance)| distance);
                    if nearest <= bound_distance(child.space(), target) {
                        continue;
                    }
                    if let Some(found) = child.search(target, best)
                        && best.map_or(i64::MAX, |(_, distance)| distance) > found.1
                    {
                        best = Some(found);
                    }
                }
                best
            }
        }
    }
}

const CHILDREN_PER_NODE: usize = 19;

fn union_space<T>(children: &[Node<T>]) -> [Parameter; 7] {
    let mut space = *children[0].space();
    for child in &children[1..] {
        for (slot, parameter) in space.iter_mut().zip(child.space()) {
            *slot = slot.union(*parameter);
        }
    }
    space
}

/// Sort by how far the node sits from the origin overall. This is the order a
/// node's own children end up in, and it is a different key from the one the
/// bucketing uses.
fn sort_by_total_magnitude<T>(nodes: &mut [Node<T>]) {
    nodes.sort_by_key(|node| {
        node.space()
            .iter()
            .map(|parameter| ((parameter.min + parameter.max) / 2).abs())
            .sum::<i64>()
    });
}

/// Sort by the centre of `dimension`, then by the centres of the dimensions
/// after it, wrapping around.
fn sort_nodes<T>(nodes: &mut [Node<T>], dimension: usize, absolute: bool) {
    let key = |node: &Node<T>| -> [i64; 7] {
        let space = node.space();
        let mut out = [0i64; 7];
        for step in 0..7 {
            let parameter = space[(dimension + step) % 7];
            let centre = (parameter.min + parameter.max) / 2;
            out[step] = if absolute { centre.abs() } else { centre };
        }
        out
    };
    nodes.sort_by_key(key);
}

/// How many nodes go in each bucket: the largest power of the branching factor
/// that still leaves more than one bucket.
fn bucket_size(count: usize, children_per_node: usize) -> usize {
    children_per_node
        .pow(((count as f64 - 0.01).ln() / (children_per_node as f64).ln()).floor() as u32)
        .max(1)
}

fn space_cost(space: &[Parameter; 7]) -> i64 {
    space
        .iter()
        .map(|parameter| (parameter.max - parameter.min).abs())
        .sum()
}

fn bucketed_cost<T>(children: &[Node<T>], per_bucket: usize) -> i64 {
    children
        .chunks(per_bucket)
        .map(|bucket| space_cost(&union_space(bucket)))
        .sum()
}

/// Group the nodes along whichever dimension gives the tightest bounds, so a
/// search prunes as much as possible.
fn build_node<T>(mut children: Vec<Node<T>>, children_per_node: usize) -> Node<T> {
    if children.len() == 1 {
        return children.pop().expect("one child");
    }
    if children.len() <= children_per_node {
        sort_by_total_magnitude(&mut children);
        return Node::SubTree {
            space: union_space(&children),
            children,
        };
    }

    let per_bucket = bucket_size(children.len(), children_per_node);
    let mut best_dimension = 0;
    let mut lowest_cost = i64::MAX;
    for dimension in 0..7 {
        sort_nodes(&mut children, dimension, false);
        let cost = bucketed_cost(&children, per_bucket);
        if cost < lowest_cost {
            lowest_cost = cost;
            best_dimension = dimension;
        }
    }

    sort_nodes(&mut children, best_dimension, false);
    let mut buckets: Vec<Node<T>> = Vec::new();
    let mut rest = children;
    while !rest.is_empty() {
        let take = per_bucket.min(rest.len());
        let bucket: Vec<Node<T>> = rest.drain(..take).collect();
        buckets.push(Node::SubTree {
            space: union_space(&bucket),
            children: bucket,
        });
    }
    // The reference orders the buckets before descending into them, along the
    // dimension it bucketed on, so the child order is the bucketing's order.
    sort_nodes(&mut buckets, best_dimension, true);
    let subtrees: Vec<Node<T>> = buckets
        .into_iter()
        .map(|bucket| match bucket {
            Node::SubTree { children, .. } => build_node(children, children_per_node),
            leaf => leaf,
        })
        .collect();
    Node::SubTree {
        space: union_space(&subtrees),
        children: subtrees,
    }
}

/// A biome table, searched by nearest climate.
#[derive(Debug, Clone)]
pub struct ParameterList<T> {
    values: Vec<(ParameterPoint, T)>,
    index: Node<usize>,
}

impl<T> ParameterList<T> {
    pub fn new(values: Vec<(ParameterPoint, T)>) -> Self {
        assert!(
            !values.is_empty(),
            "a climate table needs at least one entry"
        );
        let leaves = values
            .iter()
            .enumerate()
            .map(|(slot, (point, _))| Node::Leaf {
                space: point.space(),
                value: slot,
            })
            .collect();
        let index = build_node(leaves, CHILDREN_PER_NODE);
        ParameterList { values, index }
    }

    pub fn values(&self) -> &[(ParameterPoint, T)] {
        &self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The entry whose climate fits `target` best.
    pub fn find_value(&self, target: TargetPoint) -> &T {
        self.find_value_from(target, &mut None)
    }

    /// [`Self::find_value`] seeded with `last`, the slot the previous query
    /// answered. Climate drifts slowly between neighbouring cells, so that leaf
    /// is usually the answer or next to it, and its distance prunes most of the
    /// tree before the walk starts.
    pub fn find_value_from(&self, target: TargetPoint, last: &mut Option<usize>) -> &T {
        &self.values[self.find_slot_from(target, last)].1
    }

    /// [`Self::find_value_from`], answering with the entry's index into
    /// [`Self::values`] rather than the entry.
    pub fn find_slot_from(&self, target: TargetPoint, last: &mut Option<usize>) -> usize {
        let coords = target.coords();
        let seed = last.map(|slot| (slot, bound_distance(&self.values[slot].0.space(), &coords)));
        let (slot, _) = self.index.search(&coords, seed).expect("a non-empty tree");
        *last = Some(slot);
        slot
    }

    /// The same answer by scanning every entry, which is what the tree has to
    /// agree with. Ties go to the earlier entry here; the tree visits its own
    /// sorted order, so a tie can land elsewhere and the tree is the authority.
    pub fn find_value_brute_force(&self, target: TargetPoint) -> &T {
        let mut best = &self.values[0];
        let mut best_fitness = best.0.fitness(target);
        for candidate in &self.values[1..] {
            let fitness = candidate.0.fitness(target);
            if fitness < best_fitness {
                best_fitness = fitness;
                best = candidate;
            }
        }
        &best.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(temperature: f32, humidity: f32) -> ParameterPoint {
        ParameterPoint {
            temperature: Parameter::point(temperature),
            humidity: Parameter::point(humidity),
            continentalness: Parameter::point(0.0),
            erosion: Parameter::point(0.0),
            depth: Parameter::point(0.0),
            weirdness: Parameter::point(0.0),
            offset: 0,
        }
    }

    #[test]
    fn quantization_truncates_toward_zero_at_f32_width() {
        assert_eq!(quantize_coord(0.0), 0);
        assert_eq!(quantize_coord(1.0), 10000);
        assert_eq!(quantize_coord(-1.0), -10000);
        assert_eq!(quantize_coord(0.05), 500);
        // 0.26666668 * 10000 lands just under 2666.667, and truncation is what
        // the reference does with it.
        assert_eq!(quantize_coord(0.266_666_68), 2666);
        assert_eq!(unquantize_coord(quantize_coord(0.5)), 0.5);
    }

    #[test]
    fn a_span_measures_only_the_distance_outside_itself() {
        let span = Parameter::span(-0.2, 0.2);
        assert_eq!(span.distance(quantize_coord(0.0)), 0);
        assert_eq!(span.distance(quantize_coord(0.2)), 0);
        assert_eq!(span.distance(quantize_coord(0.3)), 1000);
        assert_eq!(span.distance(quantize_coord(-0.3)), 1000);
    }

    #[test]
    fn a_union_covers_both_spans() {
        let joined = Parameter::span(-0.5, -0.2).union(Parameter::span(0.1, 0.4));
        assert_eq!(joined, Parameter::span(-0.5, 0.4));
    }

    #[test]
    fn the_offset_is_a_flat_penalty_that_breaks_a_tie() {
        let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let plain = point(0.0, 0.0);
        let penalized = ParameterPoint {
            offset: quantize_coord(0.1),
            ..plain
        };
        assert_eq!(plain.fitness(target), 0);
        assert_eq!(penalized.fitness(target), 1000 * 1000);
        let table = ParameterList::new(vec![(penalized, "penalized"), (plain, "plain")]);
        assert_eq!(*table.find_value(target), "plain");
    }

    #[test]
    fn the_nearest_entry_wins_and_ties_go_to_the_earlier_one() {
        let table = ParameterList::new(vec![
            (point(-0.5, 0.0), "cold"),
            (point(0.5, 0.0), "warm"),
            (point(0.5, 0.0), "warm duplicate"),
        ]);
        assert_eq!(
            *table.find_value(TargetPoint::new(-0.4, 0.0, 0.0, 0.0, 0.0, 0.0)),
            "cold"
        );
        assert_eq!(
            *table.find_value(TargetPoint::new(0.6, 0.0, 0.0, 0.0, 0.0, 0.0)),
            "warm"
        );
    }

    #[test]
    fn every_coordinate_counts_toward_the_fit() {
        let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut only_weirdness = point(0.0, 0.0);
        only_weirdness.weirdness = Parameter::point(1.0);
        assert_eq!(only_weirdness.fitness(target), 10000 * 10000);
        let mut only_depth = point(0.0, 0.0);
        only_depth.depth = Parameter::point(1.0);
        assert_eq!(only_depth.fitness(target), 10000 * 10000);
    }

    /// The tree only prunes, so it has to reach the same entry a full scan
    /// does. Ties can land elsewhere, since the tree walks its own sorted
    /// order, and the tree is the authority there.
    #[test]
    fn the_index_agrees_with_a_full_scan() {
        let mut rng = 0x2545F4914F6CDD1Du64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            ((rng >> 11) as f32 / (1u64 << 53) as f32) * 4.0 - 2.0
        };
        let table = ParameterList::new(
            (0..400)
                .map(|slot| {
                    let low = next();
                    let high = low + next().abs();
                    (
                        ParameterPoint {
                            temperature: Parameter::span(low.min(high), high.max(low)),
                            humidity: Parameter::point(next()),
                            continentalness: Parameter::point(next()),
                            erosion: Parameter::point(next()),
                            depth: Parameter::point(next()),
                            weirdness: Parameter::point(next()),
                            offset: 0,
                        },
                        slot,
                    )
                })
                .collect(),
        );
        let mut checked = 0;
        let mut last = None;
        for step in 0..2000 {
            let target = TargetPoint::new(next(), next(), next(), next(), next(), next());
            // Every other query carries the previous answer in, which the seed
            // may only use to prune, never to settle on a worse leaf.
            let indexed = if step % 2 == 0 {
                *table.find_value(target)
            } else {
                *table.find_value_from(target, &mut last)
            };
            let scanned = *table.find_value_brute_force(target);
            if indexed != scanned {
                // Only a tie may differ, and then both must fit equally well.
                assert_eq!(
                    table.values()[indexed].0.fitness(target),
                    table.values()[scanned].0.fitness(target),
                    "the index found a worse entry than the scan"
                );
            }
            checked += 1;
        }
        assert_eq!(checked, 2000);
    }

    /// The explicit `biomes` form of a multi-noise source carries the same
    /// spans through the JSON codec, so a parsed entry and a built one must
    /// quantize identically.
    #[test]
    fn a_parsed_entry_matches_a_built_one() {
        let parsed: ClimateParameters = serde_json::from_str(
            r#"{"temperature":[-0.45,-0.15],"humidity":0.0,"continentalness":[-0.11,0.55],
                "erosion":[-0.375,0.05],"depth":0.0,"weirdness":[-1.0,-0.78],"offset":0.0}"#,
        )
        .unwrap();
        let built = ParameterPoint {
            temperature: Parameter::span(-0.45, -0.15),
            humidity: Parameter::point(0.0),
            continentalness: Parameter::span(-0.11, 0.55),
            erosion: Parameter::span(-0.375, 0.05),
            depth: Parameter::point(0.0),
            weirdness: Parameter::span(-1.0, -0.78),
            offset: 0,
        };
        assert_eq!(ParameterPoint::from(&parsed), built);
    }
}
