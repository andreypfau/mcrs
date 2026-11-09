use crate::mob_view::{MobTarget, MobView};
use crate::set::VanillaContext;
use bevy_math::DVec3;
use mcrs_command::ArgumentHandle;
use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_command_argument_entity::{
    Candidate, EntityArgument, EntityBounds, SelectorKind, Target,
};
use mcrs_minecraft_command_engine::{CommandFailure, Invocation, PlayerEntry};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::{PLAYER_HEIGHT, PLAYER_WIDTH};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

/// The entities a selector found, which a handler keeps between its runs so that a search
/// allocates only when it finds more entities than every search before it.
#[derive(Default)]
pub(crate) struct FoundPlayers {
    found: Vec<&'static PlayerEntry>,
    mobs: Vec<MobTarget>,
    candidates: Vec<EntityCandidate<'static>>,
    picked: Vec<&'static EntityCandidate<'static>>,
}

impl FoundPlayers {
    /// Lends the buffers to `search` and takes them back on every path out of it.
    pub(crate) fn with<'c, R>(&mut self, search: impl FnOnce(&mut Selection<'c>) -> R) -> R {
        let mut selection = Selection {
            found: mcrs_command::recycle(core::mem::take(&mut self.found)),
            mobs: core::mem::take(&mut self.mobs),
            candidates: mcrs_command::recycle(core::mem::take(&mut self.candidates)),
            picked: core::mem::take(&mut self.picked),
        };
        let result = search(&mut selection);
        self.found = mcrs_command::recycle(selection.found);
        self.mobs = selection.mobs;
        self.candidates = mcrs_command::recycle(selection.candidates);
        self.picked = selection.picked;
        result
    }
}

/// The entities a search found, players and mobs apart, and the buffers the search reuses.
pub(crate) struct Selection<'c> {
    pub(crate) found: Vec<&'c PlayerEntry>,
    /// The mobs of this world, in the order of the selector.
    pub(crate) mobs: Vec<MobTarget>,
    candidates: Vec<EntityCandidate<'c>>,
    picked: Vec<&'static EntityCandidate<'static>>,
}

impl Selection<'_> {
    /// The number of players and mobs found.
    pub(crate) fn len(&self) -> usize {
        self.found.len().saturating_add(self.mobs.len())
    }
}

/// The entities a selector argument finds: the players in every dimension and the mobs of this
/// world, in `out`, or the failure of its search.
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
    let dimension = targets
        .dimension_of(call.source.dimension)
        .unwrap_or_default();
    let me = match (call.source.player, call.source.mob) {
        (Some(session), _) => targets.player(session).map(EntityCandidate::Player),
        (None, Some(mob)) => context
            .lookup()
            .mob(mob)
            .map(|view| EntityCandidate::Mob { view, dimension }),
        (None, None) => None,
    };
    let mut random = |bound: usize| rand::random_range(0..bound.max(1));
    let mut candidates = mcrs_command::recycle(core::mem::take(&mut out.candidates));
    match selector.target {
        Target::Kind(SelectorKind::SelfEntity) => {}
        Target::Uuid(uuid) => {
            candidates.extend(targets.players().iter().map(EntityCandidate::Player));
            candidates.extend(
                context
                    .lookup()
                    .mob_by_uuid(uuid.as_u128())
                    .map(|view| EntityCandidate::Mob { view, dimension }),
            );
        }
        Target::Name(_) => {
            candidates.extend(targets.players().iter().map(EntityCandidate::Player));
        }
        Target::Kind(_) => {
            candidates.extend(targets.players().iter().map(EntityCandidate::Player));
            if selector.includes_entities() {
                candidates.extend(
                    context
                        .lookup()
                        .mobs()
                        .map(|view| EntityCandidate::Mob { view, dimension }),
                );
            }
        }
    }
    let mut picked = mcrs_command::recycle(core::mem::take(&mut out.picked));
    let selected = selector.select(
        &source,
        me.as_ref(),
        candidates.iter(),
        &mut random,
        &mut picked,
    );
    out.found.clear();
    out.mobs.clear();
    for candidate in &picked {
        match candidate {
            EntityCandidate::Player(player) => out.found.push(player),
            EntityCandidate::Mob { view, .. } => out.mobs.push(view.target()),
        }
    }
    out.picked = mcrs_command::recycle(picked);
    candidates.clear();
    out.candidates = candidates;
    selected.map_err(|error| CommandFailure(error.text(context.line())))?;
    Ok(true)
}

/// What a branch without a selector needs of its source, and the failure when the source is
/// not that.
#[derive(Clone, Copy)]
pub(crate) enum SelfSource {
    Player,
    Entity,
}

impl SelfSource {
    fn require<P>(
        self,
        entity: Option<PlayerSession>,
        player: Option<P>,
    ) -> Result<(), CommandFailure> {
        let (satisfied, key) = match self {
            Self::Player => (player.is_some(), "permissions.requires.player"),
            Self::Entity => (entity.is_some(), "permissions.requires.entity"),
        };
        if satisfied {
            Ok(())
        } else {
            Err(CommandFailure::translate(key, vec![]))
        }
    }
}

/// The players a selector finds, or the source player when the branch has no selector, in
/// `out`. A branch without a selector fails when its source is not what `needs` says.
pub(crate) fn targets_or_self<'c>(
    context: &'c VanillaContext,
    call: &Invocation,
    arg: ArgumentHandle<EntityArgument>,
    none_found: &'static str,
    needs: SelfSource,
    out: &mut Selection<'c>,
) -> Result<(), CommandFailure> {
    if !select_if_present(context, call, arg, out)? {
        out.found.clear();
        out.mobs.clear();
        let source = call.source.player;
        out.found
            .extend(source.and_then(|session| context.targets().player(session)));
        if let Some(mob) = call.source.mob.and_then(|mob| context.lookup().mob(mob)) {
            out.mobs.push(mob.target());
            return Ok(());
        }
        return needs.require(source, out.found.first());
    }
    if out.len() == 0 {
        return Err(CommandFailure::translate(none_found, vec![]));
    }
    Ok(())
}

/// An entity as a selector sees it: a player of any dimension, or a mob of this world. The engine
/// that owns [`PlayerEntry`] does not know the selector crate, so the candidate is a view.
#[derive(Clone, Copy)]
enum EntityCandidate<'c> {
    Player(&'c PlayerEntry),
    Mob {
        view: MobView<'c>,
        /// The position of this world in the dimension order.
        dimension: usize,
    },
}

impl Candidate for EntityCandidate<'_> {
    fn uuid(&self) -> Uuid {
        match self {
            Self::Player(player) => player.uuid,
            Self::Mob { view, .. } => view.uuid(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Player(player) => &player.name,
            Self::Mob { view, .. } => view.mob.custom_name.as_deref().unwrap_or_default(),
        }
    }

    fn entity_type(&self) -> EntityType {
        match self {
            Self::Player(_) => EntityType::Player,
            Self::Mob { view, .. } => view.entity_type(),
        }
    }

    fn position(&self) -> DVec3 {
        match self {
            Self::Player(player) => player.transform.translation,
            Self::Mob { view, .. } => view.state.body.position,
        }
    }

    fn bounding_box(&self) -> EntityBounds {
        let (position, width, height) = match self {
            Self::Player(player) => (player.transform.translation, PLAYER_WIDTH, PLAYER_HEIGHT),
            Self::Mob { view, .. } => (
                view.state.body.position,
                f64::from(view.mob.kind.width),
                f64::from(view.mob.kind.height),
            ),
        };
        let half = DVec3::new(width / 2.0, 0.0, width / 2.0);
        EntityBounds {
            min: position - half,
            max: position + half + DVec3::Y * height,
        }
    }

    fn yaw(&self) -> f32 {
        match self {
            Self::Player(player) => player.transform.rotation.yaw(),
            Self::Mob { view, .. } => view.target().yaw,
        }
    }

    fn pitch(&self) -> f32 {
        match self {
            Self::Player(player) => player.transform.rotation.pitch(),
            Self::Mob { view, .. } => view.target().pitch,
        }
    }

    fn game_mode(&self) -> Option<GameMode> {
        match self {
            Self::Player(player) => Some(player.mode),
            Self::Mob { .. } => None,
        }
    }

    // chisle: experience, player tags and teams have no state in the server yet: every player is
    // at level 0, has no tag and no team. Their components lift it.
    fn experience_level(&self) -> Option<i32> {
        match self {
            Self::Player(_) => Some(0),
            Self::Mob { .. } => None,
        }
    }

    fn alive(&self) -> bool {
        match self {
            Self::Player(_) => true,
            Self::Mob { view, .. } => view.alive(),
        }
    }

    fn has_tag(&self, tag: &str) -> bool {
        match self {
            Self::Player(_) => false,
            Self::Mob { view, .. } => view.mob.tags.iter().any(|held| held == tag),
        }
    }

    fn has_any_tag(&self) -> bool {
        match self {
            Self::Player(_) => false,
            Self::Mob { view, .. } => !view.mob.tags.is_empty(),
        }
    }

    fn team(&self) -> Option<&str> {
        None
    }

    fn dimension(&self) -> usize {
        match self {
            Self::Player(player) => player.dimension,
            Self::Mob { dimension, .. } => *dimension,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selectorless_branch_fails_with_what_its_source_lacks() {
        let entity = Some(PlayerSession(1));
        let cases = [
            (
                SelfSource::Player,
                None,
                None,
                Some("permissions.requires.player"),
            ),
            (
                SelfSource::Player,
                entity,
                None,
                Some("permissions.requires.player"),
            ),
            (SelfSource::Player, entity, Some(()), None),
            (
                SelfSource::Entity,
                None,
                None,
                Some("permissions.requires.entity"),
            ),
            (SelfSource::Entity, entity, None, None),
        ];
        for (needs, entity, player, key) in cases {
            let expected = key.map(|key| CommandFailure::translate(key, vec![]));
            assert_eq!(needs.require(entity, player).err(), expected);
        }
    }
}
