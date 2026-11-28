use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::Ref;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::query::Changed;
use bevy_ecs::query::Has;
use bevy_ecs::query::Or;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::schedule::InternedScheduleLabel;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::schedule::common_conditions;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;

use crate::Equipped;
use crate::Given;
use crate::InSlot;
use crate::ItemSet;
use crate::RequestDrop;
use crate::RequestResult;
use crate::Selected;
use crate::SlotChanged;
use crate::Slots;
use crate::item_components::ItemComponents;
use crate::request_scratch::RequestScratch;

pub struct ItemPlugin {
    schedule: InternedScheduleLabel,
}

impl Default for ItemPlugin {
    fn default() -> Self {
        Self {
            schedule: FixedUpdate.intern(),
        }
    }
}

impl ItemPlugin {
    #[must_use]
    pub fn in_schedule(mut self, schedule: impl ScheduleLabel) -> Self {
        self.schedule = schedule.intern();
        self
    }
}

#[derive(Clone, Copy, Default)]
pub struct ItemPersistencePlugin;

impl Plugin for ItemPersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemComponents>();
    }
}

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        let changed = common_conditions::any_match_filter::<HolderChanged>;
        app.init_resource::<ItemComponents>()
            .init_resource::<RequestScratch>()
            .add_message::<Given>()
            .add_message::<RequestDrop>()
            .add_message::<RequestResult>()
            .add_message::<SlotChanged>()
            .add_systems(
                self.schedule,
                project_equipped.in_set(ItemSet).run_if(changed),
            )
            .add_observer(unequip_unselected);
    }
}

type HolderChanged = (With<Selected>, Or<(Changed<Selected>, Changed<Slots>)>);

type EquippedMoved = (With<Equipped>, Or<(Changed<InSlot>, Without<InSlot>)>);

fn project_equipped(
    mut commands: Commands,
    changed: Query<(Entity, Ref<Selected>, &Slots), HolderChanged>,
    moved: Query<(Entity, Option<&InSlot>), EquippedMoved>,
    holders: Query<(Option<&Selected>, &Slots)>,
    equipped: Query<Has<Equipped>>,
) {
    let mut project = |stack: Entity, holder: Option<Entity>| {
        let selected = holder.and_then(|holder| selected_stack(&holders, holder)) == Some(stack);
        let Ok(has) = equipped.get(stack) else {
            return;
        };
        match (selected, has) {
            (true, false) => {
                commands.entity(stack).try_insert(Equipped);
            }
            (false, true) => {
                commands.entity(stack).try_remove::<Equipped>();
            }
            _ => {}
        }
    };
    for (holder, selected, slots) in &changed {
        if selected.is_changed() {
            for stack in slots.occupied() {
                project(stack, Some(holder));
            }
        } else if let Some(stack) = slots.get(usize::from(selected.0)) {
            project(stack, Some(holder));
        }
    }
    for (stack, slot) in &moved {
        project(stack, slot.map(InSlot::holder));
    }
}

fn unequip_unselected(
    unselected: On<Remove<Selected>>,
    mut commands: Commands,
    holders: Query<&Slots>,
    equipped: Query<(), With<Equipped>>,
) {
    let Ok(slots) = holders.get(unselected.entity) else {
        return;
    };
    for stack in slots.occupied() {
        if equipped.contains(stack) {
            commands.entity(stack).try_remove::<Equipped>();
        }
    }
}

fn selected_stack(holders: &Query<(Option<&Selected>, &Slots)>, holder: Entity) -> Option<Entity> {
    let (selected, slots) = holders.get(holder).ok()?;
    slots.get(usize::from(selected?.0))
}
