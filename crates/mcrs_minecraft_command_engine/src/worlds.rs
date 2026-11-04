use crate::command_targets::CommandTargets;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::DetectChangesMut;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::dimension_change::{Arrival, DimensionChangeError};
use mcrs_minecraft_engine::muxer::dim_outputs;
use mcrs_minecraft_engine::player_dimension_change;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;

/// Why an operation of [`Worlds`] changed no player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlayerChangeError {
    /// No online player has the session.
    #[error("no online player has this session")]
    NotOnline,
    /// The world of the player does not hold its entity.
    #[error("the world of the player does not hold its entity")]
    NotInWorld,
    /// No dimension runs at the index, or the destination is the dimension of the player.
    #[error("no dimension runs at this index")]
    NoDimension,
    /// The player did not change dimension.
    #[error("the player did not change dimension: {0}")]
    DimensionChange(DimensionChangeError),
}

/// The host and every dimension world, lent to the appliers of the effects of one run, between
/// two ticks. A player of [`CommandTargets`] changes only through [`Worlds::set_transform`],
/// [`Worlds::change_dimension`] and [`Worlds::set_game_mode`], which keep its entry current for
/// the later effects, stages and lines of the pass.
pub struct Worlds<'s, 'w> {
    pub(crate) host: &'s mut World,
    pub(crate) worlds: &'s mut [&'w mut World],
    pub(crate) targets: &'s mut CommandTargets,
}

impl Worlds<'_, '_> {
    /// The dimensions in the dimension order and the online players, as the effects so far
    /// left them.
    pub fn targets(&self) -> &CommandTargets {
        self.targets
    }

    /// The host world and every dimension world in the dimension order.
    pub fn host_and_worlds(&mut self) -> (&mut World, impl Iterator<Item = &mut World>) {
        (
            &mut *self.host,
            self.worlds.iter_mut().map(|world| &mut **world),
        )
    }

    /// The dimension world at `index` in the dimension order.
    pub fn world_mut(&mut self, index: usize) -> Option<&mut World> {
        self.worlds.get_mut(index).map(|world| &mut **world)
    }

    /// Sends `packet` after everything the dimensions handed over before it.
    pub fn deliver(&mut self, packet: OutboundPlayerPacket) {
        dim_outputs::deliver(self.host, packet);
    }

    /// Sets the transform of the player of the session `player` in the world that holds
    /// it. Returns the entity of the player there.
    pub fn set_transform(
        &mut self,
        player: PlayerSession,
        transform: Transform,
    ) -> Result<Entity, PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        let mut current = self
            .worlds
            .get_mut(entry.dimension)
            .and_then(|world| world.get_mut::<Transform>(entry.entity))
            .ok_or(PlayerChangeError::NotInWorld)?;
        *current = transform;
        entry.transform = transform;
        Ok(entry.entity)
    }

    /// Moves the player of the session `player` to the dimension at `destination` at once, as
    /// a departure of the world it leaves that arrives at once. Returns the entity of the player
    /// in its new world.
    pub fn change_dimension(
        &mut self,
        player: PlayerSession,
        destination: usize,
        arrival: Arrival,
    ) -> Result<Entity, PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        let [from, to] = self
            .worlds
            .get_disjoint_mut([entry.dimension, destination])
            .map_err(|_| PlayerChangeError::NoDimension)?;
        let transform = arrival.transform;
        let arrived =
            player_dimension_change::change_dimension(self.host, from, to, entry.entity, arrival)
                .map_err(PlayerChangeError::DimensionChange)?;
        entry.dimension = destination;
        entry.entity = arrived;
        entry.transform = transform;
        Ok(arrived)
    }

    /// Sets the game mode of the player of the session `player`. Its world sends the
    /// packets of the change on its next tick.
    pub fn set_game_mode(
        &mut self,
        player: PlayerSession,
        mode: GameMode,
    ) -> Result<(), PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        let mut current = self
            .worlds
            .get_mut(entry.dimension)
            .and_then(|world| world.get_mut::<PlayerGameMode>(entry.entity))
            .ok_or(PlayerChangeError::NotInWorld)?;
        current.set_if_neq(PlayerGameMode(mode));
        entry.mode = mode;
        Ok(())
    }
}
