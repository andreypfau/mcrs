use mcrs_minecraft_item_icon::{IconLayer, ItemIcon};
use mcrs_minecraft_model::item_model_bake::BakedNode;

use crate::corpus::MODELS;

#[derive(Clone, Copy, Default)]
struct Reach {
    layers: usize,
    tints: usize,
}

fn widest<'a>(nodes: impl Iterator<Item = &'a BakedNode>) -> Reach {
    nodes.map(reach).fold(Reach::default(), |a, b| Reach {
        layers: a.layers.max(b.layers),
        tints: a.tints.max(b.tints),
    })
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
        BakedNode::Composite(children) => {
            children
                .iter()
                .map(reach)
                .fold(Reach::default(), |a, b| Reach {
                    layers: a.layers + b.layers,
                    tints: a.tints.max(b.tints),
                })
        }
        BakedNode::Condition {
            on_true, on_false, ..
        } => widest([&**on_true, &**on_false].into_iter()),
        BakedNode::Select {
            cases, fallback, ..
        } => widest(cases.iter().chain([&**fallback])),
        BakedNode::RangeDispatch {
            models, fallback, ..
        } => widest(models.iter().chain([&**fallback])),
    }
}

#[test]
fn no_vanilla_item_model_exceeds_the_caps_of_an_icon() {
    let mut largest = Reach::default();
    for (id, node) in &MODELS.by_id {
        let item = reach(node);
        assert!(
            item.layers <= ItemIcon::MAX_LAYERS && item.tints <= IconLayer::MAX_TINTS,
            "{id}: {} layers, {} tints",
            item.layers,
            item.tints
        );
        largest.layers = largest.layers.max(item.layers);
        largest.tints = largest.tints.max(item.tints);
    }
    assert!(
        MODELS.by_id.len() > 1000,
        "the corpus holds the vanilla items"
    );
    println!(
        "items {}, at most {} layers and {} tints",
        MODELS.by_id.len(),
        largest.layers,
        largest.tints
    );
}
