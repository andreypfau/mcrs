use crate::{PathTypeTableError, StateFlags};
use mcrs_minecraft_physics::{CollisionTable, PathType};
use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct PathState {
    pub kind: PathType,
    pub top: f64,
    pub flags: StateFlags,
}
impl PathState {
    pub(crate) const AIR: Self = Self {
        kind: PathType::Open,
        top: 0.0,
        flags: StateFlags::AIR,
    };
}
#[derive(Clone, Debug)]
pub struct PathTypeTable(Vec<PathState>);
impl PathTypeTable {
    pub fn new(
        collision: &CollisionTable,
        flags: &[StateFlags],
    ) -> Result<Self, PathTypeTableError> {
        if flags.len() != collision.len() {
            return Err(PathTypeTableError {
                collision: collision.len(),
                flags: flags.len(),
            });
        }
        let states = flags
            .iter()
            .enumerate()
            .map(|(index, &flags)| {
                let Some(id) = u16::try_from(index)
                    .ok()
                    .and_then(|n| BlockStateId::checked(n, collision.len()))
                else {
                    return PathState::AIR;
                };
                let state = collision.state(id);
                let top = collision
                    .shape_without_context(state.class)
                    .map_or(0.0, |shape| shape.top());
                PathState {
                    kind: state.path,
                    top,
                    flags,
                }
            })
            .collect();
        Ok(Self(states))
    }
    #[must_use]
    pub fn state(&self, id: BlockStateId) -> PathState {
        self.0.get(id.index()).copied().unwrap_or(PathState::AIR)
    }
}
