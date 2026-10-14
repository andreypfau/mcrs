use crate::metadata::Workspace;
use crate::mutation::plan::{ShardPlan, Unit};
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Lane {
    pub key: String,
    pub units: Vec<usize>,
    pub shards: Vec<usize>,
}

fn build_graph<'a>(workspace: &'a Workspace, unit: &'a Unit) -> BTreeSet<&'a str> {
    let mut graph = BTreeSet::new();
    let mut pending: Vec<&str> = std::iter::once(unit.package.as_str())
        .chain(unit.test_packages.iter().map(String::as_str))
        .collect();
    while let Some(name) = pending.pop() {
        if !graph.insert(name) {
            continue;
        }
        if let Some(package) = workspace.packages.iter().find(|p| p.name == name) {
            pending.extend(
                package
                    .dependencies
                    .iter()
                    .filter(|dependency| dependency.local)
                    .map(|dependency| dependency.name.as_str()),
            );
        }
    }
    graph
}

pub(crate) fn lanes(workspace: &Workspace, units: &[Unit], shards: &[ShardPlan]) -> Vec<Lane> {
    let mut groups: Vec<(BTreeSet<&str>, Vec<usize>)> = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        if !shards.iter().any(|planned| planned.unit == index) {
            continue;
        }
        let mut graph = build_graph(workspace, unit);
        let mut joined = vec![index];
        let (touching, apart): (Vec<_>, Vec<_>) = groups
            .into_iter()
            .partition(|(other, _)| !other.is_disjoint(&graph));
        groups = apart;
        for (other, members) in touching {
            graph.extend(other);
            joined.extend(members);
        }
        joined.sort_unstable();
        groups.push((graph, joined));
    }
    groups.sort_by_key(|(_, members)| members[0]);
    groups
        .into_iter()
        .map(|(_, members)| Lane {
            key: members
                .iter()
                .map(|&unit| units[unit].package.as_str())
                .min()
                .unwrap_or_default()
                .to_owned(),
            shards: shards
                .iter()
                .enumerate()
                .filter(|(_, planned)| members.contains(&planned.unit))
                .map(|(index, _)| index)
                .collect(),
            units: members,
        })
        .collect()
}

pub(crate) fn largest(lanes: &[Lane]) -> Option<usize> {
    lanes
        .iter()
        .enumerate()
        .max_by_key(|(index, lane)| (lane.units.len(), std::cmp::Reverse(*index)))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{Dependency, Kind, Package};

    fn workspace(edges: &[(&str, &[&str])]) -> Workspace {
        Workspace {
            packages: edges
                .iter()
                .map(|(name, deps)| Package {
                    name: (*name).to_owned(),
                    dir: format!("crates/{name}"),
                    edition: "2024".to_owned(),
                    dependencies: deps
                        .iter()
                        .map(|dep| Dependency {
                            name: (*dep).to_owned(),
                            kind: Kind::Normal,
                            local: true,
                        })
                        .collect(),
                    targets: Vec::new(),
                })
                .collect(),
        }
    }

    fn unit(package: &str, tested: &[&str]) -> Unit {
        Unit {
            package: package.to_owned(),
            test_packages: tested.iter().map(|name| (*name).to_owned()).collect(),
            names: vec!["a mutant".to_owned()],
            floor: None,
        }
    }

    fn shard(unit: usize, index: u32, count: u32) -> ShardPlan {
        ShardPlan { unit, index, count }
    }

    fn lane(key: &str, units: &[usize], shards: &[usize]) -> Lane {
        Lane {
            key: key.to_owned(),
            units: units.to_vec(),
            shards: shards.to_vec(),
        }
    }

    fn one_shard_each(count: usize) -> Vec<ShardPlan> {
        (0..count).map(|unit| shard(unit, 0, 1)).collect()
    }

    fn chain_and_tool() -> Workspace {
        workspace(&[
            ("low", &[]),
            ("mid", &["low"]),
            ("high", &["mid"]),
            ("tool", &[]),
        ])
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one build-graph fixture per way packages can share dependencies, asserted together"
    )]
    fn units_are_split_where_their_build_graphs_share_no_package() {
        let diamond = workspace(&[
            ("shared", &[]),
            ("left", &["shared"]),
            ("right", &["shared"]),
            ("a", &["x"]),
            ("b", &["y"]),
            ("c", &["x", "y"]),
            ("x", &[]),
            ("y", &[]),
            ("alpha", &["shared"]),
            ("zeta", &["shared"]),
        ]);
        let mut with_dev = workspace(&[("shared", &[]), ("left", &[]), ("right", &["shared"])]);
        with_dev.packages[1].dependencies.push(Dependency {
            name: "shared".to_owned(),
            kind: Kind::Dev,
            local: true,
        });
        let mut external = workspace(&[("left", &[]), ("right", &[])]);
        for package in &mut external.packages {
            package.dependencies.push(Dependency {
                name: "serde".to_owned(),
                kind: Kind::Normal,
                local: false,
            });
        }

        struct Row {
            name: &'static str,
            workspace: Workspace,
            units: Vec<Unit>,
            shards: Vec<ShardPlan>,
            expected: Vec<Lane>,
        }
        let rows = vec![
            Row {
                name: "a change to the middle of a chain and to an unrelated tool",
                workspace: chain_and_tool(),
                units: vec![unit("mid", &["mid", "high"]), unit("tool", &["tool"])],
                shards: one_shard_each(2),
                expected: vec![lane("mid", &[0], &[0]), lane("tool", &[1], &[1])],
            },
            Row {
                name: "the bottom and the top of a chain",
                workspace: chain_and_tool(),
                units: vec![
                    unit("low", &["low", "mid", "high"]),
                    unit("high", &["high"]),
                ],
                shards: one_shard_each(2),
                expected: vec![lane("high", &[0, 1], &[0, 1])],
            },
            Row {
                name: "a unit reaches the bottom of a chain only through the middle",
                workspace: chain_and_tool(),
                units: vec![unit("high", &["high"]), unit("low", &["low"])],
                shards: one_shard_each(2),
                expected: vec![lane("high", &[0, 1], &[0, 1])],
            },
            Row {
                name: "two crates that only share a third dependency",
                workspace: diamond,
                units: vec![unit("left", &["left"]), unit("right", &["right"])],
                shards: one_shard_each(2),
                expected: vec![lane("left", &[0, 1], &[0, 1])],
            },
            Row {
                name: "a single unit",
                workspace: chain_and_tool(),
                units: vec![unit("tool", &["tool"])],
                shards: vec![shard(0, 0, 2), shard(0, 1, 2)],
                expected: vec![lane("tool", &[0], &[0, 1])],
            },
            Row {
                name: "a third unit that joins two lanes",
                workspace: workspace(&[
                    ("a", &["x"]),
                    ("b", &["y"]),
                    ("c", &["x", "y"]),
                    ("x", &[]),
                    ("y", &[]),
                ]),
                units: vec![unit("a", &["a"]), unit("b", &["b"]), unit("c", &["c"])],
                shards: one_shard_each(3),
                expected: vec![lane("a", &[0, 1, 2], &[0, 1, 2])],
            },
            Row {
                name: "lanes and shards keep plan order and the key is the first package",
                workspace: workspace(&[
                    ("tool", &[]),
                    ("zeta", &["shared"]),
                    ("alpha", &["shared"]),
                    ("shared", &[]),
                ]),
                units: vec![
                    unit("tool", &["tool"]),
                    unit("zeta", &["zeta"]),
                    unit("alpha", &["alpha"]),
                ],
                shards: vec![
                    shard(0, 0, 2),
                    shard(1, 0, 1),
                    shard(0, 1, 2),
                    shard(2, 0, 1),
                ],
                expected: vec![lane("tool", &[0], &[0, 2]), lane("alpha", &[1, 2], &[1, 3])],
            },
            Row {
                name: "a dependency of the development kind joins the graphs",
                workspace: with_dev,
                units: vec![unit("left", &["left"]), unit("right", &["right"])],
                shards: one_shard_each(2),
                expected: vec![lane("left", &[0, 1], &[0, 1])],
            },
            Row {
                name: "an external crate in common joins nothing",
                workspace: external,
                units: vec![unit("left", &["left"]), unit("right", &["right"])],
                shards: one_shard_each(2),
                expected: vec![lane("left", &[0], &[0]), lane("right", &[1], &[1])],
            },
            Row {
                name: "a unit without shards starts no lane",
                workspace: chain_and_tool(),
                units: vec![unit("tool", &["tool"]), unit("low", &["low"])],
                shards: vec![shard(1, 0, 1)],
                expected: vec![lane("low", &[1], &[0])],
            },
            Row {
                name: "no units",
                workspace: chain_and_tool(),
                units: Vec::new(),
                shards: Vec::new(),
                expected: Vec::new(),
            },
        ];
        let mut problems = Vec::new();
        for Row {
            name,
            workspace,
            units,
            shards,
            expected,
        } in rows
        {
            let found = lanes(&workspace, &units, &shards);
            if found != expected {
                problems.push(format!("{name}: expected {expected:?}, found {found:?}"));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    #[test]
    fn the_lane_with_the_most_units_is_the_largest_and_the_first_wins_a_tie() {
        let found = [
            lane("a", &[0], &[0]),
            lane("b", &[1, 2], &[1, 2]),
            lane("c", &[3, 4], &[3, 4]),
        ];
        assert_eq!(largest(&found), Some(1));
        assert_eq!(largest(&found[..1]), Some(0));
        assert_eq!(largest(&[]), None);
    }
}
