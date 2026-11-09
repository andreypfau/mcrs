use std::collections::BTreeMap;

use mcrs_molang::Rate;

pub const RATES: [Rate; 6] = [
    Rate::Constant,
    Rate::Load,
    Rate::Frame,
    Rate::EntityTick,
    Rate::EntityFrame,
    Rate::AnimationFrame,
];

fn from_spec(name: &str) -> Option<Rate> {
    Some(match name {
        "load" => Rate::Load,
        "frame" => Rate::Frame,
        "entity_tick" => Rate::EntityTick,
        "entity_frame" => Rate::EntityFrame,
        "animation_frame" => Rate::AnimationFrame,
        _ => return None,
    })
}

/// A join of rates: the set of axes along which a value can change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Axes(u8);

impl Axes {
    const LOAD: u8 = 1;
    const FRAME: u8 = 2;
    const ENTITY: u8 = 4;
    const ANIMATION: u8 = 8;

    pub const CONSTANT: Axes = Axes(0);

    pub fn of(class: Rate) -> Axes {
        Axes(match class {
            Rate::Constant => 0,
            Rate::Load => Self::LOAD,
            Rate::Frame => Self::FRAME,
            Rate::EntityTick => Self::ENTITY,
            Rate::EntityFrame => Self::ENTITY | Self::FRAME,
            Rate::AnimationFrame => Self::ENTITY | Self::FRAME | Self::ANIMATION,
        })
    }

    #[must_use]
    pub fn join(self, other: Axes) -> Axes {
        Axes(self.0 | other.0)
    }

    pub fn class(self) -> Rate {
        if self.0 & Self::ANIMATION != 0 {
            Rate::AnimationFrame
        } else if self.0 & (Self::ENTITY | Self::FRAME) == Self::ENTITY | Self::FRAME {
            Rate::EntityFrame
        } else if self.0 & Self::FRAME != 0 {
            Rate::Frame
        } else if self.0 & Self::ENTITY != 0 {
            Rate::EntityTick
        } else if self.0 & Self::LOAD != 0 {
            Rate::Load
        } else {
            Rate::Constant
        }
    }

    /// A value that a run stores once per subject, for example in `initialize`.
    #[must_use]
    pub fn frozen(self) -> Axes {
        if self == Axes::CONSTANT || self == Axes(Self::LOAD) {
            self
        } else {
            Axes(Self::ENTITY)
        }
    }
}

/// The rate of a query: from the MOLQ tables, or a guess from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryRate {
    Spec(Axes),
    /// A function of its arguments only.
    Pure,
    Guess(Axes),
}

impl QueryRate {
    pub fn rate(self) -> Axes {
        match self {
            QueryRate::Spec(rate) | QueryRate::Guess(rate) => rate,
            QueryRate::Pure => Axes::CONSTANT,
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
                        (pending_name.take(), from_spec(unquote(rate)))
                    {
                        rates.insert(name, QueryRate::Spec(Axes::of(class)));
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
            let rate = match from_spec(unquote(rate)) {
                Some(class) => QueryRate::Spec(Axes::of(class)),
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
fn guess(name: &str) -> Axes {
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
        Axes::of(Rate::AnimationFrame)
    } else if has(&FRAME) {
        Axes::of(Rate::Frame)
    } else if has(&ENTITY_FRAME) {
        Axes::of(Rate::EntityFrame)
    } else {
        Axes::of(Rate::EntityTick)
    }
}
