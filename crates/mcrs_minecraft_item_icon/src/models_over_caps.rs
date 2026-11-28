use mcrs_minecraft_model::item_model_bake::{BakedNode, ItemModels};
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::icon_layer::IconLayer;
use crate::item_icon::ItemIcon;

pub fn models_over_caps(models: &ItemModels) -> impl Iterator<Item = &ResourceLocationBuf> {
    models
        .by_id
        .iter()
        .filter(|(_, node)| reach(node).is_over_caps())
        .map(|(id, _)| id)
}

#[derive(Clone, Copy, Default)]
struct Reach {
    layers: usize,
    tints: usize,
}

impl Reach {
    fn is_over_caps(self) -> bool {
        self.layers > ItemIcon::MAX_LAYERS || self.tints > IconLayer::MAX_TINTS
    }

    fn widest(self, other: Self) -> Self {
        Self {
            layers: self.layers.max(other.layers),
            tints: self.tints.max(other.tints),
        }
    }

    fn stacked(self, other: Self) -> Self {
        Self {
            layers: self.layers.saturating_add(other.layers),
            tints: self.tints.max(other.tints),
        }
    }
}

fn reach(node: &BakedNode) -> Reach {
    match node {
        BakedNode::Empty | BakedNode::BundleSelectedItem => Reach::default(),
        BakedNode::Model { tints, .. } => Reach {
            layers: 1,
            tints: tints.len(),
        },
        BakedNode::Special { .. } => Reach {
            layers: 1,
            tints: 0,
        },
        BakedNode::Composite(children) => children
            .iter()
            .map(reach)
            .fold(Reach::default(), Reach::stacked),
        BakedNode::Condition {
            on_true, on_false, ..
        } => reach(on_true).widest(reach(on_false)),
        BakedNode::Select {
            cases, fallback, ..
        } => widest(cases.iter().chain([&**fallback])),
        BakedNode::RangeDispatch {
            models, fallback, ..
        } => widest(models.iter().chain([&**fallback])),
    }
}

fn widest<'a>(nodes: impl Iterator<Item = &'a BakedNode>) -> Reach {
    nodes.map(reach).fold(Reach::default(), Reach::widest)
}
