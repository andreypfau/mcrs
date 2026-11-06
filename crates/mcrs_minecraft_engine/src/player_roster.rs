//! The players of every dimension world as commands find them: each world reports its own
//! players after every tick, and a world that runs commands gets those of the other worlds.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

use crate::ability::PlayerGameMode;
use crate::game_profile::GameProfile;

/// One player of the dimension `dim` at the end of a tick.
#[derive(Clone, Debug, PartialEq)]
pub struct RosterRow {
    /// The label of the dimension that holds the player.
    pub dim: Entity,
    pub session: PlayerSession,
    pub uuid: Uuid,
    pub name: String,
    pub transform: Transform,
    pub mode: GameMode,
}

/// The players of the other dimension worlds at the end of the last tick, in this world. A world
/// gets them in the input of a tick that runs commands; in another tick it holds none.
#[derive(Resource, Default)]
pub struct ForeignPlayers(pub(crate) Vec<RosterRow>);

impl ForeignPlayers {
    pub fn rows(&self) -> &[RosterRow] {
        &self.0
    }
}

/// The players of this world at the end of this tick, for the output of the tick.
#[derive(Resource, Default)]
pub(crate) struct OwnPlayers(pub(crate) Vec<RosterRow>);

type RosterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Owner,
        &'static GameProfile,
        &'static Transform,
        &'static PlayerGameMode,
    ),
    With<Player>,
>;

/// Reports the players of this world, last in the tick.
pub(crate) fn report_own_players(
    label: Res<DimensionLabel>,
    players: RosterQuery,
    mut own: ResMut<OwnPlayers>,
) {
    own.0.clear();
    own.0.extend(
        players
            .iter()
            .map(|(&Owner(session), profile, &transform, mode)| RosterRow {
                dim: label.0,
                session,
                uuid: profile.id,
                name: profile.username.clone(),
                transform,
                mode: mode.0,
            }),
    );
}
