use alloc::vec::Vec;

use bevy_ecs::change_detection::Tick;
use bevy_ecs::component::Component;
use bevy_ecs::component::ComponentId;
use bevy_ecs::component::Components;
use bevy_ecs::entity::Entities;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::FilteredAccess;
use bevy_ecs::query::FilteredAccessSet;
use bevy_ecs::storage::TableId;
use bevy_ecs::storage::Tables;
use bevy_ecs::system::ReadOnlySystemParam;
use bevy_ecs::system::Res;
use bevy_ecs::system::SystemMeta;
use bevy_ecs::system::SystemParam;
use bevy_ecs::system::SystemParamValidationError;
use bevy_ecs::world::World;
use bevy_ecs::world::unsafe_world_cell::UnsafeWorldCell;

use crate::Removed;
use crate::cells::Cells;
use crate::column::Column;
use crate::item_components::ItemComponents;
use crate::row::Row;
use crate::row::bit;

/// Reads the cells of the item components of any stack.
///
/// It reads each item component type that the world had when the system was built.
pub(crate) struct ItemColumns<'w, 's> {
    entities: &'w Entities,
    tables: &'w Tables,
    components: &'w Components,
    state: &'s mut ColumnsState,
}

/// The rows that the system declared its reads of, and the item mask of each table.
pub(crate) struct ColumnsState {
    registry: ComponentId,
    rows: Vec<Row>,
    removed: Column<Removed>,
    masks: Vec<Option<u128>>,
}

impl<'w> ItemColumns<'w, '_> {
    pub(crate) fn rows(&self) -> &[Row] {
        &self.state.rows
    }

    /// The column of `T` when `T` is the type of a row.
    pub(crate) fn column<T: Component>(&self) -> Option<Column<T>> {
        Column::<T>::of(self.components)
            .filter(|column| self.state.rows.iter().any(|row| row.id() == column.id()))
    }

    pub(crate) fn cells(&mut self, stack: Entity) -> Option<Cells<'w>> {
        let location = self.entities.get(stack).ok()??;
        let table = self.tables.get(location.table_id)?;
        let mask = self.mask(location.table_id, |id| table.has_column(id));
        // SAFETY: the location is that of a live entity, and no row moves while the shared borrow
        // of the world for `'w` lasts. `init_access` declared a read of each row and of `Removed`.
        Some(unsafe { Cells::new(table, location.table_row, mask, self.state.removed) })
    }

    fn mask(&mut self, table: TableId, has: impl Fn(ComponentId) -> bool) -> u128 {
        let index = table.as_usize();
        if let Some(Some(mask)) = self.state.masks.get(index) {
            return *mask;
        }
        let mask = (self.state.rows.iter().enumerate())
            .filter(|(_, row)| has(row.id()))
            .fold(0, |mask, (index, _)| mask | bit(index));
        if self.state.masks.len() <= index {
            self.state.masks.resize(index + 1, None);
        }
        if let Some(slot) = self.state.masks.get_mut(index) {
            *slot = Some(mask);
        }
        mask
    }
}

// SAFETY: `init_access` declares a read of the registry, of the column of each row and of
// `Removed`, and it panics when a parameter before it writes one of them. `get_param` reads no
// other value, and it refuses a registry with rows that the state does not know.
unsafe impl SystemParam for ItemColumns<'_, '_> {
    type State = ColumnsState;
    type Item<'world, 'state> = ItemColumns<'world, 'state>;

    fn init_state(world: &mut World) -> ColumnsState {
        world.init_resource::<ItemComponents>();
        let registry = <Res<ItemComponents> as SystemParam>::init_state(world);
        let rows = (world.get_resource::<ItemComponents>())
            .map_or_else(Vec::new, |components| components.rows().to_vec());
        ColumnsState {
            registry,
            rows,
            removed: Column::register(world),
            masks: Vec::new(),
        }
    }

    fn init_access(
        state: &ColumnsState,
        system_meta: &mut SystemMeta,
        component_access_set: &mut FilteredAccessSet,
        world: &mut World,
    ) {
        <Res<ItemComponents> as SystemParam>::init_access(
            &state.registry,
            system_meta,
            component_access_set,
            world,
        );
        let ids = state.rows.iter().map(Row::id);
        for id in ids.chain([state.removed.id()]) {
            let mut read = FilteredAccess::default();
            read.add_read(id);
            if !component_access_set.get_conflicts_single(&read).is_empty() {
                #[expect(
                    clippy::panic,
                    reason = "the contract of `init_access` asks for a panic on a conflict"
                )]
                {
                    panic!(
                        "an item read in system {} conflicts with a write of a parameter before \
                         it; take both through a ParamSet",
                        system_meta.name()
                    );
                }
            }
            component_access_set.add(read);
        }
    }

    unsafe fn get_param<'world, 'state>(
        state: &'state mut ColumnsState,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'world>,
        change_tick: Tick,
    ) -> Result<ItemColumns<'world, 'state>, SystemParamValidationError> {
        // SAFETY: `init_access` declared the read of the registry, and the caller gives a world
        // with the access that `init_access` declared. The storages are read only through the
        // cells of the rows that `init_access` declared.
        let (registry, tables) = unsafe {
            let registry = <Res<ItemComponents> as SystemParam>::get_param(
                &mut state.registry,
                system_meta,
                world,
                change_tick,
            )?;
            (registry, &world.storages().tables)
        };
        if registry.rows().len() != state.rows.len() {
            return Err(SystemParamValidationError::skipped::<Self>(
                "an item component was registered after the system was built",
            ));
        }
        Ok(ItemColumns {
            entities: world.entities(),
            tables,
            components: world.components(),
            state,
        })
    }
}

// SAFETY: the parameter reads components and the registry, and writes only its own state.
unsafe impl ReadOnlySystemParam for ItemColumns<'_, '_> {}
