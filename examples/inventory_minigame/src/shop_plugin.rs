use bevy_app::App;
use bevy_app::Plugin;
use bevy_app::Update;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::message::MessageReader;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_inventory_menu::EntryPressed;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuOpened;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_text::Text;

use crate::OpenShop;

const SHOP_SIZE: u16 = 27;

/// Opens a shop menu at [`OpenShop`]. The shop holds one icon for each offer, and pressing an
/// offer entry gives the player the offered stack.
#[derive(Clone, Debug, Default)]
pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        let Some(count) = Count::new(1) else {
            return;
        };
        let Some(layout) = MenuLayout::of(MenuType::Generic9x3) else {
            return;
        };
        let offers = vec![Offer {
            entry: 13,
            stack: ItemStack::new(Kind(3), count),
        }];
        app.insert_resource(Shop { layout, offers })
            .add_observer(open_shop)
            .add_observer(mark_shop_menu)
            .add_observer(close_shop)
            .add_systems(Update, give_offers.run_if(on_message::<EntryPressed>));
    }
}

#[derive(Clone, Debug)]
struct Offer {
    entry: u16,
    stack: ItemStack,
}

#[derive(Component)]
struct ShopHolder;

#[derive(Component)]
struct ShopMenu {
    holder: Entity,
}

#[derive(Resource)]
struct Shop {
    layout: MenuLayout,
    offers: Vec<Offer>,
}

fn open_shop(open: On<OpenShop>, shop: Res<Shop>, mut commands: Commands) {
    let player = open.entity;
    let holder = commands
        .spawn((
            Slots::new(usize::from(SHOP_SIZE)),
            SlotPolicy::locked(),
            ShopHolder,
        ))
        .id();
    let mut buttons = MenuButtons::default();
    for offer in &shop.offers {
        commands
            .spawn_stack(offer.stack.clone())
            .put_at(holder, offer.entry);
        buttons = buttons.with(offer.entry);
    }
    let Some(parts) = MenuParts::new([holder, player]) else {
        return;
    };
    commands.entity(player).open_titled_menu(
        shop.layout.clone(),
        parts,
        buttons,
        MenuTitle(Text::text("Shop")),
    );
}

fn mark_shop_menu(
    opened: On<MenuOpened>,
    menus: Query<&MenuParts>,
    shop_holders: Query<(), With<ShopHolder>>,
    mut commands: Commands,
) {
    let Ok(parts) = menus.get(opened.menu) else {
        return;
    };
    if let Some(holder) = parts.holder(MenuLayout::GENERIC_CONTAINER)
        && shop_holders.contains(holder)
    {
        commands.entity(opened.menu).insert(ShopMenu { holder });
    }
}

fn close_shop(removed: On<Remove<ShopMenu>>, shop_menus: Query<&ShopMenu>, mut commands: Commands) {
    if let Ok(shop_menu) = shop_menus.get(removed.entity) {
        commands.entity(shop_menu.holder).try_despawn();
    }
}

fn give_offers(
    mut pressed: MessageReader<EntryPressed>,
    shop: Res<Shop>,
    shop_menus: Query<(), With<ShopMenu>>,
    mut commands: Commands,
) {
    for press in pressed.read() {
        if !shop_menus.contains(press.menu) {
            continue;
        }
        let Some(offer) = shop.offers.iter().find(|offer| offer.entry == press.entry) else {
            continue;
        };
        commands.entity(press.viewer).give(offer.stack.clone());
    }
}
