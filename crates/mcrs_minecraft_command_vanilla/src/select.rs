use crate::entity_view::EntityView;
use crate::mob_view::MobView;
use crate::set::VanillaContext;
use mcrs_command::ArgumentHandle;
use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_command_argument_entity::{EntityArgument, SelectorKind, Target};
use mcrs_minecraft_command_engine::{CommandFailure, Invocation, PlayerEntry};
use mcrs_minecraft_engine_world::session::PlayerSession;

/// The buffers of a selector search, which a handler keeps between its runs so that a search
/// allocates only when it finds more entities than every search before it.
#[derive(Default)]
pub(crate) struct SelectionBuffers {
    found: Vec<EntityView<'static>>,
    candidates: Vec<EntityView<'static>>,
    picked: Vec<&'static EntityView<'static>>,
}

impl SelectionBuffers {
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

/// The players and the mobs a search found, in the order of the selector, and the buffers the
/// search reuses.
pub(crate) struct Selection<'c> {
    found: Vec<EntityView<'c>>,
    candidates: Vec<EntityView<'c>>,
    picked: Vec<&'static EntityView<'static>>,
}

impl<'c> Selection<'c> {
    /// The number of players and mobs found.
    pub(crate) fn len(&self) -> usize {
        self.found.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.found.is_empty()
    }

    /// The entities found, in the order of the selector.
    pub(crate) fn iter(&self) -> impl Iterator<Item = EntityView<'c>> + '_ {
        self.found.iter().copied()
    }

    pub(crate) fn first(&self) -> Option<EntityView<'c>> {
        self.found.first().copied()
    }

    /// The players found, in the order of the selector.
    pub(crate) fn players(&self) -> impl Iterator<Item = &'c PlayerEntry> + '_ {
        self.found.iter().filter_map(|found| match found {
            EntityView::Player(player) => Some(*player),
            EntityView::Mob { .. } => None,
        })
    }

    /// The mobs found, in the order of the selector.
    pub(crate) fn mobs(&self) -> impl Iterator<Item = MobView<'c>> + '_ {
        self.found.iter().filter_map(|found| match found {
            EntityView::Player(_) => None,
            EntityView::Mob { view, .. } => Some(*view),
        })
    }

    /// The first entity found when it is a mob; `command.failed` otherwise, as for a target that
    /// a mob-only command cannot handle.
    pub(crate) fn single_mob(&self) -> Result<MobView<'c>, CommandFailure> {
        match self.first() {
            Some(EntityView::Mob { view, .. }) => Ok(view),
            _ => Err(CommandFailure::failed()),
        }
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
        (Some(session), _) => targets.player(session).map(EntityView::Player),
        (None, Some(mob)) => context
            .lookup()
            .mob(mob)
            .map(|view| EntityView::Mob { view, dimension }),
        (None, None) => None,
    };
    let mut random = |bound: usize| rand::random_range(0..bound.max(1));
    let mut candidates = mcrs_command::recycle(core::mem::take(&mut out.candidates));
    match selector.target {
        Target::Kind(SelectorKind::SelfEntity) => {}
        Target::Uuid(uuid) => {
            candidates.extend(targets.players().iter().map(EntityView::Player));
            candidates.extend(
                context
                    .lookup()
                    .mob_by_uuid(uuid.as_u128())
                    .map(|view| EntityView::Mob { view, dimension }),
            );
        }
        Target::Name(_) => {
            candidates.extend(targets.players().iter().map(EntityView::Player));
        }
        Target::Kind(_) => {
            candidates.extend(targets.players().iter().map(EntityView::Player));
            if selector.includes_entities() {
                candidates.extend(
                    context
                        .lookup()
                        .mobs()
                        .map(|view| EntityView::Mob { view, dimension }),
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
    out.found.extend(picked.iter().map(|&&candidate| candidate));
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
    fn require(
        self,
        session: Option<PlayerSession>,
        player: Option<&PlayerEntry>,
    ) -> Result<(), CommandFailure> {
        let (satisfied, key) = match self {
            Self::Player => (player.is_some(), "permissions.requires.player"),
            Self::Entity => (session.is_some(), "permissions.requires.entity"),
        };
        if satisfied {
            Ok(())
        } else {
            Err(CommandFailure::translate(key, vec![]))
        }
    }
}

/// The entities a selector finds, or the source entity when the branch has no selector, in
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
        let session = call.source.player;
        let player = session.and_then(|session| context.targets().player(session));
        out.found.extend(player.map(EntityView::Player));
        if let Some(view) = call.source.mob.and_then(|mob| context.lookup().mob(mob)) {
            let dimension = context
                .targets()
                .dimension_of(call.source.dimension)
                .unwrap_or_default();
            out.found.push(EntityView::Mob { view, dimension });
            return Ok(());
        }
        return needs.require(session, player);
    }
    if out.is_empty() {
        return Err(CommandFailure::translate(none_found, vec![]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::DVec3;
    use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_protocol::uuid::Uuid;

    #[test]
    fn a_selectorless_branch_fails_with_what_its_source_lacks() {
        let session = Some(PlayerSession(1));
        let entry = PlayerEntry {
            session: PlayerSession(1),
            dimension: 0,
            entity: None,
            uuid: Uuid::nil(),
            name: String::from("Steve"),
            transform: Transform {
                translation: DVec3::ZERO,
                rotation: Rotation::new(0.0, 0.0),
            },
            mode: GameMode::Survival,
        };
        let player = Some(&entry);
        let cases = [
            (
                SelfSource::Player,
                None,
                None,
                Some("permissions.requires.player"),
            ),
            (
                SelfSource::Player,
                session,
                None,
                Some("permissions.requires.player"),
            ),
            (SelfSource::Player, session, player, None),
            (
                SelfSource::Entity,
                None,
                None,
                Some("permissions.requires.entity"),
            ),
            (SelfSource::Entity, session, None, None),
        ];
        for (needs, session, player, key) in cases {
            let expected = key.map(|key| CommandFailure::translate(key, vec![]));
            assert_eq!(needs.require(session, player).err(), expected);
        }
    }
}
