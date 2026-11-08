use mcrs_minecraft_physics::Birth;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::SourceKind;
use serde::{Deserialize, Serialize, de};

/// The compound key of the birth of an entity. It is data of mcrs: the vanilla server ignores it.
pub(crate) const BIRTH: &str = "mcrs:birth";

/// The saved form of a [`Birth`]: NBT has no unsigned integers, so each value keeps its bits in
/// a signed one.
#[derive(Serialize, Deserialize)]
pub(crate) struct SavedBirth {
    tick: i64,
    kind: i8,
    source_most: i64,
    source_least: i64,
    sequence: i32,
    phase: i8,
}

impl From<&Birth> for SavedBirth {
    fn from(birth: &Birth) -> Self {
        let key = birth.key;
        Self {
            tick: key.tick().cast_signed(),
            kind: key.kind() as i8,
            source_most: ((key.source() >> 64) as u64).cast_signed(),
            source_least: (key.source() as u64).cast_signed(),
            sequence: key.sequence().cast_signed(),
            phase: birth.phase.cast_signed(),
        }
    }
}

impl SavedBirth {
    pub(crate) fn to_birth<E: de::Error>(&self) -> Result<Birth, E> {
        let kind = usize::try_from(self.kind)
            .ok()
            .and_then(|number| KINDS.get(number).copied())
            .ok_or_else(|| E::custom(format!("unknown birth source kind {}", self.kind)))?;
        let source = u128::from(self.source_most.cast_unsigned()) << 64
            | u128::from(self.source_least.cast_unsigned());
        Ok(Birth {
            key: BirthKey::new(
                self.tick.cast_unsigned(),
                kind,
                source,
                self.sequence.cast_unsigned(),
            ),
            phase: self.phase.cast_unsigned(),
        })
    }
}

/// The kinds in the order of their saved number.
const KINDS: [SourceKind; 6] = [
    SourceKind::Load,
    SourceKind::Player,
    SourceKind::Command,
    SourceKind::Spawn,
    SourceKind::Block,
    SourceKind::Entity,
];
