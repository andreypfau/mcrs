use crate::keys::{VillagerProfession, VillagerType};
use serde::{Deserialize, Serialize};

pub const MIN_VILLAGER_LEVEL: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Fields")]
pub struct VillagerData {
    #[serde(rename = "type")]
    pub kind: VillagerType,
    pub profession: VillagerProfession,
    level: i32,
}

impl VillagerData {
    pub const fn new(kind: VillagerType, profession: VillagerProfession, level: i32) -> Self {
        VillagerData {
            kind,
            profession,
            level: if level < MIN_VILLAGER_LEVEL {
                MIN_VILLAGER_LEVEL
            } else {
                level
            },
        }
    }

    pub const fn level(&self) -> i32 {
        self.level
    }
}

impl Default for VillagerData {
    fn default() -> Self {
        VillagerData::new(
            VillagerType::Plains,
            VillagerProfession::None,
            MIN_VILLAGER_LEVEL,
        )
    }
}

#[derive(Deserialize)]
struct Fields {
    #[serde(rename = "type")]
    kind: Option<VillagerType>,
    profession: Option<VillagerProfession>,
    level: Option<i32>,
}

impl From<Fields> for VillagerData {
    fn from(fields: Fields) -> Self {
        let default = VillagerData::default();
        VillagerData::new(
            fields.kind.unwrap_or(default.kind),
            fields.profession.unwrap_or(default.profession),
            fields.level.unwrap_or(default.level),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_nbt::compound::NbtCompound;
    use mcrs_minecraft_nbt::tag::NbtTag;
    use mcrs_minecraft_nbt::{from_tag, to_nbt_tag};

    fn compound(fields: &[(&str, NbtTag)]) -> NbtTag {
        let mut compound = NbtCompound::default();
        for (name, tag) in fields {
            compound.put(name, tag.clone());
        }
        NbtTag::Compound(compound)
    }

    fn text(value: &str) -> NbtTag {
        NbtTag::String(value.to_owned())
    }

    #[test]
    fn the_villager_data_reads_and_writes_one_shape() {
        use VillagerProfession::{Librarian, None as Unemployed};
        use VillagerType::{Plains, Snow};

        let cases: [(&str, NbtTag, Result<VillagerData, ()>); 9] = [
            (
                "all three fields",
                compound(&[
                    ("type", text("minecraft:snow")),
                    ("profession", text("minecraft:librarian")),
                    ("level", NbtTag::Int(3)),
                ]),
                Ok(VillagerData::new(Snow, Librarian, 3)),
            ),
            (
                "no fields",
                compound(&[]),
                Ok(VillagerData::new(Plains, Unemployed, 1)),
            ),
            (
                "no type",
                compound(&[
                    ("profession", text("minecraft:librarian")),
                    ("level", NbtTag::Int(2)),
                ]),
                Ok(VillagerData::new(Plains, Librarian, 2)),
            ),
            (
                "no profession",
                compound(&[("type", text("minecraft:snow")), ("level", NbtTag::Int(2))]),
                Ok(VillagerData::new(Snow, Unemployed, 2)),
            ),
            (
                "no level",
                compound(&[
                    ("type", text("minecraft:snow")),
                    ("profession", text("minecraft:librarian")),
                ]),
                Ok(VillagerData::new(Snow, Librarian, 1)),
            ),
            (
                "a level below the first is raised to it",
                compound(&[("level", NbtTag::Int(-4))]),
                Ok(VillagerData::new(Plains, Unemployed, 1)),
            ),
            (
                "a level above the last is kept",
                compound(&[("level", NbtTag::Int(9))]),
                Ok(VillagerData::new(Plains, Unemployed, 9)),
            ),
            (
                "a type the registry lacks",
                compound(&[("type", text("minecraft:moon"))]),
                Err(()),
            ),
            (
                "a level that is not an int",
                compound(&[("level", text("high"))]),
                Err(()),
            ),
        ];

        for (label, nbt, expected) in cases {
            let read = from_tag::<VillagerData>(nbt).map_err(drop);
            assert_eq!(read, expected, "{label}");
            if let Ok(data) = read {
                let written = to_nbt_tag(&data).unwrap();
                assert_eq!(from_tag::<VillagerData>(written.clone()).unwrap(), data);
                let NbtTag::Compound(written) = written else {
                    panic!("{label}: villager data is a compound");
                };
                assert_eq!(written.get_int("level"), Some(data.level()), "{label}");
                assert_eq!(written.get_string("type"), Some(data.kind.as_static_str()));
                assert_eq!(
                    written.get_string("profession"),
                    Some(data.profession.as_static_str())
                );
            }
        }
    }
}
