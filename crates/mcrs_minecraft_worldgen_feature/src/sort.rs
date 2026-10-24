use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;

/// A vertex of the ordering graph: `(step, index, token)`, compared step first,
/// where `index` counts distinct tokens in order of first encounter. One index
/// always carries one token, so the token never decides an order.
type Vertex<T> = (usize, usize, T);

/// Orders every feature the given biomes carry into one list per decoration
/// step, so that no biome's own list ever runs out of order.
///
/// A biome is a list of steps, a step a list of identity tokens: two entries
/// with the same token are the same feature and share a vertex, and equal
/// tokens are the caller's statement of identity, not of equal shape. The
/// error is the token the walk found a cycle through.
pub fn build_features_per_step<T: Clone + Eq + Hash + Ord>(
    biomes: &[Vec<Vec<T>>],
) -> Result<Vec<Vec<T>>, T> {
    let mut vertex_of: HashMap<T, usize> = HashMap::new();
    let mut edges: BTreeMap<Vertex<T>, BTreeSet<Vertex<T>>> = BTreeMap::new();
    let mut max_step = 0;

    for steps in biomes {
        max_step = max_step.max(steps.len());

        let mut list: Vec<Vertex<T>> = Vec::new();
        for (step, entries) in steps.iter().enumerate() {
            for token in entries {
                let next_index = vertex_of.len();
                let index = *vertex_of.entry(token.clone()).or_insert(next_index);
                list.push((step, index, token.clone()));
            }
        }

        for (i, vertex) in list.iter().enumerate() {
            let successors = edges.entry(vertex.clone()).or_default();
            if let Some(next) = list.get(i + 1) {
                successors.insert(next.clone());
            }
        }
    }

    let mut discovered: BTreeSet<Vertex<T>> = BTreeSet::new();
    let mut visiting: BTreeSet<Vertex<T>> = BTreeSet::new();
    let mut sorted: Vec<Vertex<T>> = Vec::new();
    for vertex in edges.keys() {
        if !discovered.contains(vertex)
            && let Some(cycle) =
                depth_first(&edges, &mut discovered, &mut visiting, &mut sorted, vertex)
        {
            return Err(cycle.2);
        }
    }
    sorted.reverse();

    Ok((0..max_step)
        .map(|step| {
            sorted
                .iter()
                .filter(|(vertex_step, ..)| *vertex_step == step)
                .map(|(_, _, token)| token.clone())
                .collect()
        })
        .collect())
}

fn depth_first<T: Clone + Ord>(
    edges: &BTreeMap<Vertex<T>, BTreeSet<Vertex<T>>>,
    discovered: &mut BTreeSet<Vertex<T>>,
    visiting: &mut BTreeSet<Vertex<T>>,
    out: &mut Vec<Vertex<T>>,
    current: &Vertex<T>,
) -> Option<Vertex<T>> {
    if discovered.contains(current) {
        return None;
    }
    if !visiting.insert(current.clone()) {
        return Some(current.clone());
    }
    for next in edges.get(current).into_iter().flatten() {
        if let Some(cycle) = depth_first(edges, discovered, visiting, out, next) {
            return Some(cycle);
        }
    }
    visiting.remove(current);
    discovered.insert(current.clone());
    out.push(current.clone());
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_features_come_out_in_reverse_order_of_first_encounter() {
        let steps = build_features_per_step(&[vec![vec![7]], vec![vec![2]]]).unwrap();
        assert_eq!(steps, vec![vec![2, 7]]);
    }

    #[test]
    fn keeps_each_biome_in_its_own_order() {
        let steps = build_features_per_step(&[
            vec![vec![10, 20, 30]],
            vec![vec![20, 40]],
            vec![vec![50], vec![60]],
        ])
        .unwrap();
        assert_eq!(steps, vec![vec![50, 10, 20, 40, 30], vec![60]]);
    }

    #[test]
    fn the_same_feature_at_two_steps_is_two_vertices() {
        let steps = build_features_per_step(&[vec![vec![1], vec![1]]]).unwrap();
        assert_eq!(steps, vec![vec![1], vec![1]]);
    }
}
