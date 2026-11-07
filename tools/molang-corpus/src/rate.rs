use std::collections::BTreeMap;

/// The rate classes of `docs/molang.md`, section 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RateClass {
    Constant,
    Load,
    Frame,
    EntityTick,
    EntityFrame,
    AnimationFrame,
}

impl RateClass {
    pub const ALL: [RateClass; 6] = [
        RateClass::Constant,
        RateClass::Load,
        RateClass::Frame,
        RateClass::EntityTick,
        RateClass::EntityFrame,
        RateClass::AnimationFrame,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RateClass::Constant => "Constant",
            RateClass::Load => "Load",
            RateClass::Frame => "Frame",
            RateClass::EntityTick => "EntityTick",
            RateClass::EntityFrame => "EntityFrame",
            RateClass::AnimationFrame => "AnimationFrame",
        }
    }

    fn from_spec(name: &str) -> Option<RateClass> {
        Some(match name {
            "load" => RateClass::Load,
            "frame" => RateClass::Frame,
            "entity_tick" => RateClass::EntityTick,
            "entity_frame" => RateClass::EntityFrame,
            "animation_frame" => RateClass::AnimationFrame,
            _ => return None,
        })
    }
}

/// A join of rate classes: the set of axes along which a value can change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rate(u8);

impl Rate {
    const LOAD: u8 = 1;
    const FRAME: u8 = 2;
    const ENTITY: u8 = 4;
    const ANIMATION: u8 = 8;

    pub const CONSTANT: Rate = Rate(0);

    pub fn of(class: RateClass) -> Rate {
        Rate(match class {
            RateClass::Constant => 0,
            RateClass::Load => Self::LOAD,
            RateClass::Frame => Self::FRAME,
            RateClass::EntityTick => Self::ENTITY,
            RateClass::EntityFrame => Self::ENTITY | Self::FRAME,
            RateClass::AnimationFrame => Self::ENTITY | Self::FRAME | Self::ANIMATION,
        })
    }

    #[must_use]
    pub fn join(self, other: Rate) -> Rate {
        Rate(self.0 | other.0)
    }

    pub fn class(self) -> RateClass {
        if self.0 & Self::ANIMATION != 0 {
            RateClass::AnimationFrame
        } else if self.0 & (Self::ENTITY | Self::FRAME) == Self::ENTITY | Self::FRAME {
            RateClass::EntityFrame
        } else if self.0 & Self::FRAME != 0 {
            RateClass::Frame
        } else if self.0 & Self::ENTITY != 0 {
            RateClass::EntityTick
        } else if self.0 & Self::LOAD != 0 {
            RateClass::Load
        } else {
            RateClass::Constant
        }
    }

    /// A value that a run stores once per subject, for example in `initialize`.
    #[must_use]
    pub fn frozen(self) -> Rate {
        if self == Rate::CONSTANT || self == Rate(Self::LOAD) {
            self
        } else {
            Rate(Self::ENTITY)
        }
    }
}

/// The rate of a query: from the MOLQ tables, or a guess from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryRate {
    Spec(Rate),
    /// A function of its arguments only.
    Pure,
    Guess(Rate),
}

impl QueryRate {
    pub fn rate(self) -> Rate {
        match self {
            QueryRate::Spec(rate) | QueryRate::Guess(rate) => rate,
            QueryRate::Pure => Rate::CONSTANT,
        }
    }
}

/// The queries that `docs/spec/molang-queries.md` defines, with their rate classes.
#[derive(Clone, Debug, Default)]
pub struct SpecQueries {
    pub rates: BTreeMap<String, QueryRate>,
}

impl SpecQueries {
    /// Reads the query tables. A table row names a query in its `Name` column; a two-column
    /// table names one query in its `name` row.
    pub fn parse(markdown: &str) -> SpecQueries {
        let mut rates = BTreeMap::new();
        let mut columns: Option<Columns> = None;
        let mut pending_name: Option<String> = None;
        for line in markdown.lines() {
            let cells: Vec<&str> = line.trim().split('|').map(str::trim).collect();
            if !line.trim_start().starts_with('|') || cells.len() < 3 {
                columns = None;
                continue;
            }
            let row = cells.get(1..cells.len() - 1).unwrap_or_default();
            if let Some(found) = Columns::find(row) {
                columns = Some(found);
                continue;
            }
            match (row.first().copied(), row.get(1).copied()) {
                (Some("name"), Some(name)) => pending_name = Some(unquote(name).to_owned()),
                (Some("rate class"), Some(rate)) => {
                    if let (Some(name), Some(class)) =
                        (pending_name.take(), RateClass::from_spec(unquote(rate)))
                    {
                        rates.insert(name, QueryRate::Spec(Rate::of(class)));
                    }
                }
                _ => {}
            }
            let Some(columns) = columns else {
                continue;
            };
            let (Some(name), Some(rate)) = (row.get(columns.name), row.get(columns.rate)) else {
                continue;
            };
            let name = unquote(name);
            if name.is_empty() || name.starts_with('-') {
                continue;
            }
            let rate = match RateClass::from_spec(unquote(rate)) {
                Some(class) => QueryRate::Spec(Rate::of(class)),
                None if rate.contains("arguments") => QueryRate::Pure,
                None => continue,
            };
            rates.insert(name.to_owned(), rate);
        }
        SpecQueries { rates }
    }

    pub fn rate(&self, name: &str) -> QueryRate {
        self.rates
            .get(name)
            .copied()
            .unwrap_or_else(|| QueryRate::Guess(guess(name)))
    }

    pub fn defines(&self, name: &str) -> bool {
        self.rates.contains_key(name)
    }
}

#[derive(Clone, Copy)]
struct Columns {
    name: usize,
    rate: usize,
}

impl Columns {
    fn find(row: &[&str]) -> Option<Columns> {
        let name = row.iter().position(|cell| *cell == "Name")?;
        let rate = row.iter().position(|cell| *cell == "Rate class")?;
        Some(Columns { name, rate })
    }
}

fn unquote(cell: &str) -> &str {
    cell.trim_matches('`')
}

/// A guess for a query that the MOLQ tables do not define. The words of the name select the
/// class; each other query is per-entity tick state.
fn guess(name: &str) -> Rate {
    const ANIMATION: [&str; 3] = ["anim_time", "key_frame", "animation"];
    const FRAME: [&str; 6] = [
        "time_of_day",
        "frame_alpha",
        "camera",
        "is_first_person",
        "moon",
        "client_",
    ];
    const ENTITY_FRAME: [&str; 15] = [
        "rotation",
        "life_time",
        "delta_time",
        "distance",
        "modified_",
        "walk",
        "swing",
        "position",
        "offset",
        "look_at",
        "speed",
        "time_stamp",
        "_time",
        "lerp",
        "yaw",
    ];
    let has = |words: &[&str]| words.iter().any(|word| name.contains(word));
    if has(&ANIMATION) {
        Rate::of(RateClass::AnimationFrame)
    } else if has(&FRAME) {
        Rate::of(RateClass::Frame)
    } else if has(&ENTITY_FRAME) {
        Rate::of(RateClass::EntityFrame)
    } else {
        Rate::of(RateClass::EntityTick)
    }
}
