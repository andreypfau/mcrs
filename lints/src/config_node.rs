#[derive(Debug, Clone)]
pub enum ConfigNode<L> {
    Leaf(L),
    Tuple(Vec<ConfigNode<L>>),
    Configured {
        inner: Box<ConfigNode<L>>,
        run_if: bool,
        sets: Vec<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Placement {
    pub run_if: bool,
    pub sets: Vec<String>,
}

impl Placement {
    #[must_use]
    pub fn within(&self, outer: &Placement) -> Placement {
        let mut sets = outer.sets.clone();
        sets.extend(self.sets.iter().cloned());
        Placement {
            run_if: self.run_if || outer.run_if,
            sets,
        }
    }
}

impl<L: Clone> ConfigNode<L> {
    #[must_use]
    pub fn placements(&self) -> Vec<(L, Placement)> {
        let mut out = Vec::new();
        self.place(&Placement::default(), &mut out);
        out
    }

    fn place(&self, outer: &Placement, out: &mut Vec<(L, Placement)>) {
        match self {
            ConfigNode::Leaf(leaf) => out.push((leaf.clone(), outer.clone())),
            ConfigNode::Tuple(children) => {
                for child in children {
                    child.place(outer, out);
                }
            }
            ConfigNode::Configured {
                inner,
                run_if,
                sets,
            } => {
                let own = Placement {
                    run_if: *run_if,
                    sets: sets.clone(),
                };
                inner.place(&own.within(outer), out);
            }
        }
    }
}
