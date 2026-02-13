use crate::proto::Interval;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "Interval<QuantizedCoord>")]
#[serde(into = "Interval<QuantizedCoord>")]
pub struct Param {
    pub min: QuantizedCoord,
    pub max: QuantizedCoord,
}

impl<I: Into<QuantizedCoord>> From<I> for Param {
    fn from(value: I) -> Self {
        let value = value.into();
        Param {
            min: value.clone(),
            max: value,
        }
    }
}

impl Param {
    fn distance(&self, value: i64) -> i64 {
        let m = value - self.max.0;
        if m > 0 {
            m
        } else {
            (self.min.0 - value).max(0)
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParamPoint {
    pub temperature: Param,
    pub humidity: Param,
    pub continentalness: Param,
    pub erosion: Param,
    pub depth: Param,
    pub weirdness: Param,
    pub offset: QuantizedCoord,
}

impl ParamPoint {
    #[inline]
    pub fn new<P, Q>(
        temperature: P,
        humidity: P,
        continentalness: P,
        erosion: P,
        depth: P,
        weirdness: P,
        offset: Q,
    ) -> ParamPoint
    where
        P: Into<Param>,
        Q: Into<QuantizedCoord>,
    {
        ParamPoint {
            temperature: temperature.into(),
            humidity: humidity.into(),
            continentalness: continentalness.into(),
            erosion: erosion.into(),
            depth: depth.into(),
            weirdness: weirdness.into(),
            offset: offset.into(),
        }
    }
}

impl From<ParamPoint> for [Param; 7] {
    #[inline]
    fn from(value: ParamPoint) -> Self {
        [
            value.temperature,
            value.humidity,
            value.continentalness,
            value.erosion,
            value.depth,
            value.weirdness,
            Param {
                min: value.offset,
                max: value.offset,
            },
        ]
    }
}

struct TargetPoint {
    temperature: QuantizedCoord,
    humidity: QuantizedCoord,
    continentalness: QuantizedCoord,
    erosion: QuantizedCoord,
    depth: QuantizedCoord,
    weirdness: QuantizedCoord,
}

impl TargetPoint {
    #[inline]
    fn new<Q>(
        temperature: Q,
        humidity: Q,
        continentalness: Q,
        erosion: Q,
        depth: Q,
        weirdness: Q,
    ) -> TargetPoint
    where
        Q: Into<QuantizedCoord>,
    {
        TargetPoint {
            temperature: temperature.into(),
            humidity: humidity.into(),
            continentalness: continentalness.into(),
            erosion: erosion.into(),
            depth: depth.into(),
            weirdness: weirdness.into(),
        }
    }
}

impl AsRef<TargetPoint> for TargetPoint {
    #[inline]
    fn as_ref(&self) -> &TargetPoint {
        self
    }
}

impl From<TargetPoint> for [i64; 7] {
    #[inline]
    fn from(value: TargetPoint) -> Self {
        [
            value.temperature.0,
            value.humidity.0,
            value.continentalness.0,
            value.erosion.0,
            value.depth.0,
            value.weirdness.0,
            0,
        ]
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "f64")]
#[serde(into = "f64")]
pub struct QuantizedCoord(pub i64);

impl From<f64> for QuantizedCoord {
    #[inline]
    fn from(value: f64) -> Self {
        QuantizedCoord(QuantizedCoord::quantize_coord(value))
    }
}

impl From<QuantizedCoord> for f64 {
    #[inline]
    fn from(value: QuantizedCoord) -> Self {
        QuantizedCoord::unquantize_coord(value.0)
    }
}

impl From<QuantizedCoord> for i64 {
    #[inline]
    fn from(value: QuantizedCoord) -> Self {
        value.0
    }
}

impl From<i64> for QuantizedCoord {
    #[inline]
    fn from(value: i64) -> Self {
        QuantizedCoord(value)
    }
}

impl QuantizedCoord {
    #[inline]
    fn quantize_coord(coord: f64) -> i64 {
        (coord * 10000.0) as i64
    }

    #[inline]
    fn unquantize_coord(coord: i64) -> f64 {
        coord as f64 / 10000.0
    }
}

impl From<Interval<QuantizedCoord>> for Param {
    #[inline]
    fn from(value: Interval<QuantizedCoord>) -> Self {
        Param {
            min: value.min,
            max: value.max,
        }
    }
}

impl From<Param> for Interval<QuantizedCoord> {
    #[inline]
    fn from(value: Param) -> Self {
        Interval {
            min: value.min,
            max: value.max,
        }
    }
}

struct RTree<T>
where
    T: Clone + PartialEq,
{
    root: RNode<T>,
}

impl<T> RTree<T>
where
    T: Clone + PartialEq + Debug,
{
    pub fn new<I: IntoIterator<Item = (ParamPoint, T)>>(points: I) -> RTree<T> {
        let root = Self::build(
            points
                .into_iter()
                .map(|(point, value)| RNode::new_leaf(point, value))
                .collect(),
        );
        RTree { root }
    }

    pub fn search<R: AsRef<TargetPoint>>(&self, target: R) -> Option<&T> {
        let target = target.as_ref();
        self.root
            .search(
                &[
                    target.temperature.0,
                    target.humidity.0,
                    target.continentalness.0,
                    target.erosion.0,
                    target.depth.0,
                    target.weirdness.0,
                    0,
                ],
                None,
            )
            .and_then(|node| match node {
                RNode::Leaf { value, .. } => Some(value),
                _ => None,
            })
    }

    fn build(mut nodes: Vec<RNode<T>>) -> RNode<T> {
        if nodes.len() == 1 {
            nodes.into_iter().next().unwrap()
        } else if nodes.len() <= 6 {
            nodes.sort_by_key(|node| {
                node.parameter_spec()
                    .iter()
                    .map(|x| ((x.min.0 + x.max.0) / 2).abs())
                    .sum::<i64>()
            });
            RNode::new_subtree(nodes)
        } else {
            let mut f = i64::MAX;
            let mut n3 = -1;
            let mut result = Vec::new();
            for n2 in 0..7 {
                nodes.sort_by_key(|node| Self::sort_key(&node.parameter_spec()[n2], false));
                let list = Self::bucketrize(&nodes);

                let f2 = list
                    .iter()
                    .map(|x| {
                        x.parameters
                            .iter()
                            .map(|x| (x.max.0 - x.min.0).abs())
                            .sum::<i64>()
                    })
                    .sum();

                if f > f2 {
                    f = f2;
                    n3 = n2 as isize;
                    result = list;
                }
            }

            result.sort_by_key(|node| Self::sort_key(&node.parameters[n3 as usize], true));
            RNode::new_subtree(
                result
                    .into_iter()
                    .map(|node| Self::build(node.children))
                    .collect(),
            )
        }
    }

    #[inline]
    fn sort_key(param: &Param, abs: bool) -> i64 {
        let f = (param.min.0 + param.max.0) / 2;
        if abs { f.abs() } else { f }
    }

    fn bucketrize(nodes: &Vec<RNode<T>>) -> Vec<RSubTree<T>> {
        let mut list1 = Vec::new();
        let mut list2 = Vec::new();
        let i = (6.0f64.powf(((nodes.len() as f64) - 0.01).ln() / 6.0f64.ln())) as usize;
        for node in nodes {
            list2.push(node.clone());
            if list2.len() < i {
                continue;
            }
            list1.push(RSubTree::new(list2));
            list2 = Vec::new();
        }
        if !list2.is_empty() {
            list1.push(RSubTree::new(list2));
        }
        list1
    }
}

#[derive(Clone, Debug, PartialEq)]
enum RNode<T>
where
    T: Clone + PartialEq,
{
    Leaf { value: T, parameters: [Param; 7] },
    SubTree(RSubTree<T>),
}

#[derive(Clone, Debug, PartialEq)]
struct RSubTree<T>
where
    T: Clone + PartialEq,
{
    children: Vec<RNode<T>>,
    parameters: [Param; 7],
}

impl<T> RSubTree<T>
where
    T: Clone + PartialEq,
{
    fn new(children: Vec<RNode<T>>) -> Self {
        let mut parameters = children[0].parameter_spec().clone();
        for node in &children[1..] {
            for (i, parameter) in node.parameter_spec().iter().enumerate() {
                parameters[i].min = parameters[i].min.0.min(parameter.min.0).into();
                parameters[i].max = parameters[i].max.0.max(parameter.max.0).into();
            }
        }
        RSubTree {
            children,
            parameters,
        }
    }
}

impl<T> RNode<T>
where
    T: Clone + PartialEq,
{
    fn new_leaf(parameter_point: ParamPoint, value: T) -> Self {
        RNode::Leaf {
            value,
            parameters: parameter_point.into(),
        }
    }

    fn new_subtree(children: Vec<RNode<T>>) -> Self {
        RNode::SubTree(RSubTree::new(children))
    }

    fn parameter_spec(&self) -> &[Param; 7] {
        match &self {
            RNode::Leaf { parameters, .. } => parameters,
            RNode::SubTree(subtree) => &subtree.parameters,
        }
    }

    fn distance<I>(&self, values: &I) -> i64
    where
        I: AsRef<[i64]>,
    {
        values
            .as_ref()
            .iter()
            .zip(self.parameter_spec())
            .map(|(v, p)| p.distance(*v))
            .sum()
    }

    fn search<'a, I>(&'a self, values: &I, mut leaf: Option<&'a RNode<T>>) -> Option<&'a RNode<T>>
    where
        I: AsRef<[i64]>,
    {
        match &self {
            RNode::Leaf { .. } => Some(self),
            RNode::SubTree(subtree) => {
                let mut dist = i64::MAX;
                for node in &subtree.children {
                    let d1 = node.distance(values);
                    if dist > d1 {
                        if let Some(l2) = node.search(values, leaf) {
                            let d2 = if node == l2 { d1 } else { l2.distance(values) };
                            if d2 == 0 {
                                return Some(l2);
                            }
                            if d2 < dist {
                                dist = d2;
                                leaf = Some(l2);
                            }
                        }
                    }
                }
                leaf
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::climate::{ParamPoint, RTree, TargetPoint};

    #[test]
    fn search_test() {
        let tree = RTree::new([
            (
                ParamPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                "red".to_owned(),
            ),
            (
                ParamPoint::new(1.0, 0.0, 0.0, 0.8, 0.0, 0.0, 0),
                "green".to_owned(),
            ),
            (
                ParamPoint::new(1.0, 0.0, 0.6, -0.8, -0.1, 0.0, 0),
                "blue".to_owned(),
            ),
        ]);
        assert_eq!(
            tree.search(TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0))
                .unwrap()
                .as_str(),
            "red"
        );
        assert_eq!(
            tree.search(TargetPoint::new(1.0, 0.0, 0.0, 0.8, 0.0, 0.0))
                .unwrap()
                .as_str(),
            "green"
        );
        assert_eq!(
            tree.search(TargetPoint::new(1.0, 0.0, 0.6, -0.8, -0.1, 0.0))
                .unwrap()
                .as_str(),
            "blue"
        );
    }

    #[test]
    fn complex_test_search() {
        let tree = RTree::new([
            (
                ParamPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                "red".to_owned(),
            ),
            (
                ParamPoint::new(1.0, 0.0, 0.0, 0.8, 0.0, 0.0, 0),
                "green".to_owned(),
            ),
            (
                ParamPoint::new(1.0, 0.0, 0.6, -0.8, -0.1, 0.0, 0),
                "blue".to_owned(),
            ),
            (
                ParamPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0),
                "blue".to_owned(),
            ),
            (
                ParamPoint::new(0.0, 0.2, 0.0, 0.0, 0.0, 0.0, 0),
                "yellow".to_owned(),
            ),
            (
                ParamPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0),
                "orange".to_owned(),
            ),
            (
                ParamPoint::new(0.0, 0.2, 0.0, 0.0, 0.0, 0.9, 0),
                "purple".to_owned(),
            ),
            (
                ParamPoint::new(0.0, -0.3, 0.0, 0.0, 0.0, 0.0, 0),
                "cyan".to_owned(),
            ),
            (
                ParamPoint::new(0.0, -0.9, 0.0, 0.0, 0.0, 0.5, 0),
                "brown".to_owned(),
            ),
            (
                ParamPoint::new(0.0, -0.1, 0.5, 0.0, 0.0, 0.0, 0),
                "black".to_owned(),
            ),
            (
                ParamPoint::new(0.0, 0.7, 0.0, 0.0, 0.0, 0.0, 0),
                "pink".to_owned(),
            ),
        ]);

        assert_eq!(
            tree.search(TargetPoint::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0))
                .unwrap()
                .as_str(),
            "red"
        );
        assert_eq!(
            tree.search(TargetPoint::new(0.4, 0.0, 0.0, 0.0, 0.7, 0.0))
                .unwrap()
                .as_str(),
            "red"
        );
        assert_eq!(
            tree.search(TargetPoint::new(0.0, 0.3, 0.0, -0.2, 0.0, 1.0))
                .unwrap()
                .as_str(),
            "purple"
        );
        assert_eq!(
            tree.search(TargetPoint::new(0.0, 0.0, 0.7, -0.2, 0.0, 0.1))
                .unwrap()
                .as_str(),
            "black"
        );
        assert_eq!(
            tree.search(TargetPoint::new(0.0, 0.6, 0.0, 0.0, 0.0, 0.0))
                .unwrap()
                .as_str(),
            "pink"
        );
    }
}
