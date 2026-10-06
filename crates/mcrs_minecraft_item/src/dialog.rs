use std::fmt;

use crate::{Template, Text};
use mcrs_minecraft_core::codec::{
    Bounded, CompactList, Validate, default_true, is_default, is_true,
};
use mcrs_minecraft_core::{ResourceLocation, validated};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::{from_tag, to_nbt_compound};
use mcrs_minecraft_registry::HolderSet;
type ClickEvent = mcrs_minecraft_text::ClickEvent<Template>;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlainMessage {
    pub contents: Text,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Description(pub PlainMessage);

impl<'de> Deserialize<'de> for Description {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Message(PlainMessage),
            Contents(Text),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemBody {
    pub item: Template,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Description>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_decorations: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_tooltip: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ItemSize,
    #[serde(default, skip_serializing_if = "is_default")]
    pub height: ItemSize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DialogBody {
    #[serde(rename = "minecraft:item")]
    Item(ItemBody),
    #[serde(rename = "minecraft:plain_message")]
    PlainMessage(PlainMessage),
}

const DIALOG_BODY_TYPE_ROWS: &[&str] = &["minecraft:item", "minecraft:plain_message"];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    DIALOG_BODY_TYPE_ROWS,
    &[],
    keys::dialog_body_type::ENTRIES
));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputKey(String);

impl InputKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for InputKey {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BooleanInput {
    pub key: InputKey,
    pub label: Text,
    #[serde(default, skip_serializing_if = "is_default")]
    pub initial: bool,
    #[serde(default = "true_text", skip_serializing_if = "is_true_text")]
    pub on_true: String,
    #[serde(default = "false_text", skip_serializing_if = "is_false_text")]
    pub on_false: String,
}

validated!(NumberRangeInput);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct NumberRangeInput {
    pub key: InputKey,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
    pub label: Text,
    #[serde(default = "generic_value", skip_serializing_if = "is_generic_value")]
    pub label_format: String,
    pub start: f32,
    pub end: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f32>,
}

impl Validate for NumberRangeInput {
    fn validate(&self) -> Result<(), String> {
        if let Some(step) = self.step
            && !(step >= f32::from_bits(1) && step <= f32::MAX)
        {
            return Err(format!("Value must be positive: {step}"));
        }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct OptionEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<Text>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub initial: bool,
}

impl Serialize for OptionEntry {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        OptionEntry::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for OptionEntry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct EntryVisitor;

        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = OptionEntry;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an option id or an option")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<OptionEntry, E> {
                Ok(OptionEntry {
                    id: id.to_owned(),
                    display: None,
                    initial: false,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<OptionEntry, A::Error> {
                OptionEntry::deserialize(value::MapAccessDeserializer::new(map))
            }
        }

        d.deserialize_any(EntryVisitor)
    }
}

validated!(SingleOptionInput);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct SingleOptionInput {
    pub key: InputKey,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
    pub options: Vec<OptionEntry>,
    pub label: Text,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub label_visible: bool,
}

impl Validate for SingleOptionInput {
    fn validate(&self) -> Result<(), String> {
        if self.options.is_empty() {
            return Err("List must have contents".into());
        }
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

validated!(TextInput);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct TextInput {
    pub key: InputKey,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ControlWidth,
    pub label: Text,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub label_visible: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub initial: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub max_length: MaxLength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiline: Option<Multiline>,
}

impl Validate for TextInput {
    fn validate(&self) -> Result<(), String> {
        let length = self.initial.encode_utf16().count();
        if i64::try_from(length).is_ok_and(|length| length > i64::from(self.max_length.0)) {
            return Err("Default text length exceeds allowed size".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Input {
    #[serde(rename = "minecraft:boolean")]
    Boolean(BooleanInput),
    #[serde(rename = "minecraft:number_range")]
    NumberRange(NumberRangeInput),
    #[serde(rename = "minecraft:single_option")]
    SingleOption(SingleOptionInput),
    #[serde(rename = "minecraft:text")]
    Text(TextInput),
}

const INPUT_CONTROL_TYPE_ROWS: &[&str] = &[
    "minecraft:boolean",
    "minecraft:number_range",
    "minecraft:single_option",
    "minecraft:text",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    INPUT_CONTROL_TYPE_ROWS,
    &[],
    keys::input_control_type::ENTRIES
));

#[derive(Debug, Clone, PartialEq)]
pub struct CommandTemplate(String);

impl CommandTemplate {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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

impl Serialize for CommandTemplate {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum DynamicAction {
    #[serde(rename = "minecraft:dynamic/run_command")]
    RunCommand { template: CommandTemplate },
    #[serde(rename = "minecraft:dynamic/custom")]
    Custom {
        id: ResourceLocation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additions: Option<NbtCompound>,
    },
}

const STATIC_ACTIONS: [&str; 7] = [
    "open_url",
    "run_command",
    "suggest_command",
    "show_dialog",
    "change_page",
    "copy_to_clipboard",
    "custom",
];

const DYNAMIC_ACTIONS: [&str; 2] = ["dynamic/run_command", "dynamic/custom"];

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Static(ClickEvent),
    Dynamic(DynamicAction),
}

impl Serialize for Action {
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

impl<'de> Deserialize<'de> for Action {
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
        let kind = ResourceLocation::read(&kind).map_err(D::Error::custom)?;
        let name = kind.as_str().strip_prefix("minecraft:").unwrap_or("");

        if DYNAMIC_ACTIONS.contains(&name) {
            compound
                .child_tags
                .push(("type".to_owned(), NbtTag::String(kind.as_str().to_owned())));
            return from_tag(NbtTag::Compound(compound))
                .map(Action::Dynamic)
                .map_err(D::Error::custom);
        }
        if !STATIC_ACTIONS.contains(&name) {
            return Err(D::Error::custom(format_args!(
                "unknown dialog action type {kind}, expected one of minecraft:{}",
                STATIC_ACTIONS
                    .iter()
                    .chain(&DYNAMIC_ACTIONS)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", minecraft:")
            )));
        }

        let given: Vec<String> = compound
            .child_tags
            .iter()
            .map(|(key, _)| key.clone())
            .collect();
        compound
            .child_tags
            .push(("action".to_owned(), NbtTag::String(name.to_owned())));
        let event: ClickEvent = from_tag(NbtTag::Compound(compound)).map_err(D::Error::custom)?;
        let written = to_nbt_compound(&event).map_err(D::Error::custom)?;
        if let Some(unknown) = given.iter().find(|key| written.get(key).is_none()) {
            return Err(D::Error::custom(format_args!(
                "unknown field `{unknown}` in the {kind} action"
            )));
        }
        Ok(Action::Static(event))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionButton {
    pub label: Text,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<Text>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub width: ButtonWidth,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
}

fn ok_button() -> ActionButton {
    ActionButton {
        label: Text::translate("gui.ok", Vec::new()),
        tooltip: None,
        width: ButtonWidth::default(),
        action: None,
    }
}

fn is_ok_button(button: &ActionButton) -> bool {
    *button == ok_button()
}

trait Specific {
    fn check(&self) -> Result<(), String> {
        Ok(())
    }
}

macro_rules! dialog_type {
    ($name:ident { $($field:tt)* }) => {
        validated!($name);

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(remote = "Self", deny_unknown_fields)]
        pub struct $name {
            pub title: Text,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub external_title: Option<Text>,
            #[serde(default = "default_true", skip_serializing_if = "is_true")]
            pub can_close_with_escape: bool,
            #[serde(default = "default_true", skip_serializing_if = "is_true")]
            pub pause: bool,
            #[serde(default, skip_serializing_if = "is_default")]
            pub after_action: AfterAction,
            #[serde(default, skip_serializing_if = "CompactList::is_empty")]
            pub body: CompactList<DialogBody>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub inputs: Vec<Input>,
            $($field)*
        }

        impl Validate for $name {
            fn validate(&self) -> Result<(), String> {
                if self.pause && !self.after_action.unpauses() {
                    return Err(
                        "Dialogs that pause the game must use after_action values that unpause it after user action!"
                            .into(),
                    );
                }
                Specific::check(self)
            }
        }
    };
}

dialog_type!(Notice {
    #[serde(default = "ok_button", skip_serializing_if = "is_ok_button")]
    pub action: ActionButton,
});

impl Specific for Notice {}

dialog_type!(Confirmation {
    pub yes: ActionButton,
    pub no: ActionButton,
});

impl Specific for Confirmation {}

dialog_type!(MultiAction {
    pub actions: Vec<ActionButton>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_action: Option<ActionButton>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: Columns,
});

impl Specific for MultiAction {
    fn check(&self) -> Result<(), String> {
        if self.actions.is_empty() {
            return Err("List must have contents".into());
        }
        Ok(())
    }
}

dialog_type!(ServerLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_action: Option<ActionButton>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: Columns,
    #[serde(default, skip_serializing_if = "is_default")]
    pub button_width: ButtonWidth,
});

impl Specific for ServerLinks {}

// chisle: a dialog list names registered dialogs and tags only; the game also takes a dialog
// written inline in the list, which is refused here until a holder set that holds inline entries
// exists.
dialog_type!(DialogList {
    pub dialogs: HolderSet<keys::Dialog>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_action: Option<ActionButton>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub columns: Columns,
    #[serde(default, skip_serializing_if = "is_default")]
    pub button_width: ButtonWidth,
});

impl Specific for DialogList {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Dialog {
    #[serde(rename = "minecraft:notice")]
    Notice(Notice),
    #[serde(rename = "minecraft:confirmation")]
    Confirmation(Confirmation),
    #[serde(rename = "minecraft:multi_action")]
    MultiAction(MultiAction),
    #[serde(rename = "minecraft:server_links")]
    ServerLinks(ServerLinks),
    #[serde(rename = "minecraft:dialog_list")]
    DialogList(DialogList),
}

const DIALOG_TYPE_ROWS: &[&str] = &[
    "minecraft:notice",
    "minecraft:confirmation",
    "minecraft:multi_action",
    "minecraft:server_links",
    "minecraft:dialog_list",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    DIALOG_TYPE_ROWS,
    &[],
    keys::dialog_type::ENTRIES
));

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn dialog_body_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<DialogBody>(
            DIALOG_BODY_TYPE_ROWS,
            &[],
            keys::dialog_body_type::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }

    #[test]
    fn input_control_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<Input>(
            INPUT_CONTROL_TYPE_ROWS,
            &[],
            keys::input_control_type::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }

    #[test]
    fn dialog_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<Dialog>(
            DIALOG_TYPE_ROWS,
            &[],
            keys::dialog_type::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
