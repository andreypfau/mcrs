use mcrs_minecraft_block::keys::BlockEntityType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(&'static str),
    List(&'static [Tag]),
    Compound(Fields),
    ByteArray(&'static [u8]),
    IntArray(&'static [i32]),
    LongArray(&'static [i64]),
}

pub type Fields = &'static [(&'static str, Tag)];

impl Tag {
    pub fn to_nbt(&self) -> NbtTag {
        match *self {
            Tag::Byte(value) => NbtTag::Byte(value),
            Tag::Short(value) => NbtTag::Short(value),
            Tag::Int(value) => NbtTag::Int(value),
            Tag::Long(value) => NbtTag::Long(value),
            Tag::Float(value) => NbtTag::Float(value),
            Tag::Double(value) => NbtTag::Double(value),
            Tag::String(value) => NbtTag::String(value.to_owned()),
            Tag::List(items) => NbtTag::List(items.iter().map(Tag::to_nbt).collect()),
            Tag::Compound(fields) => NbtTag::Compound(compound(fields)),
            Tag::ByteArray(bytes) => NbtTag::ByteArray(bytes.into()),
            Tag::IntArray(ints) => NbtTag::IntArray(ints.to_vec()),
            Tag::LongArray(longs) => NbtTag::LongArray(longs.to_vec()),
        }
    }
}

pub fn compound(fields: Fields) -> NbtCompound {
    NbtCompound {
        child_tags: fields
            .iter()
            .map(|(name, tag)| ((*name).to_owned(), tag.to_nbt()))
            .collect(),
    }
}

fn bucket(name: &str, buckets: u32) -> u32 {
    let hash = name.encode_utf16().fold(0u32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(unit.into())
    });
    (hash ^ (hash >> 16)) & (buckets - 1)
}

/// The fields of every layer, a later layer replacing what an earlier one
/// says. Stored templates list an entity's fields as a hash map of them
/// iterates: by bucket, the table doubling from 16 while more than three
/// quarters full, and by name within a bucket.
pub fn layered(layers: &[Fields]) -> NbtCompound {
    let mut fields: Vec<(&str, Tag)> = Vec::new();
    for (name, tag) in layers.iter().copied().flatten() {
        match fields.iter_mut().find(|(known, _)| known == name) {
            Some(field) => field.1 = *tag,
            None => fields.push((name, *tag)),
        }
    }
    let mut buckets = 16;
    while fields.len() * 4 > buckets as usize * 3 {
        buckets *= 2;
    }
    fields.sort_by_key(|(name, _)| (bucket(name, buckets), *name));
    NbtCompound {
        child_tags: fields
            .iter()
            .map(|(name, tag)| ((*name).to_owned(), tag.to_nbt()))
            .collect(),
    }
}

pub(super) enum BlockData {
    Jigsaw {
        name: String,
        pool: String,
        final_state: String,
        aligned: bool,
    },
    Fixed(NbtCompound),
}

fn texts(fields: &[(&str, &str)]) -> NbtCompound {
    NbtCompound {
        child_tags: fields
            .iter()
            .map(|(name, value)| ((*name).to_owned(), NbtTag::String((*value).to_owned())))
            .collect(),
    }
}

pub(super) fn chest(loot_table: &str) -> BlockData {
    BlockData::Fixed(texts(&[
        ("LootTable", loot_table),
        ("id", BlockEntityType::Chest.as_static_str()),
    ]))
}

impl BlockData {
    pub(super) fn to_nbt(&self, vertical: bool) -> NbtCompound {
        match self {
            BlockData::Fixed(nbt) => nbt.clone(),
            BlockData::Jigsaw {
                name,
                pool,
                final_state,
                aligned,
            } => {
                let joint = if vertical && !aligned {
                    "rollable"
                } else {
                    "aligned"
                };
                texts(&[
                    ("joint", joint),
                    ("final_state", final_state),
                    ("name", name),
                    ("pool", pool),
                    ("id", BlockEntityType::Jigsaw.as_static_str()),
                    ("target", name),
                ])
            }
        }
    }
}
