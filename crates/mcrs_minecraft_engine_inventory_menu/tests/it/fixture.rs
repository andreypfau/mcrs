use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuClosed;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuOpened;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuPlugin;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_inventory_menu::MenuLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logged {
    Opened(Entity),
    Closed(Entity),
}

#[derive(Resource, Default)]
pub struct Log(pub Vec<Logged>);

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MenuPlugin)
        .init_resource::<Log>()
        .add_observer(|opened: On<MenuOpened>, mut log: ResMut<Log>| {
            log.0.push(Logged::Opened(opened.menu));
        })
        .add_observer(|closed: On<MenuClosed>, mut log: ResMut<Log>| {
            log.0.push(Logged::Closed(closed.menu));
        });
    app
}

pub fn open(app: &mut App, viewer: Entity, holders: &[Entity]) -> Entity {
    let parts = MenuParts::new(holders.iter().copied()).unwrap();
    let world = app.world_mut();
    world
        .commands()
        .entity(viewer)
        .open_menu(MenuLayout::INVENTORY, parts, MenuButtons::default());
    world.flush();
    world.get::<Menus>(viewer).unwrap().open().unwrap()
}

pub fn close(app: &mut App, viewer: Entity) {
    let world = app.world_mut();
    world.commands().entity(viewer).close_menu();
    world.flush();
}

pub fn menus(app: &App, viewer: Entity) -> Vec<Entity> {
    app.world()
        .get::<Menus>(viewer)
        .map(|menus| menus.iter().collect())
        .unwrap_or_default()
}

pub fn log(app: &App) -> Vec<Logged> {
    app.world().resource::<Log>().0.clone()
}
