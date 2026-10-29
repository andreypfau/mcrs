use crate::set::VanillaContext;
use bevy_math::DVec3;
use core::ops::{Deref, DerefMut};
use mcrs_command::ArgumentHandle;
use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_command_engine::{CommandFailure, Invocation, PlayerEntry};
use mcrs_minecraft_command_entity::{Candidate, EntityArgument, EntityBounds};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_entity::{PLAYER_HEIGHT, PLAYER_WIDTH};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

/// The players a selector found, which a handler keeps between its runs so that a search
/// allocates only when it finds more players than every search before it.
#[derive(Default)]
pub(crate) struct FoundPlayers {
    found: Vec<&'static PlayerEntry>,
    candidates: Vec<PlayerCandidate<'static>>,
    picked: Vec<&'static PlayerCandidate<'static>>,
}

impl FoundPlayers {
    /// Lends the buffers to `search` and takes them back on every path out of it.
    pub(crate) fn with<'c, R>(&mut self, search: impl FnOnce(&mut Selection<'c>) -> R) -> R {
        let mut selection = Selection {
            found: mcrs_command::recycle(core::mem::take(&mut self.found)),
            candidates: mcrs_command::recycle(core::mem::take(&mut self.candidates)),
            picked: core::mem::take(&mut self.picked),
        };
        let result = search(&mut selection);
        self.found = mcrs_command::recycle(selection.found);
        self.candidates = mcrs_command::recycle(selection.candidates);
        self.picked = selection.picked;
        result
    }
}

/// The players a search found, and the buffers the search reuses.
pub(crate) struct Selection<'c> {
    found: Vec<&'c PlayerEntry>,
    candidates: Vec<PlayerCandidate<'c>>,
    picked: Vec<&'static PlayerCandidate<'static>>,
}

impl<'c> Deref for Selection<'c> {
    type Target = Vec<&'c PlayerEntry>;

    fn deref(&self) -> &Self::Target {
        &self.found
    }
}

impl DerefMut for Selection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.found
    }
}

/// The players a selector argument finds in every dimension (ARG-ENT-013, ARG-ENT-014), in
/// `out`, or the failure of its search.
pub(crate) fn select<'c>(
    context: &'c VanillaContext,
    call: &Invocation,
    arg: ArgumentHandle<EntityArgument>,
    out: &mut Selection<'c>,
) -> Result<(), CommandFailure> {
    if select_if_present(context, call, arg, out)? {
        Ok(())
    } else {
        Err(CommandFailure::failed())
    }
}

fn select_if_present<'c>(
    context: &'c VanillaContext,
    call: &Invocation,
    arg: ArgumentHandle<EntityArgument>,
    out: &mut Selection<'c>,
) -> Result<bool, CommandFailure> {
    let Some(selector) = context.opt(call, arg)? else {
        return Ok(false);
    };
    let source = context.source(call)?;
    let targets = context.targets();
    let me = call
        .source
        .entity
        .and_then(|anchor| targets.player(anchor))
        .map(PlayerCandidate);
    let mut random = |bound: usize| rand::random_range(0..bound.max(1));
    let mut candidates = mcrs_command::recycle(core::mem::take(&mut out.candidates));
    candidates.extend(targets.players().iter().map(PlayerCandidate));
    let mut picked = mcrs_command::recycle(core::mem::take(&mut out.picked));
    let selected = selector.select(
        &source,
        me.as_ref(),
        candidates.iter(),
        &mut random,
        &mut picked,
    );
    out.found.clear();
    out.found
        .extend(picked.iter().map(|&&PlayerCandidate(player)| player));
    out.picked = mcrs_command::recycle(picked);
    candidates.clear();
    out.candidates = candidates;
    selected.map_err(|error| CommandFailure(error.text(context.line())))?;
    Ok(true)
}

/// The players a selector finds, or the source player when the branch has no selector, in
/// `out`.
pub(crate) fn targets_or_self<'c>(
    context: &'c VanillaContext,
    call: &Invocation,
    arg: ArgumentHandle<EntityArgument>,
    none_found: &'static str,
    out: &mut Selection<'c>,
) -> Result<(), CommandFailure> {
    if !select_if_present(context, call, arg, out)? {
        out.clear();
        out.extend(
            call.source
                .entity
                .and_then(|anchor| context.targets().player(anchor)),
        );
        return Ok(());
    }
    if out.is_empty() {
        return Err(CommandFailure::translate(none_found, vec![]));
    }
    Ok(())
}

/// A player as an entity selector sees it. The engine that owns [`PlayerEntry`] does not know
/// the selector crate, so the candidate is a view of the entry.
#[derive(Clone, Copy)]
struct PlayerCandidate<'c>(&'c PlayerEntry);

impl Candidate for PlayerCandidate<'_> {
    fn uuid(&self) -> Uuid {
        self.0.uuid
    }

    fn name(&self) -> &str {
        &self.0.name
    }

    fn entity_type(&self) -> EntityType {
        EntityType::Player
    }

    fn position(&self) -> DVec3 {
        self.0.transform.translation
    }

    fn bounding_box(&self) -> EntityBounds {
        let half = DVec3::new(PLAYER_WIDTH / 2.0, 0.0, PLAYER_WIDTH / 2.0);
        let position = self.0.transform.translation;
        EntityBounds {
            min: position - half,
            max: position + half + DVec3::Y * PLAYER_HEIGHT,
        }
    }

    fn yaw(&self) -> f32 {
        self.0.transform.rotation.yaw()
    }

    fn pitch(&self) -> f32 {
        self.0.transform.rotation.pitch()
    }

    fn game_mode(&self) -> Option<GameMode> {
        Some(self.0.mode)
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

    fn dimension(&self) -> usize {
        self.0.dimension
    }
}
