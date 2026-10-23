use std::fmt;

use crate::component::consume::positive_float;
use crate::{Template, Text};
use mcrs_minecraft_core::codec::{
    self, Bounded, CompactList, Validate, default_true, is_default, is_true,
};
use mcrs_minecraft_core::{ResourceLocation, validated};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, to_nbt_compound};
use mcrs_minecraft_registries::{
    Named, Namer, Refs, Resolve, ResolveError, Resolved, Resolver, dispatch, registry_value,
};
type ClickEvent<M> = mcrs_minecraft_text::ClickEvent<Template<M>>;
use crate::keys::DialogActionType;
use serde::de::value::StrDeserializer;
use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

type ButtonWidth = Bounded<1, 1024, 150>;
type ControlWidth = Bounded<1, 1024, 200>;
type ItemSize = Bounded<1, 256, 16>;
type Columns = Bounded<1, { i32::MAX }, 2>;
type MaxLength = Bounded<1, { i32::MAX }, 32>;
type Positive = Bounded<1, { i32::MAX }, 1>;
type Height = Bounded<1, 512, 1>;

macro_rules! text_default {
    ($make:ident, $is:ident, $text:literal) => {
        fn $make() -> String {
            $text.to_owned()
        }

        fn $is(value: &String) -> bool {
            value == $text
        }
    };
}

text_default!(true_text, is_true_text, "true");
text_default!(false_text, is_false_text, "false");
text_default!(generic_value, is_generic_value, "options.generic_value");

fn is_variable_name(name: &str) -> bool {
    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterAction {
    #[default]
    Close,
    None,
    WaitForResponse,
}

impl AfterAction {
    fn unpauses(self) -> bool {
        self != AfterAction::None
    }
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlainMessage<M: Refs = Resolved> {
    pub contents: Text<M>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Description<M: Refs = Resolved>(pub PlainMessage<M>);

impl<'de> Deserialize<'de> for Description<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Message(PlainMessage<Named>),
            Contents(Text<Named>),
        }

        Ok(Description(match Repr::deserialize(d)? {
            Repr::Message(message) => message,
            Repr::Contents(contents) => PlainMessage {
                contents,
                width: ControlWidth::default(),
            },
        }))
    }
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemBody<M: Refs = Resolved> {
    pub item: Template<M>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Description<M>>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_decorations: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_tooltip: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ItemSize,
    #[serde(default, skip_serializing_if = "is_default")]
    pub height: ItemSize,
}

#[registry_value]
#[dispatch(registry = crate::keys::DialogBodyType)]
#[derive(Debug, Clone, PartialEq)]
pub enum DialogBody<M: Refs = Resolved> {
    Item(ItemBody<M>),
    PlainMessage(PlainMessage<M>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct InputKey(String);

impl<'de> Deserialize<'de> for InputKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let key = String::deserialize(d)?;
        if !is_variable_name(&key) {
            return Err(D::Error::custom(format_args!(
                "{key} is not a valid input name"
            )));
        }
        Ok(InputKey(key))
    }
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BooleanInput<M: Refs = Resolved> {
    pub key: InputKey,
    pub label: Text<M>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub initial: bool,
    #[serde(default = "true_text", skip_serializing_if = "is_true_text")]
    pub on_true: String,
    #[serde(default = "false_text", skip_serializing_if = "is_false_text")]
    pub on_false: String,
}

validated!(NumberRangeInput<Named>);

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct NumberRangeInput<M: Refs = Resolved> {
    pub key: InputKey,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
    pub label: Text<M>,
    #[serde(default = "generic_value", skip_serializing_if = "is_generic_value")]
    pub label_format: String,
    pub start: f32,
    pub end: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<f32>,
    #[serde(
        default,
        deserialize_with = "positive_step",
        skip_serializing_if = "Option::is_none"
    )]
    pub step: Option<f32>,
}

fn positive_step<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    positive_float(d).map(Some)
}

impl<M: Refs> Validate for NumberRangeInput<M> {
    fn validate(&self) -> Result<(), String> {
        if let Some(initial) = self.initial {
            let low = self.start.min(self.end);
            let high = self.start.max(self.end);
            if !(low..=high).contains(&initial) {
                return Err(format!(
                    "Initial value {initial} is outside of range [{low}, {high}]"
                ));
            }
        }
        Ok(())
    }
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct OptionEntry<M: Refs = Resolved> {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<Text<M>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub initial: bool,
}

impl Serialize for OptionEntry<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        OptionEntry::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for OptionEntry<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct EntryVisitor;

        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = OptionEntry<Named>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an option id or an option")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<OptionEntry<Named>, E> {
                Ok(OptionEntry {
                    id: id.to_owned(),
                    display: None,
                    initial: false,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<OptionEntry<Named>, A::Error> {
                OptionEntry::deserialize(value::MapAccessDeserializer::new(map))
            }
        }

        d.deserialize_any(EntryVisitor)
    }
}

validated!(SingleOptionInput<Named>);

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SingleOptionInput<M: Refs = Resolved> {
    pub key: InputKey,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
    #[serde(
        deserialize_with = "codec::non_empty",
        bound(deserialize = "OptionEntry<M>: Deserialize<'de>")
    )]
    pub options: Vec<OptionEntry<M>>,
    pub label: Text<M>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub label_visible: bool,
}

impl<M: Refs> Validate for SingleOptionInput<M> {
    fn validate(&self) -> Result<(), String> {
        if self.options.iter().filter(|option| option.initial).count() > 1 {
            return Err("Multiple initial values".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Multiline {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<Positive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Height>,
}

validated!(TextInput<Named>);

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TextInput<M: Refs = Resolved> {
    pub key: InputKey,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
    pub label: Text<M>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub label_visible: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub initial: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub max_length: MaxLength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiline: Option<Multiline>,
}

impl<M: Refs> Validate for TextInput<M> {
    fn validate(&self) -> Result<(), String> {
        let length = self.initial.encode_utf16().count();
        if i64::try_from(length).is_ok_and(|length| length > i64::from(self.max_length.0)) {
            return Err("Default text length exceeds allowed size".into());
        }
        Ok(())
    }
}

#[registry_value]
#[dispatch(registry = crate::keys::InputControlType)]
#[derive(Debug, Clone, PartialEq)]
pub enum Input<M: Refs = Resolved> {
    Boolean(BooleanInput<M>),
    NumberRange(NumberRangeInput<M>),
    SingleOption(SingleOptionInput<M>),
    Text(TextInput<M>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CommandTemplate(String);

fn check_template(input: &str) -> Result<(), String> {
    let mut start = 0;
    let mut cursor = 0;
    while let Some(found) = input[cursor..].find('$').map(|at| at + cursor) {
        if input.as_bytes().get(found + 1) == Some(&b'(') {
            let Some(end) = input[found + 1..].find(')').map(|at| at + found + 1) else {
                return Err("Unterminated macro variable".into());
            };
            let name = &input[found + 2..end];
            if !is_variable_name(name) {
                return Err(format!("Invalid macro variable name '{name}'"));
            }
            start = end + 1;
            cursor = start;
        } else {
            cursor = found + 1;
        }
    }
    if start == 0 {
        return Err("No variables in macro".into());
    }
    Ok(())
}

impl<'de> Deserialize<'de> for CommandTemplate {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let template = String::deserialize(d)?;
        check_template(&template).map_err(|reason| {
            D::Error::custom(format_args!(
                "Failed to parse template {template}: {reason}"
            ))
        })?;
        Ok(CommandTemplate(template))
    }
}

#[dispatch(registry = crate::keys::DialogActionType, unsupported(OpenUrl, RunCommand, SuggestCommand, ShowDialog, ChangePage, CopyToClipboard, Custom))]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum DynamicAction {
    #[dispatch(DynamicRunCommand)]
    RunCommand { template: CommandTemplate },
    #[dispatch(DynamicCustom)]
    Custom {
        id: ResourceLocation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additions: Option<NbtCompound>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action<M: Refs = Resolved> {
    Static(ClickEvent<M>),
    Dynamic(DynamicAction),
}

impl Resolve for Action {
    type Named = Action<Named>;

    fn resolve<C: Resolver>(named: Action<Named>, ctx: &C) -> Result<Self, ResolveError> {
        Ok(match named {
            Action::Static(event) => Action::Static(Resolve::resolve(event, ctx)?),
            Action::Dynamic(action) => Action::Dynamic(action),
        })
    }

    fn name<C: Namer>(&self, names: &C) -> Result<Action<Named>, ResolveError> {
        Ok(match self {
            Action::Static(event) => Action::Static(event.name(names)?),
            Action::Dynamic(action) => Action::Dynamic(action.clone()),
        })
    }
}

impl Serialize for Action<Named> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Action::Dynamic(action) => action.serialize(s),
            Action::Static(event) => {
                let mut written = to_nbt_compound(event).map_err(S::Error::custom)?;
                let position = written
                    .child_tags
                    .iter()
                    .position(|(key, _)| key == "action")
                    .ok_or_else(|| S::Error::custom("a click event writes no action"))?;
                let (_, name) = written.child_tags.remove(position);
                let NbtTag::String(name) = name else {
                    return Err(S::Error::custom("a click event writes a non-string action"));
                };
                written.child_tags.insert(
                    0,
                    (
                        "type".to_owned(),
                        NbtTag::String(format!("minecraft:{name}")),
                    ),
                );
                written.serialize(s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Action<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut compound = NbtCompound::deserialize(d)?;
        let position = compound
            .child_tags
            .iter()
            .position(|(key, _)| key == "type")
            .ok_or_else(|| D::Error::missing_field("type"))?;
        let NbtTag::String(kind) = compound.child_tags.remove(position).1 else {
            return Err(D::Error::custom("'type' is not a string"));
        };
        let entry = DialogActionType::deserialize(StrDeserializer::<D::Error>::new(&kind))?;

        match entry {
            DialogActionType::DynamicRunCommand | DialogActionType::DynamicCustom => {
                compound.child_tags.push((
                    "type".to_owned(),
                    NbtTag::String(entry.as_static_str().to_owned()),
                ));
                return from_tag(NbtTag::Compound(compound))
                    .map(Action::Dynamic)
                    .map_err(D::Error::custom);
            }
            DialogActionType::OpenUrl
            | DialogActionType::RunCommand
            | DialogActionType::SuggestCommand
            | DialogActionType::ShowDialog
            | DialogActionType::ChangePage
            | DialogActionType::CopyToClipboard
            | DialogActionType::Custom => {}
        }
        let kind = entry.location();
        let name = kind.path();

        let given: Vec<String> = compound
            .child_tags
            .iter()
            .map(|(key, _)| key.clone())
            .collect();
        compound
            .child_tags
            .push(("action".to_owned(), NbtTag::String(name.to_owned())));
        let event: ClickEvent<Named> =
            from_tag(NbtTag::Compound(compound)).map_err(D::Error::custom)?;
        let written = to_nbt_compound(&event).map_err(D::Error::custom)?;
        if let Some(unknown) = given.iter().find(|key| written.get(key).is_none()) {
            return Err(D::Error::custom(format_args!(
                "unknown field `{unknown}` in the {kind} action"
            )));
        }
        Ok(Action::Static(event))
    }
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionButton<M: Refs = Resolved> {
    pub label: Text<M>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<Text<M>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ButtonWidth,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action<M>>,
}

fn ok_button<M: Refs>() -> ActionButton<M> {
    ActionButton {
        label: Text::translate("gui.ok", Vec::new()),
        tooltip: None,
        width: ButtonWidth::default(),
        action: None,
    }
}

fn is_ok_button<M: Refs>(button: &ActionButton<M>) -> bool {
    *button == ok_button()
}

macro_rules! dialog_type {
    ($name:ident { $($field:tt)* }) => {
        validated!($name<Named>);

        #[registry_value]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(remote = "Self", deny_unknown_fields)]
        pub struct $name<M: Refs = Resolved> {
            pub title: Text<M>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub external_title: Option<Text<M>>,
            #[serde(default = "default_true", skip_serializing_if = "is_true")]
            pub can_close_with_escape: bool,
            #[serde(default = "default_true", skip_serializing_if = "is_true")]
            pub pause: bool,
            #[serde(default, skip_serializing_if = "is_default")]
            pub after_action: AfterAction,
            #[serde(default, skip_serializing_if = "CompactList::is_empty")]
            pub body: CompactList<DialogBody<M>>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub inputs: Vec<Input<M>>,
            $($field)*
        }

        impl<M: Refs> Validate for $name<M> {
            fn validate(&self) -> Result<(), String> {
                if self.pause && !self.after_action.unpauses() {
                    return Err(
                        "Dialogs that pause the game must use after_action values that unpause it after user action!"
                            .into(),
                    );
                }
                Ok(())
            }
        }
    };
}

dialog_type!(Notice {
    #[serde(default = "ok_button", skip_serializing_if = "is_ok_button")]
    pub action: ActionButton<M>,
});

dialog_type!(Confirmation {
    pub yes: ActionButton<M>,
    pub no: ActionButton<M>,
});

dialog_type!(MultiAction {
    #[serde(
        deserialize_with = "codec::non_empty",
        bound(deserialize = "ActionButton<M>: Deserialize<'de>")
    )]
    pub actions: Vec<ActionButton<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_action: Option<ActionButton<M>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: Columns,
});

dialog_type!(ServerLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_action: Option<ActionButton<M>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: Columns,
    #[serde(default, skip_serializing_if = "is_default")]
    pub button_width: ButtonWidth,
});

// chisle: a dialog list names registered dialogs and tags only; the game also takes a dialog
// written inline in the list, which is refused here until a holder set that holds inline entries
// exists.
dialog_type!(DialogList {
    pub dialogs: M::Set<Dialog>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_action: Option<ActionButton<M>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: Columns,
    #[serde(default, skip_serializing_if = "is_default")]
    pub button_width: ButtonWidth,
});

#[registry_value]
#[dispatch(registry = crate::keys::DialogType)]
#[derive(Debug, Clone, PartialEq)]
pub enum Dialog<M: Refs = Resolved> {
    Notice(Notice<M>),
    Confirmation(Confirmation<M>),
    MultiAction(MultiAction<M>),
    ServerLinks(ServerLinks<M>),
    DialogList(DialogList<M>),
}

pub type DialogRef<M = Resolved> = mcrs_minecraft_registries::Ref<M, Dialog, Dialog<M>>;
