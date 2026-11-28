use bevy_ecs::component::Mutable;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_region_disk::column_ticks::ColumnTicks;
use mcrs_minecraft_simulation::GameTick;
use mcrs_voxel::ColumnPos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenTicks {
    pub ticks: ColumnTicks,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopiedTicks {
    pub ticks: ColumnTicks,
    pub changed: bool,
}

pub trait ColumnTickHolder: Resource<Mutability = Mutable> {
    fn take_column(
        &mut self,
        column: ColumnPos,
        rows: impl IntoIterator<Item = i32>,
        now: GameTick,
    ) -> TakenTicks;

    fn copy_column(
        &self,
        column: ColumnPos,
        rows: impl IntoIterator<Item = i32>,
        now: GameTick,
    ) -> CopiedTicks;
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoColumnTicks;

impl ColumnTickHolder for NoColumnTicks {
    fn take_column(
        &mut self,
        _column: ColumnPos,
        _rows: impl IntoIterator<Item = i32>,
        _now: GameTick,
    ) -> TakenTicks {
        TakenTicks {
            ticks: ColumnTicks::default(),
            changed: false,
        }
    }

    fn copy_column(
        &self,
        _column: ColumnPos,
        _rows: impl IntoIterator<Item = i32>,
        _now: GameTick,
    ) -> CopiedTicks {
        CopiedTicks {
            ticks: ColumnTicks::default(),
            changed: false,
        }
    }
}
