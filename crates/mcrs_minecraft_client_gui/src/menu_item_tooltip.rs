use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::{
    GuiExtent, GuiPoint, GuiRect, TOOLTIP_WRAP_WIDTH, TooltipAnchor, TooltipRequest, WidgetId,
};
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::{SlotRef, Slots, Stacks};
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_entity::attribute_bases;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item::ItemHoverMap;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::{ItemTable, Prototype};
use mcrs_minecraft_item_tooltip::{
    AttackBases, TooltipDetail, TooltipInputs, append_tooltip_parts,
};
use mcrs_minecraft_menu_geometry::MenuGeometry;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_text::Text;

use crate::font::{Font, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::scene::{GuiAtlas, GuiConfig, GuiScene, GuiSize};
use crate::text::{self, Wrap};
use crate::tooltip::LineGap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HoveredEntry {
    menu: Entity,
    entry: u16,
    stack: Entity,
    bounds: GuiRect,
}

#[derive(Clone, Debug, PartialEq)]
struct ShownStack {
    entry: u16,
    stack: Entity,
    value: ItemStack,
    detail: TooltipDetail,
    attack: AttackBases,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HoverStart {
    menu: Entity,
    entry: u16,
    millis: u64,
}

#[derive(Default)]
pub(crate) struct MenuItemTooltip {
    shown: Option<ShownStack>,
    start: Option<HoverStart>,
    parts: Vec<Text>,
    spans: Vec<Span>,
    lines: Vec<Vec<Span>>,
}

struct LineSources<'a> {
    items: &'a ItemTable,
    registries: &'a CurrentRegistries,
    language: &'a ClientLanguage,
    font: &'a Font,
}

impl MenuItemTooltip {
    fn forget(&mut self) {
        self.shown = None;
    }

    fn follow(
        &mut self,
        hovered: HoveredEntry,
        detail: TooltipDetail,
        attack: AttackBases,
        stacks: &mut Stacks,
        sources: &LineSources,
    ) {
        let current = self.shown.as_ref().is_some_and(|shown| {
            shown.entry == hovered.entry
                && shown.stack == hovered.stack
                && shown.detail == detail
                && shown.attack == attack
                && stacks.equals_value(hovered.stack, &shown.value)
        });
        if current {
            return;
        }
        self.lines.clear();
        let shown = stacks.to_value(hovered.stack).map(|value| ShownStack {
            entry: hovered.entry,
            stack: hovered.stack,
            value,
            detail,
            attack,
        });
        if let Some(shown) = &shown {
            self.write(&shown.value, detail, attack, sources);
        }
        self.shown = shown;
    }

    fn write(
        &mut self,
        stack: &ItemStack,
        detail: TooltipDetail,
        attack: AttackBases,
        sources: &LineSources,
    ) {
        let items = sources.items;
        let prototype_components = items
            .prototype(stack.kind)
            .map_or(0, |prototype| Prototype::components(prototype).len());
        let mut inputs = TooltipInputs::new(attack, prototype_components);
        inputs.detail = detail;
        self.parts.clear();
        let names = RegistryNames::new(sources.registries, items);
        append_tooltip_parts(stack, items, &names, &inputs, &mut self.parts);
        self.spans.clear();
        for (index, part) in self.parts.iter().enumerate() {
            if index > 0 {
                self.spans.push(Span::new("\n", WHITE));
            }
            match part.map(&ItemHoverMap) {
                Ok(part) => self
                    .spans
                    .extend(text::text_spans(&part, sources.language, WHITE)),
                Err(_) => self
                    .spans
                    .push(Span::new(sources.language.plain_text(part), WHITE)),
            }
        }
        text::wrap(
            &self.spans,
            TOOLTIP_WRAP_WIDTH,
            Wrap::Tooltip,
            sources.font,
            &mut self.lines,
        );
    }

    fn lines_after_delay(&mut self, hovered: HoveredEntry, now_ms: u64) -> Option<&[Vec<Span>]> {
        if self.lines.is_empty() {
            self.start = None;
            return None;
        }
        let start = match self.start {
            Some(start) if start.menu == hovered.menu && start.entry == hovered.entry => start,
            _ => *self.start.insert(HoverStart {
                menu: hovered.menu,
                entry: hovered.entry,
                millis: now_ms,
            }),
        };
        (now_ms > start.millis).then_some(self.lines.as_slice())
    }
}

fn player_attack_bases(instances: Option<&AttributeInstances>) -> AttackBases {
    let bases = attribute_bases(EntityType::Player);
    let base = |attribute: Attribute| {
        instances
            .and_then(|instances| instances.base(attribute))
            .or_else(|| bases.and_then(|bases| bases.base(attribute)))
            .unwrap_or(attribute.definition().default)
    };
    AttackBases {
        attack_damage: base(Attribute::AttackDamage),
        attack_speed: base(Attribute::AttackSpeed),
    }
}

fn hovered_entry(
    size: GuiSize,
    pointer: GuiPoint,
    menu: Entity,
    layout: &MenuLayout,
    parts: &MenuParts,
    occupant: impl Fn(SlotRef) -> Option<Entity>,
) -> Option<HoveredEntry> {
    let geometry = MenuGeometry::of_layout(layout)?;
    let corner = geometry.window(size.width, size.height);
    let Clicked::Entry(entry) = geometry.clicked_at(corner, pointer) else {
        return None;
    };
    let area = geometry.entry_area(entry)?;
    let stack = layout
        .entry(entry)
        .and_then(|slot| parts.slot(slot))
        .and_then(occupant)?;
    Some(HoveredEntry {
        menu,
        entry,
        stack,
        bounds: GuiRect::new(
            corner.x.saturating_add(area.x),
            corner.y.saturating_add(area.y),
            area.width,
            area.height,
        ),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
pub(crate) fn request_menu_item_tooltip(
    player: Query<(&PlayerHolders, &Menus, Option<&AttributeInstances>)>,
    menus: Query<(&MenuLayout, &MenuParts)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    items: Res<ItemTable>,
    registries: Option<Res<CurrentRegistries>>,
    language: Option<Res<ClientLanguage>>,
    atlas: Option<Res<GuiAtlas>>,
    config: Res<GuiConfig>,
    screen: Res<Screen>,
    time: Res<Time<Real>>,
    mut scene: ResMut<GuiScene>,
    mut tooltip: Local<MenuItemTooltip>,
) {
    let (Some(language), Some(atlas), Some(registries), Screen::Menu) =
        (language, atlas, registries, *screen)
    else {
        tooltip.start = None;
        return;
    };
    if language.is_changed() || items.is_changed() || registries.is_changed() || atlas.is_changed()
    {
        tooltip.forget();
    }
    let occupant = |slot: SlotRef| holders.get(slot.holder).ok()?.get(usize::from(slot.index));
    let pointer = GuiPoint::new(scene.cursor.x, scene.cursor.y);
    let player = player.single().ok();
    let attack = player_attack_bases(player.and_then(|(_, _, instances)| instances));
    let hovered = player.and_then(|(player, player_menus, _)| {
        let cursor = SlotRef {
            holder: player.cursor,
            index: 0,
        };
        if occupant(cursor).is_some() {
            return None;
        }
        let menu = player_menus.open()?;
        let (layout, parts) = menus.get(menu).ok()?;
        hovered_entry(scene.size, pointer, menu, layout, parts, occupant)
    });
    let Some(hovered) = hovered else {
        tooltip.start = None;
        return;
    };
    let detail = if config.advanced_tooltips {
        TooltipDetail::Advanced
    } else {
        TooltipDetail::Basic
    };
    let sources = LineSources {
        items: &items,
        registries: &registries,
        language: &language,
        font: &atlas.0.font,
    };
    tooltip.follow(hovered, detail, attack, &mut stacks, &sources);
    let now_ms = u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX);
    let screen = GuiExtent::new(scene.size.width, scene.size.height);
    let Some(lines) = tooltip.lines_after_delay(hovered, now_ms) else {
        return;
    };
    scene.tooltip.request_widget_lines(
        TooltipRequest {
            widget: WidgetId(u32::from(hovered.entry)),
            bounds: hovered.bounds,
            screen,
            anchor: TooltipAnchor::Mouse(pointer),
        },
        None,
        LineGap::AfterFirst,
        lines,
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use mcrs_minecraft_engine_inventory_menu::{MenuButtons, MenuCommandsExt};
    use mcrs_minecraft_engine_item::{ItemCommandsExt, ItemPlugin, StackCommandsExt};
    use mcrs_minecraft_engine_registries::test_item_table;
    use mcrs_minecraft_entity_attribute::InstanceEdit;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_minecraft_inventory_file::PlayerItems;
    use mcrs_minecraft_item_stack::{Count, CustomName, Kind, NameLookup, NameTable};
    use mcrs_minecraft_menu_key::menu_keys::MenuType;
    use mcrs_minecraft_model::resource_set::ResourceSet;
    use mcrs_minecraft_player_holders::{PlayerHoldersPlugin, restore_holders};
    use mcrs_minecraft_resource::ResourceLocation;
    use mcrs_test_alloc::CountingAllocator;

    use super::*;
    use crate::menu_test_app::{ATLAS, GUI};

    fn stack(items: &ItemTable, item: &str) -> ItemStack {
        let name = ResourceLocation::read(item).unwrap();
        let kind = Kind(items.id(NameTable::Item, name).unwrap());
        ItemStack::new(kind, Count::new(1).unwrap())
    }

    fn chest_app() -> (App, Entity) {
        let items = test_item_table();
        let renamed = stack(&items, "minecraft:stone").set(CustomName(Text::text("Rock")));
        let plain = stack(&items, "minecraft:stone");
        let mut app = App::new();
        app.insert_resource(items)
            .insert_resource(CurrentRegistries::new(test_registries().clone()))
            .insert_resource(ClientLanguage::load(ResourceSet::corpus()).unwrap())
            .insert_resource(GuiAtlas(Arc::clone(&ATLAS)))
            .insert_resource(GuiConfig {
                scale: 1,
                cursor: None,
                frozen_ticks: None,
                advanced_tooltips: false,
            })
            .insert_resource(GuiScene {
                size: GuiSize {
                    width: GUI.width,
                    height: GUI.height,
                    scale: 1,
                },
                ..GuiScene::default()
            })
            .insert_resource(Screen::Menu)
            .init_resource::<Time<Real>>()
            .add_plugins((ItemPlugin::default(), PlayerHoldersPlugin))
            .add_systems(
                Update,
                (
                    |mut scene: ResMut<GuiScene>| scene.tooltip.clear(),
                    request_menu_item_tooltip,
                )
                    .chain(),
            );
        let world = app.world_mut();
        let player = world.spawn_empty().id();
        world
            .run_system_cached_with(restore_holders, (player, PlayerItems::default()))
            .unwrap();
        world.flush();
        let chest = world.spawn(Slots::new(27)).id();
        let mut commands = world.commands();
        commands.entity(player).open_menu(
            MenuLayout::of(MenuType::Generic9x3).unwrap(),
            MenuParts::new([chest, player]).unwrap(),
            MenuButtons::default(),
        );
        commands.spawn_stack(renamed).put_at(chest, 0);
        commands.spawn_stack(plain).put_at(chest, 1);
        world.flush();
        (app, player)
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn frame(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .update_with_duration(Duration::from_millis(5));
        app.update();
    }

    fn shown_lines(app: &App) -> Vec<String> {
        let scene = app.world().resource::<GuiScene>();
        let lines = scene.tooltip.lines().unwrap_or_default();
        lines
            .iter()
            .map(|line| line.iter().map(|span| span.text.as_str()).collect())
            .collect()
    }

    fn hover(app: &mut App, entry: u16) -> Option<String> {
        let layout = MenuLayout::of(MenuType::Generic9x3).unwrap();
        let geometry = MenuGeometry::of_layout(&layout).unwrap();
        let corner = geometry.window(GUI.width, GUI.height);
        let at = geometry.entry(entry).unwrap();
        app.world_mut().resource_mut::<GuiScene>().cursor =
            IVec2::new(corner.x + at.x + 8, corner.y + at.y + 8);
        frame(app);
        frame(app);
        shown_lines(app).into_iter().next()
    }

    #[test]
    fn the_hovered_entry_shows_the_name_of_its_stack_unless_the_cursor_holds_one() {
        let (mut app, player) = chest_app();
        assert_eq!(hover(&mut app, 0).as_deref(), Some("Rock"));
        assert_eq!(hover(&mut app, 1).as_deref(), Some("Stone"));
        assert_eq!(hover(&mut app, 2), None);
        let cursor = app.world().get::<PlayerHolders>(player).unwrap().cursor;
        let carried = stack(app.world().resource::<ItemTable>(), "minecraft:dirt");
        let world = app.world_mut();
        world.commands().spawn_stack(carried).put_at(cursor, 0);
        world.flush();
        assert_eq!(hover(&mut app, 1), None);
    }

    #[test]
    fn the_hover_delay_starts_again_after_a_frame_without_the_menu_screen() {
        let (mut app, _) = chest_app();
        assert_eq!(hover(&mut app, 1).as_deref(), Some("Stone"));
        *app.world_mut().resource_mut::<Screen>() = Screen::None;
        frame(&mut app);
        *app.world_mut().resource_mut::<Screen>() = Screen::Menu;
        frame(&mut app);
        assert_eq!(shown_lines(&app), Vec::<String>::new());
        frame(&mut app);
        assert_eq!(shown_lines(&app), ["Stone"]);
    }

    #[test]
    fn a_weapon_adds_the_attack_base_of_the_local_player() {
        let (mut app, player) = chest_app();
        let sword = stack(app.world().resource::<ItemTable>(), "minecraft:iron_sword");
        let mut instances = AttributeInstances::new(attribute_bases(EntityType::Player).unwrap());
        let _ = instances.apply(Attribute::AttackDamage, &InstanceEdit::SetBase(3.0));
        let world = app.world_mut();
        let chest = world.get::<Menus>(player).unwrap().open().unwrap();
        let first = MenuLayout::of(MenuType::Generic9x3)
            .unwrap()
            .entry(0)
            .unwrap();
        let chest = world
            .get::<MenuParts>(chest)
            .unwrap()
            .slot(first)
            .unwrap()
            .holder;
        world.entity_mut(player).insert(instances);
        world.commands().spawn_stack(sword).put_at(chest, 3);
        world.flush();
        hover(&mut app, 3);
        assert!(
            shown_lines(&app).contains(&String::from(" 8 Attack Damage")),
            "{:?}",
            shown_lines(&app)
        );
    }

    #[test]
    fn a_steady_hover_frame_allocates_nothing() {
        let (mut app, _) = chest_app();
        assert_eq!(hover(&mut app, 0).as_deref(), Some("Rock"));
        let world = app.world_mut();
        let system = world.register_system(request_menu_item_tooltip);
        let run = |world: &mut World| {
            world.resource_mut::<GuiScene>().tooltip.clear();
            world
                .resource_mut::<Time<Real>>()
                .update_with_duration(Duration::from_millis(5));
            world.run_system(system).unwrap();
        };
        run(world);
        run(world);
        let before = mcrs_test_alloc::allocations();
        run(world);
        let allocations = mcrs_test_alloc::allocations() - before;
        assert_eq!(
            allocations, 0,
            "the hovered entry and its stack did not change"
        );
        assert_eq!(shown_lines(&app).first().map(String::as_str), Some("Rock"));
    }
}
