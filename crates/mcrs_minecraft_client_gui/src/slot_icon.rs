use bevy::app::{App, Plugin, Update};
use bevy::prelude::{
    Commands, Component, DetectChanges, DetectChangesMut, Entity, IntoScheduleConfigs, Local,
    MessageReader, Query, Res, SystemCondition, on_message, resource_changed, resource_exists,
    resource_exists_and_changed,
};
use mcrs_minecraft_engine_item::SlotChanged;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_client::InventoryClientSet;
use mcrs_minecraft_item_icon::{IconSources, ItemIcon, StackOverlay, resolve};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_bake::ItemModels;
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_registries::CurrentRegistries;

const GUI: ItemModelContext = ItemModelContext::new(DisplayContext::Gui);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotIcon {
    pub icon: ItemIcon,
    pub overlay: StackOverlay,
}

impl SlotIcon {
    #[must_use]
    pub fn of(view: StackView<'_>, sources: &IconSources<'_>) -> Self {
        Self {
            icon: resolve(view, sources, &GUI),
            overlay: StackOverlay::of(view),
        }
    }
}

pub struct SlotIconPlugin;

impl Plugin for SlotIconPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            project_slot_icons
                .after(ClientNetworkSystems::Receive)
                .after(InventoryClientSet::Predict)
                .run_if(
                    resource_exists::<ItemTable>
                        .and_then(resource_exists::<ItemModels>)
                        .and_then(
                            on_message::<SlotChanged>
                                .or_else(resource_changed::<ItemModels>)
                                .or_else(resource_changed::<ItemTable>)
                                .or_else(resource_exists_and_changed::<CurrentRegistries>),
                        ),
                ),
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
fn project_slot_icons(
    models: Res<ItemModels>,
    table: Res<ItemTable>,
    registries: Option<Res<CurrentRegistries>>,
    mut changes: MessageReader<SlotChanged>,
    holders: Query<&Slots>,
    mut icons: Query<&mut SlotIcon>,
    mut stacks: Stacks,
    mut dirty: Local<Vec<Entity>>,
    mut commands: Commands,
) {
    dirty.clear();
    let registries_changed = registries.as_ref().is_some_and(DetectChanges::is_changed);
    if models.is_changed() || table.is_changed() || registries_changed {
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
    let sources = IconSources {
        models: &models,
        registries: registries.as_deref().map(|registries| &**registries),
    };
    for &stack in dirty.iter() {
        let Some(value) = stacks.to_value(stack) else {
            continue;
        };
        let view = StackView {
            stack: &value,
            table: &table,
        };
        let projected = SlotIcon::of(view, &sources);
        match icons.get_mut(stack) {
            Ok(mut slot) => {
                slot.set_if_neq(projected);
            }
            Err(_) => {
                commands.entity(stack).try_insert(projected);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bevy::app::App;
    use bevy::prelude::DetectChanges;
    use bevy::prelude::World;
    use mcrs_minecraft_client_world::screen::Screen;
    use mcrs_minecraft_engine_inventory_menu::{MenuButtons, MenuCommandsExt, MenuParts};
    use mcrs_minecraft_engine_item::HolderCommandsExt;
    use mcrs_minecraft_engine_item::ItemCommandsExt;
    use mcrs_minecraft_engine_item::ItemPlugin;
    use mcrs_minecraft_engine_item::StackCommandsExt;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_minecraft_inventory_file::PlayerItems;
    use mcrs_minecraft_inventory_menu::MenuLayout;
    use mcrs_minecraft_item_stack::Count;
    use mcrs_minecraft_item_stack::ItemModel;
    use mcrs_minecraft_item_stack::ItemStack;
    use mcrs_minecraft_item_stack::Kind;
    use mcrs_minecraft_item_stack::NameLookup;
    use mcrs_minecraft_item_stack::NameTable;
    use mcrs_minecraft_item_stack::RegistryValue;
    use mcrs_minecraft_item_stack::Trim;
    use mcrs_minecraft_item_stack::TrimMaterialId;
    use mcrs_minecraft_item_stack::TrimPatternId;
    use mcrs_minecraft_menu_key::menu_keys::MenuType;
    use mcrs_minecraft_model_file::model_file::GuiLight;
    use mcrs_minecraft_player_holders::{PlayerHolders, PlayerHoldersPlugin, restore_holders};
    use mcrs_minecraft_resource::ResourceLocation;

    use super::*;
    use crate::menu_test_app::{self, ATLAS, MODELS, TABLE};
    use crate::scene::{GuiAtlas, GuiScene, GuiSize, draw_screens};

    fn stack(path: &str, count: u8) -> ItemStack {
        let name = format!("minecraft:{path}");
        let location = ResourceLocation::read(&name).unwrap();
        let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
        ItemStack::new(kind, Count::new(count).unwrap())
    }

    fn icon_app() -> App {
        let mut app = App::new();
        app.insert_resource(*TABLE)
            .insert_resource({
                let mut models = MODELS.clone();
                models.grass_colormap = None;
                models
            })
            .add_plugins((ItemPlugin::default(), SlotIconPlugin));
        app
    }

    fn load(app: &mut App, stacks: Vec<Option<ItemStack>>) -> Entity {
        let world = app.world_mut();
        let holder = world.spawn_empty().id();
        world.commands().entity(holder).load_inventory(stacks);
        world.flush();
        app.update();
        holder
    }

    fn occupant(app: &App, holder: Entity, index: usize) -> Entity {
        app.world()
            .get::<Slots>(holder)
            .unwrap()
            .get(index)
            .unwrap()
    }

    mod jar {
        use super::*;

        #[test]
        fn the_system_rebuilds_only_the_stacks_of_changed_slots() {
            let mut app = icon_app();
            let holder = load(&mut app, vec![Some(stack("stick", 1)); 3]);
            let (first, second) = (occupant(&app, holder, 0), occupant(&app, holder, 1));
            let changed = |app: &App, stack: Entity| {
                let slot = app.world().entity(stack).get_ref::<SlotIcon>().unwrap();
                (slot.icon.gui_light(), slot.last_changed())
            };
            let (light, built) = changed(&app, second);
            assert_eq!(light, GuiLight::Front);
            let (_, first_built) = changed(&app, first);
            let third = occupant(&app, holder, 2);
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
            let replaced = occupant(&app, holder, 0);
            assert_ne!(replaced, first);
            assert_eq!(changed(&app, replaced).0, GuiLight::Side);
            let (light, rebuilt) = changed(&app, second);
            assert_eq!(light, GuiLight::Front);
            assert_ne!(
                rebuilt, built,
                "a count change of a slot rebuilds its stack"
            );
            assert_eq!(
                app.world().get::<SlotIcon>(second).unwrap().overlay.count,
                2
            );
            assert_eq!(
                changed(&app, third),
                untouched,
                "an unchanged slot keeps its icon"
            );
        }

        #[test]
        fn new_registries_rebuild_the_icon_of_a_trimmed_armour_piece() {
            let mut app = icon_app();
            let trim = Trim {
                material: RegistryValue::Reference(TrimMaterialId(0)),
                pattern: RegistryValue::Reference(TrimPatternId(0)),
            };
            let chestplate = stack("iron_chestplate", 1).set(trim);
            let holder = load(&mut app, vec![Some(chestplate.clone())]);
            let projected = occupant(&app, holder, 0);
            let icon = |app: &App| app.world().get::<SlotIcon>(projected).unwrap().icon;
            let expected = |registries| {
                let view = StackView {
                    stack: &chestplate,
                    table: &TABLE,
                };
                let sources = IconSources {
                    models: &MODELS,
                    registries,
                };
                resolve(view, &sources, &GUI)
            };
            let (plain, trimmed) = (expected(None), expected(Some(test_registries())));
            assert_ne!(plain, trimmed, "the trim material picks its own model");
            assert_eq!(icon(&app), plain);

            app.insert_resource(CurrentRegistries::new(test_registries().clone()));
            app.update();
            assert_eq!(icon(&app), trimmed);
        }

        #[test]
        fn a_steady_frame_draws_the_slot_icons_of_full_holders_without_allocation() {
            let mut app = icon_app();
            app.insert_resource(GuiAtlas(Arc::clone(&ATLAS)))
                .insert_resource(GuiScene {
                    size: GuiSize {
                        width: menu_test_app::GUI.width,
                        height: menu_test_app::GUI.height,
                        scale: 1,
                    },
                    ..GuiScene::default()
                })
                .insert_resource(Screen::Menu)
                .add_plugins(PlayerHoldersPlugin);
            let world = app.world_mut();
            let player = world.spawn_empty().id();
            world
                .run_system_cached_with(restore_holders, (player, PlayerItems::default()))
                .unwrap();
            world.flush();
            let chest = world.spawn(Slots::new(27)).id();
            let cursor = world.get::<PlayerHolders>(player).unwrap().cursor;
            let mut commands = world.commands();
            commands.entity(player).open_menu(
                MenuLayout::of(MenuType::Generic9x3).unwrap(),
                MenuParts::new([chest, player]).unwrap(),
                MenuButtons::default(),
            );
            let places = (0..27).map(|index| (chest, index));
            let places = places.chain((0..36).map(|index| (player, index)));
            for (holder, index) in places.chain([(cursor, 0)]) {
                commands
                    .spawn_stack(stack("stone", 5))
                    .put_at(holder, index);
            }
            world.flush();
            app.update();

            let world = app.world_mut();
            let draw = world.register_system(draw_screens);
            let frame = |world: &mut World| {
                let mut scene = world.resource_mut::<GuiScene>();
                scene.quads.clear();
                scene.icons.clear();
                world.run_system(draw).unwrap();
            };
            frame(world);
            frame(world);
            let before = mcrs_test_alloc::allocations();
            frame(world);
            let allocations = mcrs_test_alloc::allocations() - before;
            let scene = world.resource::<GuiScene>();
            assert_eq!(
                scene.icons.len(),
                9 + 27 + 36 + 1,
                "the hotbar, the chest, the player inventory and the carried stack"
            );
            assert!(
                scene.icons.iter().all(|icon| icon.overlay.count == 5),
                "each icon carries the count of its stack"
            );
            assert_eq!(allocations, 0, "no slot changed since the last frame");
        }
    }
}
