use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_model::item_model_asset::TintSource;
use mcrs_minecraft_model::item_model_bake::{BakedNode, ItemModels};
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::item_model_eval::{self, Evaluator};
use mcrs_minecraft_model::model_id::ModelId;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_model_file::model_file::GuiLight;

use crate::icon_layer::IconLayer;
use crate::icon_sources::IconSources;
use crate::item_icon::ItemIcon;

#[must_use]
pub fn resolve(
    view: StackView<'_>,
    sources: &IconSources<'_>,
    context: &ItemModelContext,
) -> ItemIcon {
    let models = sources.models;
    let root = view
        .table
        .effective::<ItemModel>(view.stack)
        .and_then(|model| models.by_id.get(&model.get()))
        .unwrap_or(&models.missing);
    let mut walk = Walk {
        evaluator: Evaluator {
            stack: view,
            registries: sources.registries,
            grass_colormap: models.grass_colormap.as_deref(),
            context: *context,
        },
        models,
        icon: ItemIcon {
            layers: [None; ItemIcon::MAX_LAYERS],
            foil: view.has_foil(),
            gui_light: GuiLight::Front,
            dynamic: false,
        },
    };
    walk.visit(root);
    walk.icon
}

struct Walk<'a> {
    evaluator: Evaluator<'a>,
    models: &'a ItemModels,
    icon: ItemIcon,
}

impl Walk<'_> {
    fn visit(&mut self, node: &BakedNode) {
        match node {
            BakedNode::Empty => {}
            BakedNode::BundleSelectedItem => self.icon.dynamic = true,
            BakedNode::Model { model, tints } => self.push(*model, tints),
            BakedNode::Special { properties, .. } => self.push(*properties, &[]),
            BakedNode::Composite(children) => {
                for child in children {
                    self.visit(child);
                }
            }
            BakedNode::Condition {
                property,
                on_true,
                on_false,
            } => {
                self.icon.dynamic |= item_model_eval::condition_reads_context(property);
                if self.evaluator.condition(property) {
                    self.visit(on_true);
                } else {
                    self.visit(on_false);
                }
            }
            BakedNode::Select {
                switch,
                cases,
                fallback,
            } => {
                self.icon.dynamic |= item_model_eval::select_reads_context(switch);
                let chosen = self
                    .evaluator
                    .select(switch)
                    .and_then(|index| cases.get(index))
                    .unwrap_or(fallback);
                self.visit(chosen);
            }
            BakedNode::RangeDispatch {
                property,
                scale,
                thresholds,
                models,
                fallback,
            } => {
                self.icon.dynamic |= item_model_eval::range_reads_context(property);
                let value = self.evaluator.range(property) * scale;
                let chosen = BakedNode::range_index(thresholds, value)
                    .and_then(|index| models.get(index))
                    .unwrap_or(fallback);
                self.visit(chosen);
            }
        }
    }

    fn push(&mut self, model: ModelId, sources: &[TintSource]) {
        let Some(baked) = self.models.model(model) else {
            return;
        };
        let Some(free) = self.icon.layers.iter().position(Option::is_none) else {
            return;
        };
        let mut tints = [IconLayer::UNTINTED; IconLayer::MAX_TINTS];
        for (tint, source) in tints.iter_mut().zip(sources) {
            self.icon.dynamic |= item_model_eval::tint_reads_context(source);
            *tint = self.evaluator.tint(source);
        }
        if free == 0 {
            self.icon.gui_light = baked.gui_light;
        }
        if let Some(slot) = self.icon.layers.get_mut(free) {
            *slot = Some(IconLayer { model, tints });
        }
    }
}
