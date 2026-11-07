use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};

use crate::property_table::PropertyTable;
use crate::resource_set::full_id;

pub const MISSING_MODEL: &str = "minecraft:builtin/missing";

/// One model of a part, with the turn of the model and its weight in the part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub model: String,
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub uvlock: bool,
    pub weight: u32,
}

impl Variant {
    #[must_use]
    pub fn missing() -> Self {
        Self {
            model: MISSING_MODEL.to_owned(),
            x: 0,
            y: 0,
            z: 0,
            uvlock: false,
            weight: 1,
        }
    }
}

/// The parts a state draws. A part is a weighted list of variants.
#[derive(Clone, Debug, PartialEq)]
pub enum Selection<'a> {
    /// The one part of the `variants` key that matches the state.
    Variants(&'a [Variant]),
    /// The `apply` parts of the `multipart` entries whose condition holds, in file order.
    Multipart(Vec<&'a [Variant]>),
    /// The state draws the missing model.
    Missing,
}

/// A blockstate file, checked against the properties of its block.
#[derive(Clone, Debug, Default)]
pub struct BlockStateFile {
    variants: Vec<KeyedPart>,
    multipart: Option<Vec<MultipartEntry>>,
    errors: Vec<String>,
}

#[derive(Clone, Debug)]
struct KeyedPart {
    /// `None` for a key with a content error, which matches no state.
    terms: Option<Vec<Term>>,
    part: Vec<Variant>,
}

#[derive(Clone, Copy, Debug)]
struct Term {
    property: usize,
    value: usize,
}

#[derive(Clone, Debug)]
struct MultipartEntry {
    when: Option<Condition>,
    apply: Vec<Variant>,
}

#[derive(Clone, Debug)]
enum Condition {
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Tests(Vec<PropertyTest>),
}

#[derive(Clone, Debug)]
struct PropertyTest {
    property: usize,
    /// Per value index of the property, whether an alternative accepts it.
    accepted: Vec<bool>,
}

impl Condition {
    fn holds(&self, state: &[usize]) -> bool {
        match self {
            Condition::All(conditions) => conditions.iter().all(|c| c.holds(state)),
            Condition::Any(conditions) => conditions.iter().any(|c| c.holds(state)),
            Condition::Tests(tests) => tests.iter().all(|test| {
                state
                    .get(test.property)
                    .and_then(|value| test.accepted.get(*value))
                    .copied()
                    .unwrap_or(false)
            }),
        }
    }
}

impl BlockStateFile {
    /// Reads a blockstate file. An error is a content error of the whole file: each state of
    /// the block then draws the missing model.
    pub fn parse(bytes: &[u8], properties: &PropertyTable) -> Result<Self, String> {
        let raw: RawFile = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if raw.variants.is_none() && raw.multipart.is_none() {
            return Err("the file has neither `variants` nor `multipart`".to_owned());
        }
        let mut file = Self::default();
        if let Some(Ordered(variants)) = raw.variants {
            if variants.is_empty() {
                return Err("`variants` is empty".to_owned());
            }
            for (key, part) in variants {
                let part = part.compile()?;
                let terms = match variant_key(&key, properties) {
                    Ok(terms) => Some(terms),
                    Err(error) => {
                        file.errors.push(format!("variant key `{key}`: {error}"));
                        None
                    }
                };
                file.variants.push(KeyedPart { terms, part });
            }
        }
        if let Some(entries) = raw.multipart {
            if entries.is_empty() {
                return Err("`multipart` is empty".to_owned());
            }
            let mut compiled = Vec::with_capacity(entries.len());
            for entry in entries {
                compiled.push(MultipartEntry {
                    when: entry
                        .when
                        .map(|when| condition(when, properties))
                        .transpose()?,
                    apply: entry.apply.compile()?,
                });
            }
            file.multipart = Some(compiled);
        }
        Ok(file)
    }

    /// The content errors of single variant keys, which leave the rest of the file valid.
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The parts of the state with these value indices, one per property of the block.
    #[must_use]
    pub fn select(&self, state: &[usize]) -> Selection<'_> {
        let matched = self.variants.iter().find(|keyed| {
            keyed.terms.as_ref().is_some_and(|terms| {
                terms
                    .iter()
                    .all(|term| state.get(term.property) == Some(&term.value))
            })
        });
        if let Some(keyed) = matched {
            return Selection::Variants(&keyed.part);
        }
        match &self.multipart {
            Some(entries) => Selection::Multipart(
                entries
                    .iter()
                    .filter(|entry| entry.when.as_ref().is_none_or(|when| when.holds(state)))
                    .map(|entry| entry.apply.as_slice())
                    .collect(),
            ),
            None => Selection::Missing,
        }
    }
}

fn variant_key(key: &str, properties: &PropertyTable) -> Result<Vec<Term>, String> {
    let mut terms: Vec<Term> = Vec::new();
    for term in key.split(',').filter(|term| !term.is_empty()) {
        let (name, value) = term
            .split_once('=')
            .ok_or_else(|| format!("the term `{term}` has no `=`"))?;
        let property = properties
            .index_of(name)
            .ok_or_else(|| format!("unknown property `{name}`"))?;
        let value = properties
            .properties
            .get(property)
            .and_then(|domain| domain.value_index(value))
            .ok_or_else(|| format!("unknown value `{value}` of `{name}`"))?;
        terms.retain(|earlier| earlier.property != property);
        terms.push(Term { property, value });
    }
    Ok(terms)
}

fn condition(raw: RawCondition, properties: &PropertyTable) -> Result<Condition, String> {
    let Ordered(members) = raw.0;
    let combined = members.iter().any(|(key, _)| key == "AND" || key == "OR");
    if combined {
        let [(key, RawTestValue::List(list))] = <[_; 1]>::try_from(members)
            .map_err(|_| "`AND` or `OR` beside another member".to_owned())?
        else {
            return Err("`AND` or `OR` without an array".to_owned());
        };
        let conditions = list
            .into_iter()
            .map(|condition_entry| condition(condition_entry, properties))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(if key == "AND" {
            Condition::All(conditions)
        } else {
            Condition::Any(conditions)
        });
    }
    if members.is_empty() {
        return Err("an empty condition".to_owned());
    }
    let mut tests = Vec::with_capacity(members.len());
    for (name, value) in members {
        let text = match value {
            RawTestValue::Text(text) => text,
            RawTestValue::Integer(number) => number.to_string(),
            RawTestValue::Boolean(flag) => flag.to_string(),
            RawTestValue::List(_) => return Err(format!("`{name}` holds an array")),
        };
        tests.push(property_test(&name, &text, properties)?);
    }
    Ok(Condition::Tests(tests))
}

fn property_test(
    name: &str,
    text: &str,
    properties: &PropertyTable,
) -> Result<PropertyTest, String> {
    let property = properties
        .index_of(name)
        .ok_or_else(|| format!("unknown property `{name}`"))?;
    let domain = properties
        .properties
        .get(property)
        .ok_or_else(|| format!("unknown property `{name}`"))?;
    let mut accepted = vec![false; domain.values.len()];
    for alternative in text.split('|') {
        let (negated, value) = match alternative.strip_prefix('!') {
            Some(value) => (true, value),
            None => (false, alternative),
        };
        if value.is_empty() {
            return Err(format!("an empty alternative in `{name}: {text}`"));
        }
        let index = domain
            .value_index(value)
            .ok_or_else(|| format!("unknown value `{value}` of `{name}`"))?;
        for (other, slot) in accepted.iter_mut().enumerate() {
            if (other == index) != negated {
                *slot = true;
            }
        }
    }
    Ok(PropertyTest { property, accepted })
}

#[derive(Deserialize)]
struct RawFile {
    variants: Option<Ordered<RawPart>>,
    multipart: Option<Vec<RawMultipart>>,
}

#[derive(Deserialize)]
struct RawMultipart {
    apply: RawPart,
    when: Option<RawCondition>,
}

#[derive(Deserialize)]
struct RawCondition(Ordered<RawTestValue>);

#[derive(Deserialize)]
#[serde(untagged)]
enum RawTestValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
    List(Vec<RawCondition>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPart {
    One(RawVariant),
    Many(Vec<RawVariant>),
}

#[derive(Deserialize)]
struct RawVariant {
    model: String,
    x: Option<i64>,
    y: Option<i64>,
    z: Option<i64>,
    uvlock: Option<bool>,
    weight: Option<i64>,
}

impl RawPart {
    fn compile(self) -> Result<Vec<Variant>, String> {
        match self {
            RawPart::One(variant) => Ok(vec![variant.compile(Weighted::No)?]),
            RawPart::Many(variants) if variants.is_empty() => Err("an empty part".to_owned()),
            RawPart::Many(variants) => variants
                .into_iter()
                .map(|variant| variant.compile(Weighted::Yes))
                .collect(),
        }
    }
}

#[derive(Clone, Copy)]
enum Weighted {
    Yes,
    No,
}

impl RawVariant {
    fn compile(self, weighted: Weighted) -> Result<Variant, String> {
        let turn = |angle: Option<i64>| -> Result<u16, String> {
            let degrees = angle.unwrap_or(0).rem_euclid(360);
            match degrees {
                0 | 90 | 180 | 270 => Ok(degrees as u16),
                _ => Err(format!(
                    "the rotation {} is no quarter turn",
                    angle.unwrap_or(0)
                )),
            }
        };
        let weight = match weighted {
            Weighted::Yes => self.weight.unwrap_or(1),
            Weighted::No => 1,
        };
        let weight = u32::try_from(weight)
            .ok()
            .filter(|weight| *weight >= 1)
            .ok_or_else(|| format!("the weight {weight} is below 1"))?;
        Ok(Variant {
            model: full_id(&self.model),
            x: turn(self.x)?,
            y: turn(self.y)?,
            z: turn(self.z)?,
            uvlock: self.uvlock.unwrap_or(false),
            weight,
        })
    }
}

/// The members of a JSON object in the order of the file.
struct Ordered<T>(Vec<(String, T)>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Ordered<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Members<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for Members<T> {
            type Value = Ordered<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry::<String, T>()? {
                    members.push(member);
                }
                Ok(Ordered(members))
            }
        }

        deserializer.deserialize_map(Members(PhantomData))
    }
}

impl<T> fmt::Debug for Ordered<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} members", self.0.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property_table::PropertyDomain;

    fn fence() -> PropertyTable {
        PropertyTable {
            properties: ["north", "south"]
                .into_iter()
                .map(|name| PropertyDomain {
                    name: name.to_owned(),
                    values: vec!["false".to_owned(), "true".to_owned()],
                })
                .collect(),
        }
    }

    fn models(selection: &Selection<'_>) -> Vec<String> {
        match selection {
            Selection::Variants(part) => part.iter().map(|v| v.model.clone()).collect(),
            Selection::Multipart(parts) => parts
                .iter()
                .flat_map(|part| part.iter().map(|v| v.model.clone()))
                .collect(),
            Selection::Missing => vec![MISSING_MODEL.to_owned()],
        }
    }

    #[test]
    fn a_negated_alternative_accepts_every_other_value() {
        let file = BlockStateFile::parse(
            br#"{"multipart": [{"when": {"north": "!true"}, "apply": {"model": "post"}}]}"#,
            &fence(),
        )
        .unwrap();
        assert_eq!(models(&file.select(&[0, 0])), ["minecraft:post"]);
        assert!(models(&file.select(&[1, 0])).is_empty());
    }

    #[test]
    fn variants_win_over_multipart_and_an_unmatched_state_falls_through() {
        let file = BlockStateFile::parse(
            br#"{"variants": {"north=true": {"model": "a"}},
                "multipart": [{"apply": {"model": "b"}}]}"#,
            &fence(),
        )
        .unwrap();
        assert_eq!(models(&file.select(&[1, 0])), ["minecraft:a"]);
        assert_eq!(models(&file.select(&[0, 0])), ["minecraft:b"]);
    }

    #[test]
    fn a_bad_key_matches_nothing_while_a_bad_condition_fails_the_file() {
        let file = BlockStateFile::parse(
            br#"{"variants": {"west=true": {"model": "a"}, "": {"model": "b"}}}"#,
            &fence(),
        )
        .unwrap();
        assert_eq!(file.errors().len(), 1);
        assert_eq!(models(&file.select(&[0, 0])), ["minecraft:b"]);

        let broken = BlockStateFile::parse(
            br#"{"multipart": [{"when": {"north": "maybe"}, "apply": {"model": "a"}}]}"#,
            &fence(),
        );
        assert!(broken.is_err());
        let off_turn =
            BlockStateFile::parse(br#"{"variants": {"": {"model": "a", "x": 45}}}"#, &fence());
        assert!(off_turn.is_err());
    }
}
