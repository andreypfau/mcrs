use bevy::prelude::*;
use mcrs_minecraft_engine_inventory_menu::{MenuParts, MenuTitle, Menus};
use mcrs_minecraft_engine_item::{Removed, SlotChanged, Slots};
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_icon::IconMeshes;
use mcrs_minecraft_item_stack::{Count, Kind};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_model::item_model_bake::ItemModels;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_config::GuiConfig;
use crate::gui_pointer::GuiPointer;
use crate::gui_size::GuiSize;
use crate::layer_due::layer_due;
use crate::shown_drag_preview::ShownDragPreview;
use crate::slot_icon::SlotIcon;
use mcrs_minecraft_font::font::Font;
use mcrs_minecraft_font::language::ClientLanguage;

type HolderChanged = Or<(
    Changed<PlayerHolders>,
    Changed<Menus>,
    Changed<Abilities>,
    Changed<AttributeInstances>,
    Changed<Slots>,
)>;

type MenuChanged = Or<(Changed<MenuLayout>, Changed<MenuParts>, Changed<MenuTitle>)>;

type StackChanged = Or<(
    Changed<SlotIcon>,
    Changed<Kind>,
    Changed<Count>,
    Changed<Removed>,
)>;

pub(crate) fn gui_frame_changed() -> impl SystemCondition<()> {
    resource_changed::<GuiSize>
        .or_else(resource_exists_and_changed::<GuiAtlasLayout>)
        .or_else(resource_exists_and_changed::<Font>)
}

pub(crate) fn changed_or_removed<C: Component>() -> impl SystemCondition<()> {
    any_match_filter::<Changed<C>>.or_else(any_component_removed::<C>)
}

pub(crate) fn layer_stale<Kind: Component>() -> impl SystemCondition<()> {
    any_match_filter::<Added<Kind>>
        .or_else(gui_frame_changed())
        .or_else(layer_due::<Kind>)
}

pub(crate) fn item_icons_changed() -> impl SystemCondition<()> {
    resource_exists_and_changed::<ItemTable>
        .or_else(resource_exists_and_changed::<CurrentRegistries>)
        .or_else(resource_exists_and_changed::<ItemModels>)
        .or_else(resource_exists_and_changed::<IconMeshes>)
}

pub(crate) fn slot_view_changed() -> impl SystemCondition<()> {
    resource_changed::<GuiPointer>
        .or_else(resource_changed::<GuiConfig>)
        .or_else(resource_changed_or_removed::<ShownDragPreview>)
        .or_else(resource_exists_and_changed::<ClientLanguage>)
        .or_else(any_match_filter::<HolderChanged>)
        .or_else(any_match_filter::<MenuChanged>)
        .or_else(any_match_filter::<StackChanged>)
        .or_else(any_component_removed::<Menus>)
        .or_else(any_component_removed::<PlayerHolders>)
        .or_else(any_component_removed::<Abilities>)
        .or_else(any_component_removed::<AttributeInstances>)
        .or_else(on_message::<SlotChanged>)
        .or_else(item_icons_changed())
}
