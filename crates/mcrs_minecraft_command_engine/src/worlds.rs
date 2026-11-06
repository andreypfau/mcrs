use crate::command_targets::CommandTargets;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::coordinator::clock::EditClock;
use mcrs_minecraft_engine::dim_mail::{self, DimMail};
use mcrs_minecraft_engine::dimension_change::{Arrival, DimensionChangeError};
use mcrs_minecraft_engine::player_order::{self, OrderedPlayer, PlayerOrder, TeleportOrder};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;

/// Why an operation of [`Worlds`] changed no player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlayerChangeError {
    /// No online player has the session.
    #[error("no online player has this session")]
    NotOnline,
    /// No dimension runs at the index.
    #[error("no dimension runs at this index")]
    NoDimension,
    /// The world that holds the player did not change it.
    #[error("the player did not change: {0}")]
    Change(DimensionChangeError),
}

/// The world a run of a handler happened in, lent to the appliers of its effects. A player that
/// this world holds changes at once; a player of another world changes in the next tick of that
/// world, through an order. A player of [`CommandTargets`] changes only through
/// [`Worlds::teleport`] and [`Worlds::set_game_mode`], which keep its entry current for the later
/// effects, stages and lines of the tick.
pub struct Worlds<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) targets: &'w mut CommandTargets,
}

impl Worlds<'_> {
    /// The dimensions in the dimension order and the online players, as the effects so far
    /// left them.
    pub fn targets(&self) -> &CommandTargets {
        self.targets
    }

    /// The world the run happened in.
    pub fn world_mut(&mut self) -> &mut World {
        self.world
    }

    /// Moves the player of the session `player` to `arrival` in the dimension at `destination`.
    /// A player of this world that goes to another dimension leaves after the tick.
    pub fn teleport(
        &mut self,
        player: PlayerSession,
        destination: usize,
        arrival: Arrival,
    ) -> Result<(), PlayerChangeError> {
        let to = self
            .targets
            .dimensions
            .get(destination)
            .ok_or(PlayerChangeError::NoDimension)?
            .key
            .clone();
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        entry.dimension = destination;
        entry.transform = arrival.transform;
        let order = TeleportOrder { to, arrival };
        match entry.entity {
            Some(entity) => {
                player_order::teleport(self.world, entity, order).map_err(PlayerChangeError::Change)
            }
            None => {
                send(self.world, player, PlayerOrder::Teleport(order));
                Ok(())
            }
        }
    }

    /// Sets the game mode of the player of the session `player`. Its world sends the packets of
    /// the change.
    pub fn set_game_mode(
        &mut self,
        player: PlayerSession,
        mode: GameMode,
    ) -> Result<(), PlayerChangeError> {
        let entry = self
            .targets
            .player_mut(player)
            .ok_or(PlayerChangeError::NotOnline)?;
        entry.mode = mode;
        match entry.entity {
            Some(entity) => player_order::set_game_mode(self.world, entity, mode)
                .map_err(PlayerChangeError::Change),
            None => {
                send(self.world, player, PlayerOrder::SetGameMode(mode));
                Ok(())
            }
        }
    }

    /// Changes a world clock: this world reads the change at once, and the other worlds in
    /// their next tick.
    pub fn edit_clock(&mut self, edit: EditClock) {
        dim_mail::edit_clock(self.world, edit);
    }
}

fn send(world: &mut World, session: PlayerSession, order: PlayerOrder) {
    world
        .get_resource_or_init::<DimMail>()
        .order(OrderedPlayer { session, order });
}
