use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone)]
pub enum ConfigNode<L> {
    Leaf(L),
    Tuple(Vec<ConfigNode<L>>),
    Configured {
        inner: Box<ConfigNode<L>>,
        placement: Placement,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Placement {
    pub run_if: bool,
    pub on_event: bool,
    pub sets: Vec<String>,
}

impl Placement {
    #[must_use]
    pub fn within(&self, outer: &Placement) -> Placement {
        let mut sets = outer.sets.clone();
        sets.extend(self.sets.iter().cloned());
        Placement {
            run_if: self.run_if || outer.run_if,
            on_event: self.on_event || outer.on_event,
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
            ConfigNode::Configured { inner, placement } => {
                inner.place(&placement.within(outer), out);
            }
        }
    }
}
