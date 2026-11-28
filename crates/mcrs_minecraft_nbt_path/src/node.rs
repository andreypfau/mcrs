use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::number::Number;
use crate::selected_tag::SelectedTag;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Node {
    Root(NbtCompound),
    Name(String),
    NamePattern { name: String, pattern: NbtCompound },
    All,
    Index(i32),
    ElementPattern(NbtCompound),
}

impl Node {
    pub(crate) const fn is_index(&self) -> bool {
        matches!(self, Node::All | Node::Index(_) | Node::ElementPattern(_))
    }

    pub(crate) fn select<'t>(&self, tag: &'t NbtTag, mut on_selected: impl FnMut(SelectedTag<'t>)) {
        match (self, tag) {
            (Node::Root(pattern), _) => {
                if is_matching_compound(pattern, tag) {
                    on_selected(SelectedTag::Tag(tag));
                }
            }
            (Node::Name(name), NbtTag::Compound(compound)) => {
                if let Some(value) = compound.get(name) {
                    on_selected(SelectedTag::Tag(value));
                }
            }
            (Node::NamePattern { name, pattern }, NbtTag::Compound(compound)) => {
                if let Some(value) = compound.get(name)
                    && is_matching_compound(pattern, value)
                {
                    on_selected(SelectedTag::Tag(value));
                }
            }
            (Node::All, NbtTag::List(list)) => {
                for element in list {
                    on_selected(SelectedTag::Tag(element));
                }
            }
            (Node::All, _) => {
                for index in 0..array_len(tag) {
                    if let Some(element) = array_element(tag, index) {
                        on_selected(SelectedTag::Element(element));
                    }
                }
            }
            (Node::Index(index), NbtTag::List(list)) => {
                if let Some(element) = resolve(*index, list.len()).and_then(|at| list.get(at)) {
                    on_selected(SelectedTag::Tag(element));
                }
            }
            (Node::Index(index), _) => {
                if let Some(element) =
                    resolve(*index, array_len(tag)).and_then(|at| array_element(tag, at))
                {
                    on_selected(SelectedTag::Element(element));
                }
            }
            (Node::ElementPattern(pattern), NbtTag::List(list)) => {
                for element in list {
                    if is_matching_compound(pattern, element) {
                        on_selected(SelectedTag::Tag(element));
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn get_or_create<'t>(
        &self,
        tag: &'t mut NbtTag,
        new_tag: impl FnOnce() -> NbtTag,
        out: &mut Vec<&'t mut NbtTag>,
    ) -> bool {
        match (self, tag) {
            (Node::Root(pattern), tag) => {
                if is_matching_compound(pattern, tag) {
                    out.push(tag);
                }
            }
            (Node::Name(name), NbtTag::Compound(compound)) => {
                if let Some(value) = entry(compound, name, new_tag) {
                    out.push(value);
                }
            }
            (Node::NamePattern { name, pattern }, NbtTag::Compound(compound)) => {
                let create = || NbtTag::Compound(pattern.clone());
                if let Some(value) = entry(compound, name, create)
                    && is_matching_compound(pattern, value)
                {
                    out.push(value);
                }
            }
            (Node::All, NbtTag::List(list)) => {
                if list.is_empty() {
                    list.push(new_tag());
                }
                out.extend(list.iter_mut());
            }
            (Node::Index(index), NbtTag::List(list)) => {
                let length = list.len();
                if let Some(element) = resolve(*index, length).and_then(|at| list.get_mut(at)) {
                    out.push(element);
                }
            }
            (Node::ElementPattern(pattern), NbtTag::List(list)) => {
                let found = list
                    .iter()
                    .any(|element| is_matching_compound(pattern, element));
                if !found {
                    list.push(NbtTag::Compound(pattern.clone()));
                }
                for element in list.iter_mut() {
                    if is_matching_compound(pattern, element) {
                        out.push(element);
                    }
                }
            }
            (Node::All, tag) => return array_len(tag) > 0,
            (Node::Index(index), tag) => return resolve(*index, array_len(tag)).is_some(),
            _ => {}
        }
        false
    }

    pub(crate) fn write(&self, tag: &mut NbtTag, value: &NbtTag) {
        match (self, tag) {
            (Node::Name(name), NbtTag::Compound(compound)) => compound.put(name, value.clone()),
            (Node::NamePattern { name, pattern }, NbtTag::Compound(compound)) => {
                let target = compound
                    .child_tags
                    .iter_mut()
                    .find(|(key, _)| key == name)
                    .map(|(_, existing)| existing);
                if let Some(existing) = target
                    && is_matching_compound(pattern, existing)
                {
                    *existing = value.clone();
                }
            }
            (Node::All | Node::ElementPattern(_), NbtTag::List(list)) if list.is_empty() => {
                list.push(value.clone());
            }
            (Node::All, NbtTag::List(list)) => {
                for element in list.iter_mut() {
                    *element = value.clone();
                }
            }
            (Node::Index(index), NbtTag::List(list)) => {
                let length = list.len();
                if let Some(element) = resolve(*index, length).and_then(|at| list.get_mut(at)) {
                    *element = value.clone();
                }
            }
            (Node::ElementPattern(pattern), NbtTag::List(list)) => {
                for element in list.iter_mut() {
                    if is_matching_compound(pattern, element) {
                        *element = value.clone();
                    }
                }
            }
            (Node::All | Node::Index(_), tag) => self.write_array(tag, value),
            _ => {}
        }
    }

    fn write_array(&self, tag: &mut NbtTag, value: &NbtTag) {
        let Some(number) = Number::of(value) else {
            return;
        };
        let append = matches!(self, Node::All);
        match tag {
            NbtTag::ByteArray(array) => {
                let [element] = number.to_i8().to_ne_bytes();
                if append && array.is_empty() {
                    *array = Box::from([element]);
                } else {
                    self.fill(array, element);
                }
            }
            NbtTag::IntArray(array) if append && array.is_empty() => array.push(number.to_i32()),
            NbtTag::IntArray(array) => self.fill(array, number.to_i32()),
            NbtTag::LongArray(array) if append && array.is_empty() => array.push(number.to_i64()),
            NbtTag::LongArray(array) => self.fill(array, number.to_i64()),
            _ => {}
        }
    }

    fn fill<T: Copy>(&self, elements: &mut [T], element: T) {
        match self {
            Node::All => elements.fill(element),
            Node::Index(index) => {
                let length = elements.len();
                if let Some(slot) = resolve(*index, length).and_then(|at| elements.get_mut(at)) {
                    *slot = element;
                }
            }
            _ => {}
        }
    }
}

fn is_matching_compound(pattern: &NbtCompound, tag: &NbtTag) -> bool {
    match tag {
        NbtTag::Compound(compound) => pattern.matches(compound),
        _ => false,
    }
}

fn entry<'t>(
    compound: &'t mut NbtCompound,
    name: &str,
    create: impl FnOnce() -> NbtTag,
) -> Option<&'t mut NbtTag> {
    let tags = &mut compound.child_tags;
    match tags.iter().position(|(key, _)| key == name) {
        Some(at) => tags.get_mut(at).map(|(_, value)| value),
        None => {
            tags.push((String::from(name), create()));
            tags.last_mut().map(|(_, value)| value)
        }
    }
}

fn resolve(index: i32, length: usize) -> Option<usize> {
    let magnitude = usize::try_from(index.unsigned_abs()).ok()?;
    if index < 0 {
        length.checked_sub(magnitude)
    } else {
        (magnitude < length).then_some(magnitude)
    }
}

fn array_len(tag: &NbtTag) -> usize {
    match tag {
        NbtTag::ByteArray(array) => array.len(),
        NbtTag::IntArray(array) => array.len(),
        NbtTag::LongArray(array) => array.len(),
        _ => 0,
    }
}

fn array_element(tag: &NbtTag, index: usize) -> Option<NbtTag> {
    match tag {
        NbtTag::ByteArray(array) => array
            .get(index)
            .map(|byte| NbtTag::Byte(i8::from_ne_bytes([*byte]))),
        NbtTag::IntArray(array) => array.get(index).copied().map(NbtTag::Int),
        NbtTag::LongArray(array) => array.get(index).copied().map(NbtTag::Long),
        _ => None,
    }
}
