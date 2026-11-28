use std::ops::Range;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClimateParameters {
    pub temperature: ParameterRange,
    pub humidity: ParameterRange,
    pub continentalness: ParameterRange,
    pub erosion: ParameterRange,
    pub depth: ParameterRange,
    pub weirdness: ParameterRange,
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterRange {
    Point(f64),
    Range([f64; 2]),
}

pub fn quantize_coord(coord: f32) -> i64 {
    (coord * 10000.0) as i64
}

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

    pub fn union(self, other: Parameter) -> Self {
        Parameter {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn overlaps(self, other: Parameter) -> bool {
        self == other || self.min.max(other.min) < self.max.min(other.max)
    }

    #[inline]
    pub fn distance(self, target: i64) -> i64 {
        target
            .saturating_sub(self.max)
            .saturating_add(1)
            .max(self.min.saturating_sub(target))
            .max(0)
    }
}

fn saturating_add_square(total: i64, distance: i64) -> i64 {
    total.saturating_add(distance.saturating_mul(distance))
}

impl From<&ParameterRange> for Parameter {
    fn from(range: &ParameterRange) -> Self {
        match *range {
            ParameterRange::Point(value) => Parameter::point(value as f32),
            ParameterRange::Range([min, max]) => Parameter::span(min as f32, max as f32),
        }
    }
}

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

impl ParameterPoint {
    pub fn indistinguishable_from(&self, other: &ParameterPoint) -> bool {
        self.offset == other.offset
            && self.temperature.overlaps(other.temperature)
            && self.humidity.overlaps(other.humidity)
            && self.continentalness.overlaps(other.continentalness)
            && self.erosion.overlaps(other.erosion)
            && self.depth.overlaps(other.depth)
            && self.weirdness.overlaps(other.weirdness)
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

#[inline]
fn bound_distance(space: &[Parameter; 7], target: &Coords) -> i64 {
    space
        .iter()
        .zip(target)
        .fold(0, |total, (parameter, &target)| {
            saturating_add_square(total, parameter.distance(target))
        })
}

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
}

#[derive(Debug, Clone, Default)]
struct Tree {
    min: [Vec<i64>; 6],
    max: [Vec<i64>; 6],
    offset: Vec<i64>,
    value: Vec<u32>,
    children_start: Vec<u32>,
    children_count: Vec<u32>,
}

impl Tree {
    fn push(&mut self, space: &[Parameter; 7]) {
        let [.., offset] = space;
        for ((min, max), parameter) in self.min.iter_mut().zip(&mut self.max).zip(space) {
            min.push(parameter.min);
            max.push(parameter.max);
        }
        self.offset
            .push(saturating_add_square(0, offset.distance(0)));
        self.value.push(0);
        self.children_start.push(0);
        self.children_count.push(0);
    }
}

fn flatten<T>(root: Node<T>) -> Tree
where
    T: Copy + Into<u32>,
{
    let mut tree = Tree::default();
    tree.push(root.space());
    let mut queue = std::collections::VecDeque::from([(root, 0usize)]);
    while let Some((node, at)) = queue.pop_front() {
        match node {
            Node::Leaf { value, .. } => {
                if let Some(slot) = tree.value.get_mut(at) {
                    *slot = value.into();
                }
            }
            Node::SubTree { children, .. } => {
                let start = tree.value.len();
                if let (Some(slot_start), Some(slot_count)) = (
                    tree.children_start.get_mut(at),
                    tree.children_count.get_mut(at),
                ) {
                    *slot_start = start as u32;
                    *slot_count = children.len() as u32;
                }
                for child in &children {
                    tree.push(child.space());
                }
                for (offset, child) in children.into_iter().enumerate() {
                    queue.push_back((child, start + offset));
                }
            }
        }
    }
    tree
}

struct Children<'t> {
    range: Range<usize>,
    distances: [i64; CHILDREN_PER_NODE],
    counts: &'t [u32],
    values: &'t [u32],
}

#[inline]
fn children<'t>(
    tree: &'t Tree,
    at: usize,
    target: &Coords,
    add_square: impl Fn(i64, i64) -> Option<i64>,
) -> Option<Children<'t>> {
    let start = *tree.children_start.get(at)? as usize;
    let range = start..start + *tree.children_count.get(at)? as usize;
    let mut distances = [0i64; CHILDREN_PER_NODE];
    distances
        .get_mut(..range.len())?
        .copy_from_slice(tree.offset.get(range.clone())?);
    for ((min, max), &target) in tree.min.iter().zip(&tree.max).zip(target) {
        let (min, max) = (min.get(range.clone())?, max.get(range.clone())?);
        for ((total, &min), &max) in distances.iter_mut().zip(min).zip(max) {
            *total = add_square(*total, Parameter { min, max }.distance(target))?;
        }
    }
    Some(Children {
        counts: tree.children_count.get(range.clone())?,
        values: tree.value.get(range.clone())?,
        range,
        distances,
    })
}

fn search(tree: &Tree, at: usize, target: &Coords, best: Option<(u32, i64)>) -> Option<(u32, i64)> {
    let Some(children) = children(tree, at, target, |total, distance| {
        Some(saturating_add_square(total, distance))
    }) else {
        return best;
    };
    let mut best = best;
    for (((child, &distance), &grandchildren), &value) in children
        .range
        .zip(&children.distances)
        .zip(children.counts)
        .zip(children.values)
    {
        if let Some((_, nearest)) = best
            && nearest <= distance
        {
            continue;
        }
        let found = if grandchildren == 0 {
            Some((value, distance))
        } else {
            search(tree, child, target, best)
        };
        if let Some(found) = found
            && best.is_none_or(|(_, nearest)| nearest > found.1)
        {
            best = Some(found);
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nearest {
    Unique { slot: u32, distance: i64 },
    Tied { distance: i64 },
}

impl Nearest {
    fn distance(self) -> i64 {
        match self {
            Nearest::Unique { distance, .. } | Nearest::Tied { distance } => distance,
        }
    }

    fn at_distance(self, distance: i64) -> Self {
        match self {
            Nearest::Unique { slot, .. } => Nearest::Unique { slot, distance },
            Nearest::Tied { .. } => Nearest::Tied { distance },
        }
    }
}

fn checked_add_square(total: i64, distance: i64) -> Option<i64> {
    distance.checked_mul(distance)?.checked_add(total)
}

fn search_unique(
    tree: &Tree,
    at: usize,
    target: &Coords,
    best: &mut Option<Nearest>,
    bound: i64,
) -> Option<()> {
    let children = children(tree, at, target, checked_add_square)?;
    for (((child, &distance), &grandchildren), &value) in children
        .range
        .zip(&children.distances)
        .zip(children.counts)
        .zip(children.values)
    {
        if distance > bound {
            continue;
        }
        let pruned = match *best {
            Some(Nearest::Unique {
                distance: nearest, ..
            }) => nearest < distance,
            Some(Nearest::Tied { distance: nearest }) => nearest <= distance,
            None => false,
        };
        if pruned {
            continue;
        }
        let nearest = best.map_or(i64::MAX, Nearest::distance);
        if grandchildren != 0 {
            search_unique(tree, child, target, best, bound)?;
        } else if distance < nearest {
            *best = Some(Nearest::Unique {
                slot: value,
                distance,
            });
        } else if let Some(best) = best {
            *best = Nearest::Tied { distance };
        }
    }
    Some(())
}

const NEUTRAL_DEPTH: Parameter = Parameter { min: 0, max: 1 };

const NEUTRAL_DEPTH_TARGET: i64 = 0;

const MAX_DEPTH_GROUPS: usize = 8;

#[derive(Debug, Clone)]
struct DepthGroup {
    depth: Parameter,
    tree: Tree,
    farthest_offset: i64,
}

impl DepthGroup {
    fn farthest_distance(&self, target: &Coords) -> Option<i64> {
        let mut total = self.farthest_offset;
        for ((min, max), &target) in self.tree.min.iter().zip(&self.tree.max).zip(target) {
            let (min, max) = (*min.first()?, *max.first()?);
            let distance = target
                .checked_sub(min)?
                .checked_add(1)?
                .max(max.checked_sub(target)?)
                .max(0);
            total = checked_add_square(total, distance)?;
        }
        Some(total)
    }

    fn bound(&self, seed: Option<&ParameterPoint>, target: &Coords) -> i64 {
        let space = seed.map(|point| {
            ParameterPoint {
                depth: NEUTRAL_DEPTH,
                ..*point
            }
            .space()
        });
        space
            .filter(|_| self.farthest_distance(target).is_some())
            .and_then(|space| checked_leaf_distance(&space, target))
            .unwrap_or(i64::MAX)
    }
}

fn checked_leaf_distance(space: &[Parameter; 7], target: &Coords) -> Option<i64> {
    space
        .iter()
        .zip(target)
        .try_fold(0, |total, (parameter, &target)| {
            checked_add_square(total, parameter.distance(target))
        })
}

fn depth_groups<T>(values: &[(ParameterPoint, T)]) -> Vec<DepthGroup> {
    let mut depths: Vec<Parameter> = Vec::new();
    for (point, _) in values {
        if !depths.contains(&point.depth) {
            if depths.len() == MAX_DEPTH_GROUPS {
                return Vec::new();
            }
            depths.push(point.depth);
        }
    }
    let groups: Option<Vec<DepthGroup>> = depths
        .into_iter()
        .map(|depth| {
            let leaves = values
                .iter()
                .enumerate()
                .filter(|(_, (point, _))| point.depth == depth)
                .map(|(slot, (point, _))| Node::Leaf {
                    space: ParameterPoint {
                        depth: NEUTRAL_DEPTH,
                        ..*point
                    }
                    .space(),
                    value: slot as u32,
                })
                .collect();
            let root = match build_node(leaves, CHILDREN_PER_NODE)? {
                leaf @ Node::Leaf { .. } => Node::SubTree {
                    space: *leaf.space(),
                    children: vec![leaf],
                },
                subtree @ Node::SubTree { .. } => subtree,
            };
            let tree = flatten(root);
            Some(DepthGroup {
                depth,
                farthest_offset: tree.offset.iter().copied().max()?,
                tree,
            })
        })
        .collect();
    groups.unwrap_or_default()
}

fn column_of(target: TargetPoint) -> TargetPoint {
    TargetPoint {
        depth: NEUTRAL_DEPTH_TARGET,
        ..target
    }
}

fn group_nearest<T>(
    list: &ParameterList<T>,
    column: TargetPoint,
    seeds: &mut [Option<u32>; MAX_DEPTH_GROUPS],
) -> Option<[Option<Nearest>; MAX_DEPTH_GROUPS]> {
    let coords = column.coords();
    let mut nearest = [None; MAX_DEPTH_GROUPS];
    for ((best, group), seed) in nearest.iter_mut().zip(&list.depth_groups).zip(seeds) {
        let point = seed
            .and_then(|slot| list.values.get(slot as usize))
            .map(|(point, _)| point);
        search_unique(&group.tree, 0, &coords, best, group.bound(point, &coords))?;
        if let Some(Nearest::Unique { slot, .. }) = *best {
            *seed = Some(slot);
        }
    }
    Some(nearest)
}

#[derive(Debug, Clone, Copy)]
enum ColumnNearest {
    Unsearched,
    Grouped([Option<Nearest>; MAX_DEPTH_GROUPS]),
    Overflowed,
}

#[derive(Debug, Clone)]
pub struct ClimateCursor<'a, T> {
    list: &'a ParameterList<T>,
    last: Option<usize>,
    column: Option<TargetPoint>,
    nearest: ColumnNearest,
    seeds: [Option<u32>; MAX_DEPTH_GROUPS],
}

impl<'a, T> ClimateCursor<'a, T> {
    pub fn find_value(&mut self, target: TargetPoint) -> Option<&'a T> {
        let slot = self.find_slot(target)?;
        self.list.values.get(slot).map(|(_, value)| value)
    }

    pub fn find_slot(&mut self, target: TargetPoint) -> Option<usize> {
        let Some(slot) = self.grouped_slot(target) else {
            return self.list.nearest_from(target, &mut self.last);
        };
        self.last = Some(slot);
        Some(slot)
    }

    pub fn find_unique_value(&mut self, target: TargetPoint) -> Option<&'a T> {
        let slot = self.grouped_slot(target)?;
        self.last = Some(slot);
        self.list.values.get(slot).map(|(_, value)| value)
    }

    fn grouped_slot(&mut self, target: TargetPoint) -> Option<usize> {
        let list = self.list;
        let column = column_of(target);
        if list.depth_groups.is_empty() {
            return None;
        }
        if self.column != Some(column) {
            self.column = Some(column);
            self.nearest = group_nearest(list, column, &mut self.seeds)
                .map_or(ColumnNearest::Overflowed, ColumnNearest::Grouped);
        }
        let ColumnNearest::Grouped(nearest) = &self.nearest else {
            return None;
        };
        let mut best: Option<Nearest> = None;
        for (group, nearest) in list.depth_groups.iter().zip(nearest.iter()) {
            let Some(nearest) = nearest else {
                continue;
            };
            let depth = group.depth.distance(target.depth);
            let distance = checked_add_square(nearest.distance(), depth)?;
            best = match best {
                Some(best) if distance > best.distance() => Some(best),
                Some(best) if distance == best.distance() => Some(Nearest::Tied { distance }),
                _ => Some(nearest.at_distance(distance)),
            };
        }
        match best? {
            Nearest::Unique { slot, .. } => Some(slot as usize),
            Nearest::Tied { .. } => None,
        }
    }
}

const CHILDREN_PER_NODE: usize = 19;

fn union_space<T>(children: &[Node<T>]) -> Option<[Parameter; 7]> {
    let (first, rest) = children.split_first()?;
    let mut space = *first.space();
    for child in rest {
        for (slot, parameter) in space.iter_mut().zip(child.space()) {
            *slot = slot.union(*parameter);
        }
    }
    Some(space)
}

fn sort_by_total_magnitude<T>(nodes: &mut [Node<T>]) {
    nodes.sort_by_cached_key(|node| {
        node.space()
            .iter()
            .map(|parameter| ((parameter.min + parameter.max) / 2).abs())
            .sum::<i64>()
    });
}

fn sort_nodes<T>(nodes: &mut [Node<T>], dimension: usize, absolute: bool) {
    let key = |node: &Node<T>| -> [i64; 7] {
        let space = node.space();
        let mut out = [0i64; 7];
        for (slot, parameter) in out.iter_mut().zip(space.iter().cycle().skip(dimension)) {
            let centre = (parameter.min + parameter.max) / 2;
            *slot = if absolute { centre.abs() } else { centre };
        }
        out
    };
    nodes.sort_by_cached_key(key);
}

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
        .filter_map(union_space)
        .map(|space| space_cost(&space))
        .sum()
}

fn build_node<T>(mut children: Vec<Node<T>>, children_per_node: usize) -> Option<Node<T>> {
    if children.len() == 1 {
        return children.pop();
    }
    if children.len() <= children_per_node {
        sort_by_total_magnitude(&mut children);
        return Some(Node::SubTree {
            space: union_space(&children)?,
            children,
        });
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
            space: union_space(&bucket)?,
            children: bucket,
        });
    }
    sort_nodes(&mut buckets, best_dimension, true);
    let subtrees: Vec<Node<T>> = buckets
        .into_iter()
        .map(|bucket| match bucket {
            Node::SubTree { children, .. } => build_node(children, children_per_node),
            leaf => Some(leaf),
        })
        .collect::<Option<_>>()?;
    Some(Node::SubTree {
        space: union_space(&subtrees)?,
        children: subtrees,
    })
}

#[derive(Debug, Clone)]
pub struct ParameterList<T> {
    values: Vec<(ParameterPoint, T)>,
    tree: Tree,
    depth_groups: Vec<DepthGroup>,
}

impl<T> ParameterList<T> {
    pub fn new(values: Vec<(ParameterPoint, T)>) -> Self {
        let leaves = values
            .iter()
            .enumerate()
            .map(|(slot, (point, _))| Node::Leaf {
                space: point.space(),
                value: slot as u32,
            })
            .collect();
        let tree = build_node(leaves, CHILDREN_PER_NODE)
            .map(flatten)
            .unwrap_or_default();
        let depth_groups = depth_groups(&values);
        ParameterList {
            values,
            tree,
            depth_groups,
        }
    }

    pub fn map_values<U>(&self, mut f: impl FnMut(&T) -> U) -> ParameterList<U> {
        ParameterList {
            values: self
                .values
                .iter()
                .map(|(point, value)| (*point, f(value)))
                .collect(),
            tree: self.tree.clone(),
            depth_groups: self.depth_groups.clone(),
        }
    }

    pub fn try_map_values<U>(
        &self,
        mut f: impl FnMut(&T) -> Option<U>,
    ) -> Option<ParameterList<U>> {
        Some(ParameterList {
            values: self
                .values
                .iter()
                .map(|(point, value)| Some((*point, f(value)?)))
                .collect::<Option<_>>()?,
            tree: self.tree.clone(),
            depth_groups: self.depth_groups.clone(),
        })
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

    pub fn cursor(&self) -> ClimateCursor<'_, T> {
        ClimateCursor {
            list: self,
            last: None,
            column: None,
            nearest: ColumnNearest::Unsearched,
            seeds: [None; MAX_DEPTH_GROUPS],
        }
    }

    pub fn find_value(&self, target: TargetPoint) -> Option<&T> {
        self.cursor().find_value(target)
    }

    fn nearest_from(&self, target: TargetPoint, last: &mut Option<usize>) -> Option<usize> {
        let coords = target.coords();
        let seed = last.and_then(|slot| {
            let (point, _) = self.values.get(slot)?;
            Some((slot as u32, bound_distance(&point.space(), &coords)))
        });
        let slot = match self.tree.children_count.first()? {
            0 => *self.tree.value.first()?,
            _ => search(&self.tree, 0, &coords, seed)?.0,
        } as usize;
        *last = Some(slot);
        Some(slot)
    }

    pub fn find_value_brute_force(&self, target: TargetPoint) -> Option<&T> {
        let (mut best, rest) = self.values.split_first()?;
        let mut best_fitness = best.0.fitness(target);
        for candidate in rest {
            let fitness = candidate.0.fitness(target);
            if fitness < best_fitness {
                best_fitness = fitness;
                best = candidate;
            }
        }
        Some(&best.1)
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
        assert_eq!(quantize_coord(0.266_666_68), 2666);
        assert_eq!(quantize_coord(0.5), 5000);
    }

    #[test]
    fn a_span_measures_only_the_distance_outside_itself() {
        let span = Parameter::span(-0.2, 0.2);
        assert_eq!(span.distance(quantize_coord(-0.2)), 0);
        assert_eq!(span.distance(quantize_coord(0.0)), 0);
        assert_eq!(span.distance(quantize_coord(0.2)), 1);
        assert_eq!(span.distance(quantize_coord(0.3)), 1001);
        assert_eq!(span.distance(quantize_coord(-0.3)), 1000);
    }

    #[test]
    fn spans_that_only_touch_do_not_overlap() {
        let low = Parameter::span(-1.0, 0.7);
        assert!(!low.overlaps(Parameter::span(0.7, 1.0)));
        assert!(low.overlaps(Parameter::span(0.6, 1.0)));
        assert!(Parameter::point(0.0).overlaps(Parameter::point(0.0)));
        assert!(!Parameter::point(0.0).overlaps(Parameter::span(-1.0, 1.0)));
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
        assert_eq!(plain.fitness(target), 6);
        assert_eq!(penalized.fitness(target), 6 + 1000 * 1000);
        let table = ParameterList::new(vec![(penalized, "penalized"), (plain, "plain")]);
        assert_eq!(*table.find_value(target).unwrap(), "plain");
    }

    #[test]
    fn the_nearest_entry_wins_and_ties_go_to_the_earlier_one() {
        let table = ParameterList::new(vec![
            (point(-0.5, 0.0), "cold"),
            (point(0.5, 0.0), "warm"),
            (point(0.5, 0.0), "warm duplicate"),
        ]);
        assert_eq!(
            *table
                .find_value(TargetPoint::new(-0.4, 0.0, 0.0, 0.0, 0.0, 0.0))
                .unwrap(),
            "cold"
        );
        assert_eq!(
            *table
                .find_value(TargetPoint::new(0.6, 0.0, 0.0, 0.0, 0.0, 0.0))
                .unwrap(),
            "warm"
        );
    }

    #[test]
    fn every_coordinate_counts_toward_the_fit() {
        let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut only_weirdness = point(0.0, 0.0);
        only_weirdness.weirdness = Parameter::point(1.0);
        assert_eq!(only_weirdness.fitness(target), 10000 * 10000 + 5);
        let mut only_depth = point(0.0, 0.0);
        only_depth.depth = Parameter::point(1.0);
        assert_eq!(only_depth.fitness(target), 10000 * 10000 + 5);
    }

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
        let mut cursor = table.cursor();
        for step in 0..2000 {
            let target = TargetPoint::new(next(), next(), next(), next(), next(), next());
            let indexed = if step % 2 == 0 {
                *table.find_value(target).unwrap()
            } else {
                *cursor.find_value(target).unwrap()
            };
            let scanned = *table.find_value_brute_force(target).unwrap();
            if indexed != scanned {
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

    #[test]
    fn a_column_cursor_finds_the_slot_of_the_warm_started_search() {
        let table = crate::overworld_preset::overworld_parameter_list();
        assert!(
            !table.depth_groups.is_empty(),
            "the overworld list has few distinct depths, so the grouped search is on"
        );
        let mut rng = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            ((rng >> 11) as f32 / (1u64 << 53) as f32) * 2.4 - 1.2
        };
        let mut cursor = table.cursor();
        let mut last = None;
        let mut grouped = 0;
        for _ in 0..300 {
            let (temperature, humidity, continentalness, erosion, weirdness) =
                (next(), next(), next(), next(), next());
            let mut depths: Vec<f32> = (0..40).map(|step| step as f32 * 0.05 - 0.5).collect();
            depths.push(next());
            depths.push(0.0);
            depths.push(1.0);
            for depth in depths {
                let target = TargetPoint::new(
                    temperature,
                    humidity,
                    continentalness,
                    erosion,
                    depth,
                    weirdness,
                );
                let expected = table.nearest_from(target, &mut last);
                grouped += usize::from(cursor.clone().grouped_slot(target).is_some());
                assert_eq!(cursor.find_slot(target), expected);
            }
        }
        assert!(
            grouped > 10_000,
            "the grouped path answered {grouped} lookups"
        );
    }

    #[test]
    fn a_bound_from_the_previous_column_keeps_every_group_answer() {
        let overworld = crate::overworld_preset::overworld_parameter_list().values();
        let duplicates = overworld.iter().step_by(7);
        let table = ParameterList::new(
            overworld
                .iter()
                .chain(duplicates)
                .map(|(point, _)| (*point, ()))
                .collect(),
        );
        let steps = 25;
        let at = |step: usize| (step % steps) as f32 * 0.1 - 1.2;
        let mut seeds = [None; MAX_DEPTH_GROUPS];
        let mut tied = 0;
        for row in 0..steps {
            for step in 0..steps {
                let column = TargetPoint::new(
                    at(row),
                    at(row + 2 * step),
                    at(step),
                    at(3 * row + step),
                    0.0,
                    at(row * step),
                );
                let unbounded = group_nearest(&table, column, &mut [None; MAX_DEPTH_GROUPS]);
                let bounded = group_nearest(&table, column, &mut seeds);
                assert_eq!(bounded, unbounded, "at row {row}, step {step}");
                tied += unbounded
                    .iter()
                    .flatten()
                    .flatten()
                    .filter(|nearest| matches!(nearest, Nearest::Tied { .. }))
                    .count();
            }
        }
        assert!(
            tied > 50,
            "the duplicated entries tie in {tied} group searches"
        );
    }

    #[test]
    fn a_tie_inside_a_depth_group_keeps_the_warm_started_answer() {
        let at = |temperature: i64| ParameterPoint {
            temperature: Parameter {
                min: temperature,
                max: temperature,
            },
            ..point(0.0, 0.0)
        };
        let table = ParameterList::new(vec![(at(-5000), "cold"), (at(5001), "warm")]);
        let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(
            table.values()[0].0.fitness(target),
            table.values()[1].0.fitness(target),
            "the two entries are equally far from the middle"
        );
        for lead in [-0.4, 0.4] {
            let mut cursor = table.cursor();
            let mut last = None;
            for temperature in [lead, 0.0, 0.0] {
                let target = TargetPoint::new(temperature, 0.0, 0.0, 0.0, 0.0, 0.0);
                assert_eq!(
                    cursor.find_slot(target),
                    table.nearest_from(target, &mut last),
                    "after a lead at {lead}"
                );
            }
        }
    }

    #[test]
    fn a_depth_group_overflow_falls_back_to_the_full_search() {
        let far = 3_100_000_000;
        let at = |temperature: i64, depth: i64| ParameterPoint {
            temperature: Parameter {
                min: temperature,
                max: temperature,
            },
            depth: Parameter {
                min: depth,
                max: depth,
            },
            ..point(0.0, 0.0)
        };
        for (near, outlier) in [(at(0, 100), at(0, far)), (at(100, 100), at(far, 100))] {
            let mut values = vec![(at(0, 0), 0); 19];
            values.push((near, 19));
            values.push((outlier, 20));
            let table = ParameterList::new(values);
            let target = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            let mut cursor = table.cursor();
            let mut last = None;
            for _ in 0..2 {
                assert_eq!(
                    cursor.find_slot(target),
                    table.nearest_from(target, &mut last),
                    "the outlier is pruned by the full search"
                );
            }
        }
    }

    #[test]
    fn an_entry_far_outside_the_climate_range_still_yields_a_value() {
        let hot = ParameterPoint {
            temperature: Parameter::point(1e6),
            ..point(0.0, 0.0)
        };
        let cold = ParameterPoint {
            temperature: Parameter::point(-1e6),
            ..point(0.0, 0.0)
        };
        let origin = TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let far = TargetPoint::new(f32::MAX, 0.0, 0.0, 0.0, 0.0, 0.0);
        for (values, target, expected) in [
            (
                vec![(hot, "hot"), (point(0.1, 0.0), "near")],
                origin,
                "near",
            ),
            (vec![(hot, "hot"), (cold, "cold")], origin, "hot"),
            (vec![(point(0.0, 0.0), "only")], far, "only"),
        ] {
            let table = ParameterList::new(values);
            assert_eq!(table.find_value(target), Some(&expected));
            assert_eq!(table.cursor().find_value(target), Some(&expected));
        }
    }

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
