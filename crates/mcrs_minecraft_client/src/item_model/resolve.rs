use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::SystemSet;
use bevy::ecs::world::EntityRef;
use bevy::math::{EulerRot, Mat3, Mat4, Quat, Vec3};
use bevy::prelude::{
    Commands, Component, DetectChanges, Entity, Has, IntoScheduleConfigs, Query, Ref, Res,
    SystemCondition, With, resource_exists,
};
use bytemuck::{Pod, Zeroable};
use mcrs_minecraft_item::{
    Held, ItemStack, Items, StackRevision, bundle_weight, children, component_value, damage_value,
    has_component, has_foil, has_non_default, is_damaged, max_damage, max_stack_size,
    next_damage_will_break,
};
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_protocol::item::{
    BlockState, CustomModelData, DyedColor, FireworkExplosion, Holder, ItemDataComponent,
    ItemModel, PotionContents, Trim,
};

use super::asset::{
    Case, ChargeType, ConditionProperty, DisplayContext, RangeProperty, SelectSwitch, TintSource,
};
use super::bake::{BakedItemModel, BakedNode, ItemModels};
use crate::blocks::sample_colormap;
use crate::model::{GuiLight, ItemTransform};

/// A slot-local GUI vertex: `pos` in pixels from the slot's top-left, `y` down and
/// `z` out of the screen; `sprite` is the sprite id under the flag bits. The four vertices of a
/// quad wind so that `(p1 - p0) × (p2 - p0)` points along the face's own direction.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GuiVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub sprite: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foil {
    None,
    Standard,
    Special,
}

#[derive(Debug)]
pub struct RenderLayer {
    pub vertices: Vec<GuiVertex>,
    pub foil: Foil,
}

/// The GUI projection of one held stack, recomputed when its revision changes.
#[derive(Component, Debug)]
pub struct ItemRenderLayers {
    pub layers: Vec<RenderLayer>,
    pub gui_light: GuiLight,
    pub oversized_in_gui: bool,
    pub animated: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemRenderSet {
    Resolve,
}

pub struct ItemRenderPlugin;

impl Plugin for ItemRenderPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            ItemRenderSet::Resolve.after(ClientNetworkSystems::Receive),
        )
        .add_systems(
            Update,
            resolve_item_layers
                .in_set(ItemRenderSet::Resolve)
                .run_if(resource_exists::<Items>.and_then(resource_exists::<ItemModels>)),
        );
    }
}

pub fn resolve_item_layers(
    items: Res<Items>,
    models: Res<ItemModels>,
    stacks: Query<(Entity, Ref<StackRevision>, Has<ItemRenderLayers>), With<Held>>,
    reads: Query<EntityRef, With<ItemStack>>,
    mut commands: Commands,
) {
    let all = models.is_changed();
    for (entity, revision, resolved) in &stacks {
        if !(all || !resolved || revision.is_changed()) {
            continue;
        }
        let Ok(stack) = reads.get(entity) else {
            continue;
        };
        let lookup = |child| reads.get(child).ok();
        commands
            .entity(entity)
            .insert(resolve(stack, &items, &models, &lookup));
    }
}

pub fn resolve<'a>(
    stack: EntityRef<'a>,
    items: &Items,
    models: &ItemModels,
    lookup: &impl Fn(Entity) -> Option<EntityRef<'a>>,
) -> ItemRenderLayers {
    let item = match stack.get::<ItemModel>() {
        Some(ItemModel(id)) => models.get(id.as_str()),
        None => &models.missing,
    };
    let evaluator = Evaluator {
        stack,
        items,
        models,
        lookup,
    };
    let mut layers = Vec::new();
    evaluator.collect(&item.root, &mut layers);
    let gui_light = layers
        .first()
        .map_or(GuiLight::Front, |layer| layer.model.gui_light);
    let foil = if has_foil(stack, items) {
        Foil::Standard
    } else {
        Foil::None
    };
    ItemRenderLayers {
        animated: layers.iter().any(|layer| layer.model.animated),
        layers: layers
            .into_iter()
            .map(|layer| RenderLayer {
                vertices: gui_vertices(layer.model, layer.transform, &layer.tints, gui_light),
                foil,
            })
            .collect(),
        gui_light,
        oversized_in_gui: item.oversized_in_gui,
    }
}

struct Layer<'m> {
    model: &'m Arc<BakedItemModel>,
    transform: Mat4,
    tints: Vec<u32>,
}

struct Evaluator<'a, 'l, L> {
    stack: EntityRef<'a>,
    items: &'l Items,
    models: &'l ItemModels,
    lookup: L,
}

fn opaque(color: i32) -> u32 {
    color as u32 | 0xFF00_0000
}

impl<'a, L: Fn(Entity) -> Option<EntityRef<'a>>> Evaluator<'a, '_, L> {
    fn collect<'m>(&self, node: &'m BakedNode, out: &mut Vec<Layer<'m>>) {
        match node {
            BakedNode::Empty | BakedNode::BundleSelectedItem => {}
            BakedNode::Model {
                model,
                tints,
                transform,
            } => out.push(Layer {
                model,
                transform: *transform,
                tints: tints.iter().map(|tint| self.tint(tint)).collect(),
            }),
            // ponytail: a special model contributes its lighting and an empty layer;
            // chests, banners, heads and the like draw nothing until they get renderers.
            BakedNode::Special {
                properties,
                transform,
                ..
            } => out.push(Layer {
                model: properties,
                transform: *transform,
                tints: Vec::new(),
            }),
            BakedNode::Composite(children) => {
                for child in children {
                    self.collect(child, out);
                }
            }
            BakedNode::Condition {
                property,
                on_true,
                on_false,
            } => self.collect(
                if self.condition(property) {
                    on_true
                } else {
                    on_false
                },
                out,
            ),
            BakedNode::Select {
                switch,
                cases,
                fallback,
            } => self.collect(self.select(switch).map_or(&**fallback, |i| &cases[i]), out),
            BakedNode::RangeDispatch {
                property,
                scale,
                thresholds,
                models,
                fallback,
            } => {
                let value = self.range(property) * scale;
                let chosen =
                    BakedNode::range_index(thresholds, value).map_or(&**fallback, |i| &models[i]);
                self.collect(chosen, out);
            }
        }
    }

    fn get<K: ItemDataComponent + Component>(&self) -> Option<&K> {
        self.stack.get::<K>()
    }

    fn custom_model_data(&self) -> Option<&CustomModelData> {
        self.get::<CustomModelData>()
    }

    fn condition(&self, property: &ConditionProperty) -> bool {
        match property {
            ConditionProperty::Damaged => is_damaged(self.stack),
            ConditionProperty::Broken => next_damage_will_break(self.stack),
            ConditionProperty::HasComponent {
                component,
                ignore_default,
            } => {
                if *ignore_default {
                    has_non_default(self.stack, self.items, component.0)
                } else {
                    has_component(self.stack, self.items, component.0)
                }
            }
            ConditionProperty::CustomModelData { index } => {
                self.custom_model_data()
                    .and_then(|data| data.flags.get(*index as usize))
                    == Some(&true)
            }
            ConditionProperty::Component(_)
            | ConditionProperty::UsingItem
            | ConditionProperty::Selected
            | ConditionProperty::Carried
            | ConditionProperty::ExtendedView
            | ConditionProperty::KeybindDown { .. }
            | ConditionProperty::ViewEntity
            | ConditionProperty::FishingRodCast
            | ConditionProperty::BundleHasSelectedItem => false,
        }
    }

    fn select(&self, switch: &SelectSwitch) -> Option<usize> {
        fn find<T: PartialEq>(cases: &[Case<T>], value: &T) -> Option<usize> {
            cases.iter().position(|case| case.when.contains(value))
        }
        match switch {
            SelectSwitch::TrimMaterial { cases } => match &self.get::<Trim>()?.material {
                Holder::Reference(key) => find(cases, key),
                Holder::Direct(_) => None,
            },
            SelectSwitch::DisplayContext { cases } => find(cases, &DisplayContext::Gui),
            SelectSwitch::BlockState {
                block_state_property,
                cases,
            } => find(
                cases,
                self.get::<BlockState>()?.0.get(block_state_property)?,
            ),
            SelectSwitch::ChargeType { cases } => find(cases, &self.charge_type()),
            SelectSwitch::CustomModelData { index, cases } => find(
                cases,
                self.custom_model_data()?.strings.get(*index as usize)?,
            ),
            SelectSwitch::Component(switch) => {
                let value = component_value(self.stack, switch.component)?;
                switch
                    .cases
                    .iter()
                    .position(|case| case.when.iter().any(|when| when.0 == value))
            }
            SelectSwitch::MainHand { .. }
            | SelectSwitch::LocalTime { .. }
            | SelectSwitch::ContextEntityType { .. }
            | SelectSwitch::ContextDimension { .. } => None,
        }
    }

    fn charge_type(&self) -> ChargeType {
        let projectiles = children(self.stack, &self.lookup);
        if projectiles.is_empty() {
            return ChargeType::None;
        }
        let rocket = projectiles.iter().any(|child| {
            child
                .get::<ItemStack>()
                .and_then(|stack| self.items.get(stack.item()))
                .is_some_and(|entry| entry.identifier.as_str() == "minecraft:firework_rocket")
        });
        if rocket {
            ChargeType::Rocket
        } else {
            ChargeType::Arrow
        }
    }

    fn range(&self, property: &RangeProperty) -> f32 {
        match property {
            RangeProperty::Damage { normalize } => {
                let damage = damage_value(self.stack) as f32;
                let max = max_damage(self.stack) as f32;
                if *normalize {
                    (damage / max).clamp(0.0, 1.0)
                } else {
                    damage.clamp(0.0, max)
                }
            }
            RangeProperty::Count { normalize } => {
                let count = self.stack.get::<ItemStack>().map_or(0, ItemStack::count) as f32;
                let max = max_stack_size(self.stack) as f32;
                if *normalize {
                    (count / max).clamp(0.0, 1.0)
                } else {
                    count.clamp(0.0, max)
                }
            }
            RangeProperty::CustomModelData { index } => self
                .custom_model_data()
                .and_then(|data| data.floats.get(*index as usize))
                .copied()
                .unwrap_or(0.0),
            RangeProperty::BundleFullness => bundle_weight(self.stack, self.items, &self.lookup),
            RangeProperty::Cooldown
            | RangeProperty::CrossbowPull
            | RangeProperty::UseDuration { .. }
            | RangeProperty::UseCycle { .. }
            | RangeProperty::Time { .. }
            | RangeProperty::Compass { .. } => 0.0,
        }
    }

    fn tint(&self, source: &TintSource) -> u32 {
        match source {
            TintSource::Constant { value } => opaque(value.0),
            TintSource::Dye { default } => self
                .get::<DyedColor>()
                .map_or(default.0 as u32, |DyedColor(rgb)| opaque(rgb.0)),
            // ponytail: without the custom colour a potion shows the base colour;
            // averaging effect colours needs the potion and mob_effect tables in the corpus.
            TintSource::Potion { default } => opaque(
                self.get::<PotionContents>()
                    .and_then(|contents| contents.custom_color)
                    .unwrap_or(default.0),
            ),
            TintSource::Firework { default } => {
                match self
                    .get::<FireworkExplosion>()
                    .map_or(&[][..], |e| &e.colors[..])
                {
                    [] => default.0 as u32,
                    [only] => opaque(*only),
                    colors => {
                        let channel = |shift: u32| {
                            let sum: i32 = colors.iter().map(|c| (c >> shift) & 0xFF).sum();
                            (sum / colors.len() as i32) as u32
                        };
                        0xFF00_0000 | channel(16) << 16 | channel(8) << 8 | channel(0)
                    }
                }
            }
            TintSource::Grass {
                temperature,
                downfall,
            } => sample_colormap(
                self.models.grass_colormap.as_deref(),
                *temperature,
                *downfall,
            )
            .map_or(0xFFFF_00FF, |[r, g, b, _]| {
                0xFF00_0000
                    | ((r * 255.0) as u32) << 16
                    | ((g * 255.0) as u32) << 8
                    | (b * 255.0) as u32
            }),
            TintSource::CustomModelData { index, default } => opaque(
                self.custom_model_data()
                    .and_then(|data| data.colors.get(*index as usize))
                    .map_or(default.0, |color| color.0),
            ),
            TintSource::Team { default } => opaque(default.0),
        }
    }
}

const GUI: usize = DisplayContext::Gui as usize - 1;

fn gui_matrix(model: &BakedItemModel, local: Mat4) -> Mat4 {
    let display = model.display[GUI];
    let centre = Mat4::from_translation(Vec3::splat(-0.5));
    let posed = if display == ItemTransform::NONE {
        centre
    } else {
        let radians = display.rotation_deg * (std::f32::consts::PI / 180.0);
        Mat4::from_translation(display.translation)
            * Mat4::from_quat(Quat::from_euler(
                EulerRot::XYZ,
                radians.x,
                radians.y,
                radians.z,
            ))
            * Mat4::from_scale(display.scale)
            * centre
    };
    Mat4::from_translation(Vec3::new(8.0, 8.0, 0.0))
        * Mat4::from_scale(Vec3::new(16.0, -16.0, 16.0))
        * posed
        * local
}

const LIGHT_0: Vec3 = Vec3::new(0.2, 1.0, -0.7);
const LIGHT_1: Vec3 = Vec3::new(-0.2, 1.0, 0.7);

pub fn light_directions(gui_light: GuiLight) -> [Vec3; 2] {
    use std::f32::consts::PI;
    let pose = match gui_light {
        GuiLight::Front => Mat3::from_rotation_y(-PI / 8.0) * Mat3::from_rotation_x(PI * 3.0 / 4.0),
        GuiLight::Side => {
            Mat3::from_diagonal(Vec3::new(1.0, -1.0, 1.0))
                * Mat3::from_euler(EulerRot::YXZ, 1.0821041, 3.2375858, 0.0)
                * Mat3::from_euler(EulerRot::YXZ, -PI / 8.0, PI * 3.0 / 4.0, 0.0)
        }
    };
    [pose * LIGHT_0.normalize(), pose * LIGHT_1.normalize()]
}

pub fn light_factor(normal: Vec3, lights: [Vec3; 2]) -> f32 {
    let diffuse = normal.dot(lights[0]).max(0.0) + normal.dot(lights[1]).max(0.0);
    (diffuse * 0.6 + 0.4).min(1.0)
}

fn gui_vertices(
    model: &BakedItemModel,
    local: Mat4,
    tints: &[u32],
    gui_light: GuiLight,
) -> Vec<GuiVertex> {
    let matrix = gui_matrix(model, local);
    let normals = Mat3::from_mat4(matrix).inverse().transpose();
    let lights = light_directions(gui_light);
    let mut vertices = Vec::with_capacity(model.quads.len() * 4);
    for quad in &model.quads {
        let tint = quad
            .tint
            .and_then(|index| tints.get(index as usize).copied())
            .unwrap_or(0xFFFF_FFFF);
        let normal = (normals * quad.dir.normal().as_vec3()).normalize();
        let factor = light_factor(normal, lights);
        let color = [
            (((tint >> 16) & 0xFF) as f32 * factor).round() as u8,
            (((tint >> 8) & 0xFF) as f32 * factor).round() as u8,
            ((tint & 0xFF) as f32 * factor).round() as u8,
            (tint >> 24) as u8,
        ];
        let sprite = u32::from(quad.sprite);
        let positions = quad.positions.map(|p| matrix.transform_point3(p));
        // The vertex shader flips y into clip space, which mirrors the winding: a front face
        // must be clockwise here to come out counter-clockwise for the pipeline's cull.
        let winding = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        let order: [usize; 4] = if winding.dot(normal) > 0.0 {
            [3, 2, 1, 0]
        } else {
            [0, 1, 2, 3]
        };
        vertices.extend(order.map(|i| GuiVertex {
            pos: positions[i].to_array(),
            uv: quad.uvs[i],
            color,
            sprite,
        }));
    }
    vertices
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use bevy::app::{App, TaskPoolPlugin};
    use bevy::asset::{AssetPlugin, AssetServer};
    use bevy::ecs::world::World;
    use mcrs_minecraft_block::definition::load_block_definitions;
    use mcrs_minecraft_core::codec::Bounded;
    use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
    use bevy::ecs::system::Command;
    use mcrs_minecraft_inventory::{Op, Slot, Transaction};
    use mcrs_minecraft_item::{SlotTable, load_item_definitions};
    use mcrs_minecraft_protocol::item::{
        BundleContents, ChargedProjectiles, ComponentPatch, Damage, Enchantments,
        FireworkExplosion, FireworkShape, ItemComponentKind, ItemStackValue, RgbInt,
        Template,
    };

    use super::*;
    use crate::atlas::SpriteRegistry;
    use crate::item_model::asset::ComponentKindId;
    use crate::item_model::bake::bake_all;
    use crate::model::Pack;

    fn items() -> &'static Items {
        static ITEMS: OnceLock<Items> = OnceLock::new();
        ITEMS.get_or_init(|| {
            let mut app = App::new();
            app.add_plugins(TaskPoolPlugin::default());
            app.add_plugins(AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            });
            let assets = app.world().resource::<AssetServer>().clone();
            let (blocks, _) = load_block_definitions(&assets).expect("the block corpus loads");
            Items(Arc::new(
                load_item_definitions(&assets, &blocks).expect("the item corpus loads"),
            ))
        })
    }

    fn models() -> &'static ItemModels {
        static MODELS: OnceLock<ItemModels> = OnceLock::new();
        MODELS.get_or_init(|| bake_all(Pack::corpus(), &mut SpriteRegistry::new()).unwrap())
    }

    fn value(path: &str, count: i32, components: ComponentPatch) -> ItemStackValue {
        ItemStackValue {
            item: ResourceKey::from_location(ResourceLocation::minecraft(path)),
            count: Bounded(count),
            components,
        }
    }

    fn world() -> World {
        let mut world = World::new();
        world.insert_resource(items().clone());
        world
    }

    fn spawn(world: &mut World, path: &str, count: i32, components: ComponentPatch) -> Entity {
        mcrs_minecraft_inventory::value::spawn_stack(world, &value(path, count, components), items()).unwrap()
    }

    fn set<K: mcrs_minecraft_protocol::item::ItemDataComponent>(world: &mut World, stack: Entity, value: K) {
        Transaction(vec![Op::Insert {
            stack,
            component: value.into_value(),
        }])
        .apply(world);
    }

    type Lookup<'w> = Box<dyn Fn(Entity) -> Option<EntityRef<'w>> + 'w>;

    fn eval<'w>(world: &'w World, stack: Entity) -> Evaluator<'w, 'static, Lookup<'w>> {
        Evaluator {
            stack: world.entity(stack),
            items: items(),
            models: models(),
            lookup: Box::new(move |child| world.get_entity(child).ok()),
        }
    }

    fn resolved(world: &World, stack: Entity) -> ItemRenderLayers {
        resolve(world.entity(stack), items(), models(), &|child| {
            world.get_entity(child).ok()
        })
    }

    fn patch(f: impl FnOnce(&mut ComponentPatch)) -> ComponentPatch {
        let mut patch = ComponentPatch::EMPTY;
        f(&mut patch);
        patch
    }

    fn template(path: &str, count: i32) -> Template {
        Template(value(path, count, ComponentPatch::EMPTY))
    }

    fn near(a: Vec3, b: [f32; 3]) -> bool {
        a.abs_diff_eq(Vec3::from(b), 1e-4)
    }

    #[test]
    fn damage_and_count_ranges_follow_the_stack() {
        let mut world = world();
        let pick = spawn(
            &mut world,
            "diamond_pickaxe",
            1,
            patch(|p| p.set(Damage(Bounded(100)))),
        );
        let e = eval(&world, pick);
        assert!((e.range(&RangeProperty::Damage { normalize: true }) - 0.0640615).abs() < 1e-6);
        assert_eq!(e.range(&RangeProperty::Damage { normalize: false }), 100.0);
        assert!(e.condition(&ConditionProperty::Damaged));
        assert!(!e.condition(&ConditionProperty::Broken));
        drop(e);
        set(&mut world, pick, Damage(Bounded(1560)));
        let e = eval(&world, pick);
        assert!(e.condition(&ConditionProperty::Broken));
        drop(e);
        let stone = spawn(&mut world, "stone", 17, ComponentPatch::EMPTY);
        let e = eval(&world, stone);
        assert_eq!(e.range(&RangeProperty::Count { normalize: true }), 0.265625);
        assert!(!e.condition(&ConditionProperty::Damaged));
    }

    #[test]
    fn bundle_fullness_weighs_children_and_nested_bundles() {
        let mut world = world();
        let inner = value(
            "bundle",
            1,
            patch(|p| p.set(BundleContents(vec![template("arrow", 16)]))),
        );
        let bundle = spawn(
            &mut world,
            "bundle",
            1,
            patch(|p| {
                p.set(BundleContents(vec![
                    template("stone", 3),
                    Template(inner),
                    template("beehive", 1),
                ]))
            }),
        );
        let e = eval(&world, bundle);
        assert_eq!(e.range(&RangeProperty::BundleFullness), 0.375);
        drop(e);
        let empty = spawn(&mut world, "bundle", 1, ComponentPatch::EMPTY);
        let e = eval(&world, empty);
        assert_eq!(e.range(&RangeProperty::BundleFullness), 0.0);
    }

    #[test]
    fn charge_type_reads_the_loaded_projectiles() {
        let mut world = world();
        let charged = |projectile: Option<&str>| {
            patch(|p| {
                p.set(
                    ChargedProjectiles::new(
                        projectile.map(|p| template(p, 1)).into_iter().collect(),
                    )
                    .unwrap(),
                )
            })
        };
        let empty = spawn(&mut world, "crossbow", 1, charged(None));
        let arrow = spawn(&mut world, "crossbow", 1, charged(Some("arrow")));
        let rocket = spawn(&mut world, "crossbow", 1, charged(Some("firework_rocket")));
        let e = eval(&world, empty);
        assert_eq!(e.charge_type(), ChargeType::None);
        let e = eval(&world, arrow);
        assert_eq!(e.charge_type(), ChargeType::Arrow);
        let e = eval(&world, rocket);
        assert_eq!(e.charge_type(), ChargeType::Rocket);
        let BakedNode::Select { switch, .. } = &models().get("minecraft:crossbow").root else {
            panic!("the crossbow selects on charge type");
        };
        let e = eval(&world, rocket);
        assert!(e.select(switch).is_some());
    }

    #[test]
    fn has_component_distinguishes_prototype_and_patch() {
        let mut world = world();
        let stone = spawn(&mut world, "stone", 1, ComponentPatch::EMPTY);
        let max_stack = |ignore_default| ConditionProperty::HasComponent {
            component: ComponentKindId(ItemComponentKind::MaxStackSize),
            ignore_default,
        };
        let dyed = |ignore_default| ConditionProperty::HasComponent {
            component: ComponentKindId(ItemComponentKind::DyedColor),
            ignore_default,
        };
        let e = eval(&world, stone);
        assert!(e.condition(&max_stack(false)));
        assert!(!e.condition(&max_stack(true)));
        assert!(!e.condition(&dyed(false)));
        drop(e);
        set(&mut world, stone, DyedColor(RgbInt(0xFF0000)));
        Transaction(vec![Op::Remove {
            stack: stone,
            kind: ItemComponentKind::MaxStackSize,
        }])
        .apply(&mut world);
        let e = eval(&world, stone);
        assert!(e.condition(&dyed(false)));
        assert!(e.condition(&dyed(true)));
        assert!(!e.condition(&max_stack(false)));
        assert!(e.condition(&max_stack(true)));
    }

    #[test]
    fn tints_follow_vanilla_colour_rules() {
        let mut world = world();
        let star = spawn(
            &mut world,
            "firework_star",
            1,
            patch(|p| {
                p.set(FireworkExplosion {
                    shape: FireworkShape::SmallBall,
                    colors: vec![0x112233, 0x445566, 0x778899],
                    fade_colors: Vec::new(),
                    has_trail: false,
                    has_twinkle: false,
                })
            }),
        );
        let plain = spawn(&mut world, "firework_star", 1, ComponentPatch::EMPTY);
        let firework = TintSource::Firework {
            default: RgbInt(-7697782),
        };
        let potion = spawn(&mut world, "potion", 1, ComponentPatch::EMPTY);
        let dyed = spawn(
            &mut world,
            "leather_chestplate",
            1,
            patch(|p| p.set(DyedColor(RgbInt(0x123456)))),
        );
        assert_eq!(eval(&world, star).tint(&firework), 0xFF44_5566);
        assert_eq!(eval(&world, plain).tint(&firework), (-7697782i32) as u32);
        assert_eq!(
            eval(&world, dyed).tint(&TintSource::Dye { default: RgbInt(0) }),
            0xFF12_3456
        );
        let e = eval(&world, potion);
        assert_eq!(
            e.tint(&TintSource::Potion {
                default: RgbInt(-13083194)
            }),
            (-13083194i32) as u32
        );
        assert_eq!(
            e.tint(&TintSource::Constant {
                value: RgbInt(0xFF0000)
            }),
            0xFFFF_0000
        );
        assert_eq!(
            e.tint(&TintSource::Dye {
                default: RgbInt(0x123456)
            }),
            0x0012_3456
        );
        let grass = e.tint(&TintSource::Grass {
            temperature: 0.5,
            downfall: 1.0,
        });
        assert_eq!(grass, 0xFF7C_BD6B, "{grass:#x}");
    }

    #[test]
    fn light_directions_match_the_vanilla_poses() {
        let [flat0, flat1] = light_directions(GuiLight::Front);
        assert!(near(flat0, [-0.222519, -0.1714986, 0.9597257]), "{flat0}");
        assert!(near(flat1, [-0.2150121, -0.9718252, 0.0965678]), "{flat1}");
        let [side0, side1] = light_directions(GuiLight::Side);
        assert!(near(side0, [-0.9334392, -0.2626947, -0.2443002]), "{side0}");
        assert!(near(side1, [-0.1035714, -0.9766068, 0.1884464]), "{side1}");
    }

    fn quads(layer: &RenderLayer) -> impl Iterator<Item = (&[GuiVertex], Vec3)> {
        layer.vertices.chunks(4).map(|quad| {
            let p = |i: usize| Vec3::from(quad[i].pos);
            (quad, (p(2) - p(0)).cross(p(1) - p(0)).normalize())
        })
    }

    #[test]
    fn a_block_item_shows_its_top_north_and_east_faces_lit_from_the_side() {
        let mut world = world();
        let stone = spawn(&mut world, "stone", 1, ComponentPatch::EMPTY);
        let layers = resolved(&world, stone);
        assert_eq!(layers.gui_light, GuiLight::Side);
        assert_eq!(layers.layers.len(), 1);
        assert!(!layers.oversized_in_gui && !layers.animated);
        let layer = &layers.layers[0];
        assert_eq!(layer.vertices.len(), 24);
        assert_eq!(layer.foil, Foil::None);
        let facing: Vec<Vec3> = quads(layer)
            .filter(|(_, n)| n.z > 0.0)
            .map(|(_, n)| n)
            .collect();
        assert_eq!(facing.len(), 3, "{facing:?}");
        for (normal, brightness) in [
            ([0.0, -0.8660254, 0.5], 255),
            ([0.7071066, 0.3535534, 0.6123725], 102),
            ([-0.7071069, 0.3535533, 0.6123724], 166),
        ] {
            let (quad, _) = quads(layer)
                .find(|(_, n)| near(*n, normal))
                .unwrap_or_else(|| panic!("no face with normal {normal:?}"));
            assert!(quad.iter().all(
                |v| v.color == [brightness; 3].into_iter().chain([255]).collect::<Vec<_>>()[..]
            ));
        }
        let all: Vec<Vec3> = layer.vertices.iter().map(|v| Vec3::from(v.pos)).collect();
        assert!(
            all.iter().any(|p| near(*p, [15.071068, 3.6698732, 2.5])),
            "{all:?}"
        );
        assert!(all.iter().any(|p| near(*p, [0.9289322, 3.6698723, 2.5])));
    }

    #[test]
    fn a_flat_item_is_drawn_at_sprite_colour_and_glints_when_enchanted() {
        let mut world = world();
        let stick = spawn(&mut world, "stick", 1, ComponentPatch::EMPTY);
        let layers = resolved(&world, stick);
        assert_eq!(layers.gui_light, GuiLight::Front);
        let layer = &layers.layers[0];
        let (front, _) = quads(layer)
            .find(|(_, n)| near(*n, [0.0, 0.0, 1.0]))
            .expect("a front face");
        assert!(front.iter().all(|v| v.color == [255; 4]));
        assert!(
            front
                .iter()
                .all(|v| (0.0..=16.0).contains(&v.pos[0]) && (0.0..=16.0).contains(&v.pos[1]))
        );
        assert!(quads(layer).all(|(quad, _)| quad.iter().all(|v| v.sprite == front[0].sprite)));

        let sharp = spawn(
            &mut world,
            "stick",
            1,
            patch(|p| {
                p.set(Enchantments(vec![(
                    ResourceKey::from_location(ResourceLocation::minecraft("sharpness")),
                    1,
                )]))
            }),
        );
        assert!(
            resolved(&world, sharp)
                .layers
                .iter()
                .all(|layer| layer.foil == Foil::Standard)
        );
    }

    #[test]
    fn the_system_resolves_held_stacks_once_per_revision() {
        let mut app = App::new();
        app.insert_resource(items().clone());
        app.insert_resource(ItemModels {
            by_id: models().by_id.clone(),
            missing: models().missing.clone(),
            grass_colormap: None,
        });
        app.add_systems(Update, resolve_item_layers);
        let world = app.world_mut();
        let holder = world.spawn(SlotTable::fixed(9)).id();
        Transaction(vec![Op::Spawn {
            value: value("stick", 1, ComponentPatch::EMPTY),
            to: Slot::new(holder, 0),
        }])
        .apply(world);
        let held = world.get::<SlotTable>(holder).unwrap().get(0).unwrap();
        let loose = spawn(world, "stick", 1, ComponentPatch::EMPTY);
        app.update();
        assert_eq!(
            app.world().get::<ItemRenderLayers>(held).unwrap().gui_light,
            GuiLight::Front
        );
        assert!(app.world().get::<ItemRenderLayers>(loose).is_none());

        let world = app.world_mut();
        let before = world
            .entity(held)
            .get_ref::<ItemRenderLayers>()
            .unwrap()
            .last_changed();
        app.update();
        assert_eq!(
            app.world()
                .entity(held)
                .get_ref::<ItemRenderLayers>()
                .unwrap()
                .last_changed(),
            before
        );

        let world = app.world_mut();
        set(world, held, ItemModel(ResourceLocation::minecraft("stone")));
        app.update();
        let after = app
            .world()
            .entity(held)
            .get_ref::<ItemRenderLayers>()
            .unwrap();
        assert_eq!(after.gui_light, GuiLight::Side);
        assert_ne!(after.last_changed(), before);
    }
}
