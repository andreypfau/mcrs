use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A vertex of the ordering graph: `(step, index)`, compared step first, where
/// `index` counts distinct features in order of first encounter.
type Vertex = (usize, usize);

/// Orders every feature the given biomes carry into one list per decoration
/// step, so that no biome's own list ever runs out of order.
///
/// A biome is a list of steps, a step a list of identity tokens: two entries
/// with the same token are the same feature and share a vertex, and equal
/// tokens are the caller's statement of identity, not of equal shape. The
/// error is the token the walk found a cycle through.
pub fn build_features_per_step(biomes: &[Vec<Vec<usize>>]) -> Result<Vec<Vec<usize>>, usize> {
    let mut vertex_of: HashMap<usize, usize> = HashMap::new();
    let mut token_of: Vec<usize> = Vec::new();
    let mut edges: BTreeMap<Vertex, BTreeSet<Vertex>> = BTreeMap::new();
    let mut max_step = 0;

    for steps in biomes {
        max_step = max_step.max(steps.len());

        let mut list: Vec<Vertex> = Vec::new();
        for (step, entries) in steps.iter().enumerate() {
            for &token in entries {
                let index = *vertex_of.entry(token).or_insert_with(|| {
                    token_of.push(token);
                    token_of.len() - 1
                });
                list.push((step, index));
            }
        }

        for i in 0..list.len() {
            let successors = edges.entry(list[i]).or_default();
            if let Some(&next) = list.get(i + 1) {
                successors.insert(next);
            }
        }
    }

    let mut discovered: BTreeSet<Vertex> = BTreeSet::new();
    let mut visiting: BTreeSet<Vertex> = BTreeSet::new();
    let mut sorted: Vec<Vertex> = Vec::new();
    for &vertex in edges.keys() {
        if !discovered.contains(&vertex)
            && let Some(cycle) =
                depth_first(&edges, &mut discovered, &mut visiting, &mut sorted, vertex)
        {
            return Err(token_of[cycle.1]);
        }
    }
    sorted.reverse();

    Ok((0..max_step)
        .map(|step| {
            sorted
                .iter()
                .filter(|(vertex_step, _)| *vertex_step == step)
                .map(|&(_, index)| token_of[index])
                .collect()
        })
        .collect())
}

fn depth_first(
    edges: &BTreeMap<Vertex, BTreeSet<Vertex>>,
    discovered: &mut BTreeSet<Vertex>,
    visiting: &mut BTreeSet<Vertex>,
    out: &mut Vec<Vertex>,
    current: Vertex,
) -> Option<Vertex> {
    if discovered.contains(&current) {
        return None;
    }
    if !visiting.insert(current) {
        return Some(current);
    }
    for &next in edges.get(&current).into_iter().flatten() {
        if let Some(cycle) = depth_first(edges, discovered, visiting, out, next) {
            return Some(cycle);
        }
    }
    visiting.remove(&current);
    discovered.insert(current);
    out.push(current);
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

    #[test]
    fn a_cycle_names_a_feature_on_it() {
        let steps = vec![
            vec![vec![1, 2]],
            vec![vec![3, 4]],
            vec![vec![2, 1]],
            vec![vec![5]],
        ];
        assert!(matches!(build_features_per_step(&steps), Err(1 | 2)));
    }
}
