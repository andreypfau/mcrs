use crate::error::EntityError;
use crate::list_option::ListOption;
use crate::options::read_options;
use crate::selector::{Order, Selector, SelectorKind, State, Target};
use glam::DVec3;
use mcrs_command::Span;
use mcrs_command::{Reader, eq_ignore_case};
use mcrs_minecraft_command::{Lookup, ResourceError, Source};
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_entity::keys::{ENTITY_TYPE, EntityType};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_loot::keys::PREDICATE;
use mcrs_minecraft_value_provider::bounds::MinMaxBounds;
use uuid::Uuid;

/// The minimum and the maximum corner of the bounding box of an entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityBounds {
    pub min: DVec3,
    pub max: DVec3,
}

/// An entity as the search sees it.
pub trait Candidate {
    fn uuid(&self) -> Uuid;
    /// The plain text of its name; a player's name for a player.
    fn name(&self) -> &str;
    fn entity_type(&self) -> EntityType;
    fn position(&self) -> DVec3;
    fn bounding_box(&self) -> EntityBounds;
    fn yaw(&self) -> f32;
    fn pitch(&self) -> f32;
    fn game_mode(&self) -> Option<GameMode>;
    fn experience_level(&self) -> Option<i32>;
    fn alive(&self) -> bool;
    fn has_tag(&self, tag: &str) -> bool;
    fn has_any_tag(&self) -> bool;
    fn team(&self) -> Option<&str>;
    /// The position of its dimension in the dimension order.
    fn dimension(&self) -> usize;

    fn is_player(&self) -> bool {
        self.entity_type() == EntityType::Player
    }
}

impl<'a> Selector<'a> {
    /// Each option that holds one of several values, in the order of the text.
    fn list_options(&self, mut each: impl FnMut(ListOption<'a>)) {
        let Some(at) = self.options else {
            return;
        };
        let mut reader = Reader::at(self.input, at);
        let mut state = State::new(self.input);
        state.check_duplicates = false;
        let _ = read_options(&mut reader, &mut state, &mut each);
    }

    fn in_box<C: Candidate>(&self, candidate: &C, origin: DVec3) -> bool {
        if self.size.iter().all(Option::is_none) {
            return true;
        }
        let size = DVec3::from(self.size.map(|axis| axis.unwrap_or(0.0)));
        let low = origin + size.min(DVec3::ZERO);
        let high = origin + size.max(DVec3::ZERO) + DVec3::ONE;
        let EntityBounds { min, max } = candidate.bounding_box();
        if min == max {
            let position = candidate.position();
            return position.cmpge(low).all() && position.cmple(high).all();
        }
        min.cmplt(high).all() && max.cmpgt(low).all()
    }

    /// Reads the options once and keeps the entities of `found` that pass each one.
    fn retain_listed<C: Candidate, L: Lookup>(&self, found: &mut Vec<&C>, lookup: &L) {
        self.list_options(|option| {
            found.retain(|candidate| match option {
                ListOption::Name { name, negated } => (name == candidate.name()) != negated,
                ListOption::GameMode { mode, negated } => candidate
                    .game_mode()
                    .is_some_and(|actual| (actual == mode) != negated),
                ListOption::Team { team, negated } => {
                    (candidate.team().unwrap_or_default() == team) != negated
                }
                ListOption::Type {
                    entity_type,
                    negated,
                } => (candidate.entity_type() == entity_type) != negated,
                ListOption::TypeTag { tag, negated } => {
                    lookup.tag_contains(
                        ENTITY_TYPE.location(),
                        tag,
                        candidate.entity_type().location(),
                    ) != negated
                }
                ListOption::Tag { tag, negated } => {
                    let has = if tag.is_empty() {
                        !candidate.has_any_tag()
                    } else {
                        candidate.has_tag(tag)
                    };
                    has != negated
                }
                // No scoreboard and no advancements exist, so an objective or an
                // advancement that the map names never does either.
                ListOption::Scores { empty } | ListOption::Advancements { empty } => empty,
                // chisle: no saved data of an entity reaches a candidate yet, so the data of each
                // one counts as not containing the compound; a saved-data view lifts it.
                ListOption::Nbt { negated } => negated,
                // chisle: a loot predicate is not evaluated against an entity yet, so one that
                // exists selects nothing; evaluating loot conditions here lifts it.
                ListOption::Predicate { .. } => false,
            });
        });
    }

    fn passes<C: Candidate>(&self, candidate: &C, origin: DVec3) -> bool {
        if self.alive && !candidate.alive() {
            return false;
        }
        if self
            .type_limit
            .is_some_and(|limit| candidate.entity_type() != limit)
        {
            return false;
        }
        if let Some(level) = &self.level
            && !candidate
                .experience_level()
                .is_some_and(|experience| level.matches(experience))
        {
            return false;
        }
        let pitch = wrap_degrees(candidate.pitch());
        if let Some(MinMaxBounds { min, max }) = self.x_rotation {
            let min = min.map(wrap_degrees);
            let max = max.map(wrap_degrees);
            let inside = match (min, max) {
                (Some(min), Some(max)) if min > max => pitch >= min || pitch <= max,
                _ => min.is_none_or(|m| pitch >= m) && max.is_none_or(|m| pitch <= m),
            };
            if !inside {
                return false;
            }
        }
        if let Some(MinMaxBounds { min, max }) = self.y_rotation {
            let yaw = candidate.yaw().rem_euclid(360.0);
            let min = min.map_or(0.0, |m| m.rem_euclid(360.0));
            let inside = match max.map(|m| m.rem_euclid(360.0)) {
                None => yaw >= min,
                Some(max) if min <= max => yaw >= min && yaw <= max,
                Some(max) => yaw >= min || yaw <= max,
            };
            if !inside {
                return false;
            }
        }
        self.in_box(candidate, origin)
            && self
                .distance
                .as_ref()
                .is_none_or(|distance| distance.matches(origin.distance(candidate.position())))
    }

    fn missing_predicate<L: Lookup>(&self, lookup: &L) -> Option<Span> {
        let mut missing = None;
        self.list_options(|option| {
            if let ListOption::Predicate { id, span } = option
                && missing.is_none()
                && !lookup.has_entry(PREDICATE.location(), id)
            {
                missing = Some(span);
            }
        });
        missing
    }

    /// The entities the selector finds. `candidates` come in the order of the player list; `random`
    /// gives an index
    /// below its argument.
    pub fn select<'c, C: Candidate, L: Lookup>(
        &self,
        source: &Source<'_, L>,
        source_entity: Option<&'c C>,
        candidates: impl IntoIterator<Item = &'c C>,
        random: &mut impl FnMut(usize) -> usize,
        out: &mut Vec<&'c C>,
    ) -> Result<(), EntityError> {
        out.clear();
        if matches!(self.target, Target::Kind(_)) && !source.level.can_use_selectors() {
            return Err(EntityError::SelectorNotAllowed);
        }
        if let Some(span) = self.missing_predicate(source.lookup) {
            let registry = PREDICATE.location();
            return Err(EntityError::Resource(ResourceError::ResourceNotFound {
                span,
                registry,
            }));
        }
        let candidates = candidates.into_iter();
        let [x, y, z] = self.origin;
        let origin = DVec3::new(
            x.unwrap_or(source.position.x),
            y.unwrap_or(source.position.y),
            z.unwrap_or(source.position.z),
        );
        match self.target {
            Target::Name(name) => {
                out.extend(
                    candidates
                        .filter(|c| c.is_player())
                        .find(|c| eq_ignore_case(name.chars(), c.name().chars())),
                );
                return Ok(());
            }
            Target::Uuid(uuid) => {
                out.extend(
                    candidates
                        .filter(|c| self.includes_entities || c.is_player())
                        .find(|c| c.uuid() == uuid),
                );
                return Ok(());
            }
            Target::Kind(SelectorKind::SelfEntity) => {
                out.extend(source_entity.filter(|entity| {
                    (self.includes_entities || entity.is_player()) && self.passes(*entity, origin)
                }));
                self.retain_listed(out, source.lookup);
                return Ok(());
            }
            Target::Kind(_) => {}
        }
        out.extend(candidates.filter(|c| {
            (self.includes_entities || c.is_player())
                && (!self.world_limited || c.dimension() == source.dimension)
                && self.passes(*c, origin)
        }));
        self.retain_listed(out, source.lookup);
        if self.includes_entities && !self.world_limited {
            out.sort_by_key(|c| c.dimension());
        }
        let distance = |c: &&C| origin.distance_squared(c.position());
        match self.order {
            Order::Arbitrary => {}
            Order::Nearest => out.sort_by(|a, b| distance(a).total_cmp(&distance(b))),
            Order::Furthest => out.sort_by(|a, b| distance(b).total_cmp(&distance(a))),
            Order::Random => {
                for index in (1..out.len()).rev() {
                    out.swap(index, random(index + 1).min(index));
                }
            }
        }
        if let Some(max) = self.max_results {
            out.truncate(max as usize);
        }
        Ok(())
    }
}
