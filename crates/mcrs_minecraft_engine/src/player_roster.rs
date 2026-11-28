use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Populated, Res, ResMut};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_pose::Dimensions;
use mcrs_minecraft_protocol::uuid::Uuid;

use crate::game_profile::GameProfile;
use crate::player_dimensions::PlayerBody;

#[derive(Clone, Debug, PartialEq)]
pub struct RosterRow {
    pub dim: Entity,
    pub session: PlayerSession,
    pub uuid: Uuid,
    pub name: String,
    pub transform: Transform,
    pub mode: GameMode,
    pub dimensions: Dimensions,
}

#[derive(Resource, Default)]
pub struct ForeignPlayers(pub(crate) Vec<RosterRow>);

impl ForeignPlayers {
    pub fn rows(&self) -> &[RosterRow] {
        &self.0
    }
}

#[derive(Resource, Default)]
pub(crate) struct OwnPlayers(pub(crate) Vec<RosterRow>);

type RosterQuery<'w, 's> = Populated<
    'w,
    's,
    (
        &'static Owner,
        &'static GameProfile,
        &'static Transform,
        &'static PlayerGameMode,
        PlayerBody,
    ),
    With<Player>,
>;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the roster moves to the coordinator by value on each tick, and the profile keeps the name as a `String`, so each row clones it"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
pub(crate) fn report_own_players(
    label: Res<DimensionLabel>,
    players: RosterQuery,
    mut own: ResMut<OwnPlayers>,
) {
    own.0.clear();
    own.0.extend(
        players.iter().map(
            |(&Owner(session), profile, &transform, mode, body)| RosterRow {
                dim: label.0,
                session,
                uuid: profile.id,
                name: profile.username.clone(),
                transform,
                mode: mode.0,
                dimensions: body.dimensions(),
            },
        ),
    );
}
