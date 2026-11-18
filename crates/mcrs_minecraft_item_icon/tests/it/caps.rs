use mcrs_minecraft_item_icon::models_over_caps;
use mcrs_minecraft_model::item_model_asset::{ConditionProperty, TintSource};
use mcrs_minecraft_model::item_model_bake::BakedNode;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::corpus::MODELS;

#[test]
fn no_vanilla_item_model_exceeds_the_caps_of_an_icon() {
    let over: Vec<&ResourceLocationBuf> = models_over_caps(&MODELS).collect();
    assert!(over.is_empty(), "{over:?}");
    assert!(
        MODELS.by_id.len() > 1000,
        "the corpus holds the vanilla items"
    );
}

struct Row {
    name: &'static str,
    node: BakedNode,
    over_caps: bool,
}

#[test]
fn an_item_model_over_the_caps_is_listed() {
    let BakedNode::Model { model, .. } = MODELS.missing else {
        panic!("the missing item model is one model");
    };
    let tinted = |count: usize| BakedNode::Model {
        model,
        tints: vec![
            TintSource::Grass {
                temperature: 0.5,
                downfall: 0.5,
            };
            count
        ],
    };
    let layers = |count: usize| BakedNode::Composite(vec![tinted(0); count]);
    let rows = [
        Row {
            name: "five_layers",
            node: layers(5),
            over_caps: true,
        },
        Row {
            name: "four_layers",
            node: layers(4),
            over_caps: false,
        },
        Row {
            name: "five_tints",
            node: tinted(5),
            over_caps: true,
        },
        Row {
            name: "four_tints",
            node: tinted(4),
            over_caps: false,
        },
        Row {
            name: "two_branches_of_four_layers",
            node: BakedNode::Condition {
                property: ConditionProperty::Damaged,
                on_true: Box::new(layers(4)),
                on_false: Box::new(layers(4)),
            },
            over_caps: false,
        },
    ];
    let mut models = MODELS.clone();
    for row in rows {
        let key = ResourceLocationBuf::new("pack", row.name).expect("the key is valid");
        models.by_id.clear();
        models.by_id.insert(key.clone(), row.node);
        let over: Vec<&ResourceLocationBuf> = models_over_caps(&models).collect();
        assert_eq!(over.contains(&&key), row.over_caps, "{}", row.name);
    }
}
