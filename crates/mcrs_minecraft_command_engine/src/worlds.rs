use crate::command_targets::CommandTargets;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::coordinator::clock::EditClock;
use mcrs_minecraft_engine::dim_mail::{self, DimMail};
use mcrs_minecraft_engine::dimension_change::{Arrival, DimensionChangeError};
use mcrs_minecraft_engine::player_order::{self, OrderedPlayer, PlayerOrder, TeleportOrder};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlayerChangeError {
    #[error("no online player has this session")]
    NotOnline,
    #[error("no dimension runs at this index")]
    NoDimension,
    #[error("the player did not change: {0}")]
    Change(DimensionChangeError),
}

pub struct Worlds<'w> {
    pub(crate) world: &'w mut World,
    pub(crate) targets: &'w mut CommandTargets,
}

impl Worlds<'_> {
    pub fn targets(&self) -> &CommandTargets {
        self.targets
    }

    pub fn world_mut(&mut self) -> &mut World {
        self.world
    }

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

    pub fn edit_clock(&mut self, edit: EditClock) {
        dim_mail::edit_clock(self.world, edit);
    }
}

fn send(world: &mut World, session: PlayerSession, order: PlayerOrder) {
    world
        .get_resource_or_init::<DimMail>()
        .order(OrderedPlayer { session, order });
}
