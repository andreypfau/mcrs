mod execute;
mod gamemode;
mod locate;
mod teleport;

use crate::command::{AppCommands, CommandContext, CommandFailure, Invocation};
use crate::world::entity::player::ability::PlayerGameMode;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::Arg;
use mcrs_minecraft_command::{Candidate, EntityArgument, argument_text};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_entity::{PLAYER_HEIGHT, PLAYER_WIDTH};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

/// The vanilla commands: `execute`, `gamemode`, `teleport` with its alias `tp`, and `locate`.
pub struct VanillaCommandsPlugin;

impl Plugin for VanillaCommandsPlugin {
    fn build(&self, app: &mut App) {
        let added = [
            (
                "execute",
                app.add_modifying_command(execute::build, execute::run, execute::modify)
                    .map(drop),
            ),
            (
                "gamemode",
                app.add_command(gamemode::build, gamemode::run).map(drop),
            ),
            (
                "teleport",
                app.add_command(teleport::build, teleport::run).map(drop),
            ),
            (
                "locate",
                app.add_command(locate::build, locate::run).map(drop),
            ),
        ];
        for (name, result) in added {
            if let Err(error) = result {
                tracing::error!("{name}: {error}");
            }
        }
    }
}

/// A player of the dimension as an entity argument searches it.
#[derive(Clone, Debug)]
pub(crate) struct PlayerCandidate<'a> {
    pub entity: Entity,
    pub uuid: Uuid,
    pub name: &'a str,
    pub transform: Transform,
    pub mode: GameMode,
}

impl Candidate for PlayerCandidate<'_> {
    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn name(&self) -> &str {
        self.name
    }

    fn entity_type(&self) -> EntityType {
        EntityType::Player
    }

    fn position(&self) -> DVec3 {
        self.transform.translation
    }

    fn bounding_box(&self) -> (DVec3, DVec3) {
        let half = DVec3::new(PLAYER_WIDTH / 2.0, 0.0, PLAYER_WIDTH / 2.0);
        let p = self.transform.translation;
        (p - half, p + half + DVec3::Y * PLAYER_HEIGHT)
    }

    fn yaw(&self) -> f32 {
        self.transform.rotation.yaw()
    }

    fn pitch(&self) -> f32 {
        self.transform.rotation.pitch()
    }

    fn game_mode(&self) -> Option<GameMode> {
        Some(self.mode)
    }

    // chisle: experience, tags and teams have no state in the server yet: every player is at
    // level 0, has no tag and no team. Their components lift it.
    fn experience_level(&self) -> Option<i32> {
        Some(0)
    }

    fn alive(&self) -> bool {
        true
    }

    fn has_tag(&self, _: &str) -> bool {
        false
    }

    fn has_any_tag(&self) -> bool {
        false
    }

    fn team(&self) -> Option<&str> {
        None
    }

    fn in_source_dimension(&self) -> bool {
        true
    }
}

pub(crate) type PlayerView<'a> = (Entity, &'a GameProfile, &'a Transform, &'a PlayerGameMode);

// chisle: a selector searches the players of the source dimension only; the other dimensions
// and the entities that are not players are not candidates yet. A world-wide entity index
// lifts it.
pub(crate) fn candidates<'a>(
    players: impl Iterator<Item = PlayerView<'a>>,
) -> Vec<PlayerCandidate<'a>> {
    players
        .map(|(entity, profile, transform, mode)| PlayerCandidate {
            entity,
            uuid: profile.id,
            name: &profile.username,
            transform: *transform,
            mode: mode.0,
        })
        .collect()
}

/// The entities a selector argument finds, or the failure of its search.
pub(crate) fn select<'c>(
    context: &CommandContext,
    call: &Invocation,
    arg: Arg<EntityArgument>,
    all: &'c [PlayerCandidate<'_>],
) -> Result<Vec<&'c PlayerCandidate<'c>>, CommandFailure> {
    let selector = context.arg(call, arg)?;
    let source = context.source(call);
    let me = all.iter().find(|c| Some(c.entity) == call.source.entity);
    let mut out = Vec::new();
    let mut random = |bound: usize| rand::random_range(0..bound.max(1));
    selector
        .select(&source, me, all.iter(), &mut random, &mut out)
        .map_err(|error| CommandFailure(argument_text(context.line(), error)))?;
    Ok(out)
}

/// The players a selector finds, or the source itself when the branch has no selector.
pub(crate) fn targets_or_self(
    context: &CommandContext,
    call: &Invocation,
    arg: Option<Arg<EntityArgument>>,
    all: &[PlayerCandidate<'_>],
    none_found: &'static str,
) -> Result<Vec<Entity>, CommandFailure> {
    let Some(arg) = arg else {
        return Ok(call.source.entity.into_iter().collect());
    };
    let found = select(context, call, arg, all)?;
    if found.is_empty() {
        return Err(CommandFailure::translate(none_found, vec![]));
    }
    Ok(found.iter().map(|candidate| candidate.entity).collect())
}
