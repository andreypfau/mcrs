use crate::durability::Durability;
use bevy::app::{App, Plugin, Update};
use bevy::math::{EulerRot, Mat3, Mat4, Quat, Vec3};
use bevy::prelude::{
    Commands, Component, DetectChanges, Entity, IntoScheduleConfigs, Local, MessageReader, Query,
    Res, SystemCondition, on_message, resource_changed, resource_exists,
};
use bytemuck::{Pod, Zeroable};
use mcrs_minecraft_engine_item::SlotChanged;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_client::InventoryClientSet;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};

use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_bake::{BakedItemModel, BakedNode, ItemModels};
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::item_model_eval::Evaluator;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_model_file::model_file::{DisplayTransform, GuiLight};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GuiVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub sprite: u32,
}

#[derive(Debug)]
pub struct RenderLayer {
    pub vertices: Vec<GuiVertex>,
    pub foil: bool,
}

#[derive(Component, Debug)]
pub struct ItemRenderLayers {
    pub layers: Vec<RenderLayer>,
    pub gui_light: GuiLight,
    pub damage: Option<Durability>,
}

pub struct ItemRenderPlugin;

impl Plugin for ItemRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            resolve_item_layers
                .after(ClientNetworkSystems::Receive)
                .after(InventoryClientSet::Predict)
                .run_if(
                    resource_exists::<ItemTable>
                        .and_then(resource_exists::<ItemModels>)
                        .and_then(
                            on_message::<SlotChanged>
                                .or_else(resource_changed::<ItemModels>)
                                .or_else(resource_changed::<ItemTable>),
                        ),
                ),
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
pub fn resolve_item_layers(
    models: Res<ItemModels>,
    table: Res<ItemTable>,
    registries: Option<Res<CurrentRegistries>>,
    mut changes: MessageReader<SlotChanged>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    mut dirty: Local<Vec<Entity>>,
    mut commands: Commands,
) {
    dirty.clear();
    if models.is_changed() || table.is_changed() {
        changes.clear();
        for slots in &holders {
            dirty.extend(slots.occupied());
        }
    } else {
        for change in changes.read() {
            let slot = change.slot;
            let occupant = holders
                .get(slot.holder)
                .ok()
                .and_then(|slots| slots.get(usize::from(slot.index)));
            dirty.extend(occupant);
        }
    }
    dirty.sort_unstable();
    dirty.dedup();
    let registries = registries.as_deref().map(|registries| &**registries);
    for &stack in dirty.iter() {
        let Some(value) = stacks.to_value(stack) else {
            continue;
        };
        commands
            .entity(stack)
            .try_insert(resolve(&value, &table, &models, registries));
    }
}

pub fn resolve(
    stack: &ItemStack,
    table: &ItemTable,
    models: &ItemModels,
    registries: Option<&Registries>,
) -> ItemRenderLayers {
    let item = table
        .effective::<ItemModel>(stack)
        .and_then(|model| models.by_id.get(&model.get()))
        .unwrap_or(&models.missing);
    let stack = StackView { stack, table };
    let evaluator = Evaluator {
        stack,
        registries,
        grass_colormap: models.grass_colormap.as_deref(),
        context: ItemModelContext::new(DisplayContext::Gui),
    };
    let mut layers = Vec::new();
    collect(&evaluator, models, item, &mut layers);
    let gui_light = layers
        .first()
        .map_or(GuiLight::Front, |layer| layer.model.gui_light);
    let foil = stack.has_foil();
    ItemRenderLayers {
        layers: layers
            .into_iter()
            .map(|layer| RenderLayer {
                vertices: gui_vertices(layer.model, &layer.tints, gui_light),
                foil,
            })
            .collect(),
        gui_light,
        damage: stack.is_damaged().then(|| Durability {
            damage: stack.damage_value(),
            max_damage: stack.max_damage(),
        }),
    }
}

struct Layer<'m> {
    model: &'m BakedItemModel,
    tints: Vec<u32>,
}

fn collect<'m>(
    evaluator: &Evaluator<'_>,
    models: &'m ItemModels,
    node: &'m BakedNode,
    out: &mut Vec<Layer<'m>>,
) {
    match node {
        BakedNode::Empty | BakedNode::BundleSelectedItem => {}
        BakedNode::Model { model, tints } => out.extend(models.model(*model).map(|model| Layer {
            model,
            tints: tints.iter().map(|tint| evaluator.tint(tint)).collect(),
        })),
        BakedNode::Special { properties, .. } => {
            out.extend(models.model(*properties).map(|model| Layer {
                model,
                tints: Vec::new(),
            }))
        }
        BakedNode::Composite(children) => {
            for child in children {
                collect(evaluator, models, child, out);
            }
        }
        BakedNode::Condition {
            property,
            on_true,
            on_false,
        } => collect(
            evaluator,
            models,
            if evaluator.condition(property) {
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
        } => collect(
            evaluator,
            models,
            evaluator
                .select(switch)
                .and_then(|i| cases.get(i))
                .unwrap_or(&**fallback),
            out,
        ),
        BakedNode::RangeDispatch {
            property,
            scale,
            thresholds,
            models: entries,
            fallback,
        } => {
            let value = evaluator.range(property) * scale;
            let chosen = BakedNode::range_index(thresholds, value)
                .and_then(|i| entries.get(i))
                .unwrap_or(&**fallback);
            collect(evaluator, models, chosen, out);
        }
    }
}

fn gui_matrix(model: &BakedItemModel) -> Mat4 {
    let display = model.display;
    let centre = Mat4::from_translation(Vec3::splat(-0.5));
    let posed = if display == DisplayTransform::IDENTITY {
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

fn gui_vertices(model: &BakedItemModel, tints: &[u32], gui_light: GuiLight) -> Vec<GuiVertex> {
    let matrix = gui_matrix(model);
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
        let winding = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        let order: [usize; 4] = if winding.dot(normal) > 0.0 {
            [3, 2, 1, 0]
        } else {
            [0, 1, 2, 3]
        };
        vertices.extend(order.iter().filter_map(|&i| {
            Some(GuiVertex {
                pos: positions.get(i)?.to_array(),
                uv: *quad.uvs.get(i)?,
                color,
                sprite,
            })
        }));
    }
    vertices
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU8;
    use std::sync::{LazyLock, OnceLock};

    use bevy::app::App;
    use bevy::prelude::DetectChanges;
    use mcrs_minecraft_engine_item::HolderCommandsExt;
    use mcrs_minecraft_engine_item::ItemCommandsExt;
    use mcrs_minecraft_engine_item::ItemPlugin;
    use mcrs_minecraft_engine_item::StackCommandsExt;
    use mcrs_minecraft_engine_registries::test_item_table;
    use mcrs_minecraft_item::RgbInt;
    use mcrs_minecraft_item::data_component_type_keys::DataComponentType;
    use mcrs_minecraft_item_stack::ArgbColor;
    use mcrs_minecraft_item_stack::Count;
    use mcrs_minecraft_item_stack::Damage;
    use mcrs_minecraft_item_stack::DyedColor;
    use mcrs_minecraft_item_stack::EnchantmentId;
    use mcrs_minecraft_item_stack::EnchantmentLevel;
    use mcrs_minecraft_item_stack::Enchantments;
    use mcrs_minecraft_item_stack::FireworkExplosion;
    use mcrs_minecraft_item_stack::FireworkShape;
    use mcrs_minecraft_item_stack::Kind;
    use mcrs_minecraft_item_stack::MaxStackSize;
    use mcrs_minecraft_item_stack::NameLookup;
    use mcrs_minecraft_item_stack::NameTable;
    use mcrs_minecraft_resource::ResourceLocation;

    use super::*;
    use mcrs_minecraft_model::atlas::SpriteRegistry;
    use mcrs_minecraft_model::item_model_asset::{ConditionProperty, RangeProperty, TintSource};
    use mcrs_minecraft_model::item_model_bake::bake_all;
    use mcrs_minecraft_model::resource_set::ResourceSet;

    static TABLE: LazyLock<ItemTable> = LazyLock::new(test_item_table);

    fn models() -> &'static ItemModels {
        static MODELS: OnceLock<ItemModels> = OnceLock::new();
        MODELS.get_or_init(|| {
            bake_all(ResourceSet::corpus(), &mut SpriteRegistry::default()).unwrap()
        })
    }

    fn stack(path: &str, count: u8) -> ItemStack {
        let name = format!("minecraft:{path}");
        let location = ResourceLocation::read(&name).unwrap();
        let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
        ItemStack::new(kind, Count::new(count).unwrap())
    }

    fn eval(stack: &ItemStack) -> Evaluator<'_> {
        Evaluator {
            stack: StackView {
                stack,
                table: &TABLE,
            },
            registries: None,
            grass_colormap: models().grass_colormap.as_deref(),
            context: ItemModelContext::new(DisplayContext::Gui),
        }
    }

    fn resolved(stack: &ItemStack) -> ItemRenderLayers {
        resolve(stack, &TABLE, models(), None)
    }

    fn near(a: Vec3, b: [f32; 3]) -> bool {
        a.abs_diff_eq(Vec3::from(b), 1e-4)
    }

    mod jar {
        use super::*;

        #[test]
        fn item_stacks_evaluate_and_resolve_against_the_corpus() {
            damage_and_count_ranges_follow_the_stack();
            has_component_distinguishes_prototype_and_patch();
            tints_follow_vanilla_colour_rules();
            a_block_item_shows_its_top_north_and_east_faces_lit_from_the_side();
            a_flat_item_is_drawn_at_sprite_colour_and_glints_when_enchanted();
            the_system_rebuilds_only_the_stacks_of_changed_slots();
        }
    }

    fn damage_and_count_ranges_follow_the_stack() {
        let pick = stack("diamond_pickaxe", 1).set(Damage(100));
        let e = eval(&pick);
        assert!((e.range(&RangeProperty::Damage { normalize: true }) - 0.0640615).abs() < 1e-6);
        assert_eq!(e.range(&RangeProperty::Damage { normalize: false }), 100.0);
        assert!(e.condition(&ConditionProperty::Damaged));
        assert!(!e.condition(&ConditionProperty::Broken));
        assert_eq!(
            resolved(&pick).damage,
            Some(Durability {
                damage: 100,
                max_damage: 1561
            })
        );
        let pick = stack("diamond_pickaxe", 1).set(Damage(1560));
        assert!(eval(&pick).condition(&ConditionProperty::Broken));
        let stone = stack("stone", 17);
        let e = eval(&stone);
        assert_eq!(e.range(&RangeProperty::Count { normalize: true }), 0.265625);
        assert!(!e.condition(&ConditionProperty::Damaged));
        assert_eq!(resolved(&stone).damage, None);
    }

    fn has_component_distinguishes_prototype_and_patch() {
        let max_stack = |ignore_default| ConditionProperty::HasComponent {
            component: DataComponentType::MaxStackSize,
            ignore_default,
        };
        let dyed = |ignore_default| ConditionProperty::HasComponent {
            component: DataComponentType::DyedColor,
            ignore_default,
        };
        let stone = stack("stone", 1);
        let e = eval(&stone);
        assert!(e.condition(&max_stack(false)));
        assert!(!e.condition(&max_stack(true)));
        assert!(!e.condition(&dyed(false)));
        let stone = stack("stone", 1)
            .set(DyedColor(ArgbColor(0xFF0000)))
            .unset::<MaxStackSize>();
        let e = eval(&stone);
        assert!(e.condition(&dyed(false)));
        assert!(e.condition(&dyed(true)));
        assert!(!e.condition(&max_stack(false)));
    }

    fn tints_follow_vanilla_colour_rules() {
        let star = stack("firework_star", 1).set(FireworkExplosion {
            shape: FireworkShape::SmallBall,
            colors: vec![0x112233, 0x445566, 0x778899],
            fade_colors: Vec::new(),
            has_trail: false,
            has_twinkle: false,
        });
        let plain = stack("firework_star", 1);
        let firework = TintSource::Firework {
            default: RgbInt(-7697782),
        };
        let potion = stack("potion", 1);
        let dyed = stack("leather_chestplate", 1).set(DyedColor(ArgbColor(0x123456)));
        assert_eq!(eval(&star).tint(&firework), 0xFF44_5566);
        assert_eq!(eval(&plain).tint(&firework), (-7697782i32) as u32);
        assert_eq!(
            eval(&dyed).tint(&TintSource::Dye { default: RgbInt(0) }),
            0xFF12_3456
        );
        let e = eval(&potion);
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

    fn a_block_item_shows_its_top_north_and_east_faces_lit_from_the_side() {
        let layers = resolved(&stack("stone", 1));
        assert_eq!(layers.gui_light, GuiLight::Side);
        assert_eq!(layers.layers.len(), 1);
        let layer = &layers.layers[0];
        assert_eq!(layer.vertices.len(), 24);
        assert!(!layer.foil);
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

    fn a_flat_item_is_drawn_at_sprite_colour_and_glints_when_enchanted() {
        let layers = resolved(&stack("stick", 1));
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

        let level = EnchantmentLevel {
            enchantment: EnchantmentId(0),
            level: NonZeroU8::MIN,
        };
        let sharp = stack("stick", 1).set(Enchantments::new([level]).unwrap());
        assert!(resolved(&sharp).layers.iter().all(|layer| layer.foil));
    }

    fn the_system_rebuilds_only_the_stacks_of_changed_slots() {
        let mut app = App::new();
        app.insert_resource(*TABLE)
            .insert_resource({
                let mut models = models().clone();
                models.grass_colormap = None;
                models
            })
            .add_plugins((ItemPlugin::default(), ItemRenderPlugin));
        let world = app.world_mut();
        let holder = world.spawn_empty().id();
        world
            .commands()
            .entity(holder)
            .load_inventory(vec![Some(stack("stick", 1)); 3]);
        world.flush();
        app.update();
        let occupant = |app: &App, index| app.world().get::<Slots>(holder).unwrap().get(index);
        let (first, second) = (occupant(&app, 0).unwrap(), occupant(&app, 1).unwrap());
        let changed = |app: &App, stack: Entity| {
            let layers = app
                .world()
                .entity(stack)
                .get_ref::<ItemRenderLayers>()
                .unwrap();
            (layers.gui_light, layers.last_changed())
        };
        let (light, built) = changed(&app, second);
        assert_eq!(light, GuiLight::Front);
        let (_, first_built) = changed(&app, first);
        let third = occupant(&app, 2).unwrap();
        let untouched = changed(&app, third);

        app.update();
        assert_eq!(changed(&app, first), (GuiLight::Front, first_built));
        assert_eq!(changed(&app, second), (GuiLight::Front, built));

        let world = app.world_mut();
        let model = ItemModel::from_static(ResourceLocation::read("minecraft:stone").unwrap());
        world.commands().entity(first).despawn();
        world
            .commands()
            .spawn_stack(stack("stick", 1).set(model))
            .put_at(holder, 0);
        world
            .commands()
            .entity(second)
            .insert(Count::new(2).unwrap())
            .touch();
        world.flush();
        app.update();
        let replaced = occupant(&app, 0).unwrap();
        assert_ne!(replaced, first);
        assert_eq!(changed(&app, replaced).0, GuiLight::Side);
        let (light, rebuilt) = changed(&app, second);
        assert_eq!(light, GuiLight::Front);
        assert_ne!(
            rebuilt, built,
            "a count change of a slot rebuilds its stack"
        );
        assert_eq!(
            changed(&app, third),
            untouched,
            "an unchanged slot keeps its layers"
        );
    }
}
