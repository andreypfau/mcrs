use mcrs_nbt::compound::NbtCompound;
use mcrs_protocol::{Ident, Text};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Dialog {
    #[serde(rename = "minecraft:notice")]
    Notice {
        #[serde(flatten)]
        common: CommonDialogData,
        action: Option<ActionButton>,
    },
    #[serde(rename = "minecraft:confirmation")]
    Confirmation {
        #[serde(flatten)]
        common: CommonDialogData,
        yes: ActionButton,
        no: ActionButton,
    },
    #[serde(rename = "minecraft:multi_action")]
    MultiAction {
        #[serde(flatten)]
        common: CommonDialogData,
        actions: Vec<ActionButton>,
        exit_action: Option<ActionButton>,
        columns: Option<i32>,
    },
    #[serde(rename = "minecraft:server_links")]
    ServerLinks {
        #[serde(flatten)]
        common: CommonDialogData,
        exit_action: Option<ActionButton>,
        columns: Option<i32>,
        button_width: Option<i32>,
    },
    #[serde(rename = "minecraft:dialog_list")]
    DialogList {
        #[serde(flatten)]
        common: CommonDialogData,
        dialogs: Vec<Dialog>,
        exit_action: Option<ActionButton>,
        columns: Option<i32>,
        button_width: Option<i32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommonDialogData {
    pub title: Text,
    pub external_title: Option<Text>,
    pub can_close_with_escape: Option<bool>,
    pub pause: Option<bool>,
    pub after_action: Option<DialogAction>,
    pub body: Option<Vec<DialogBody>>,
    pub inputs: Option<Vec<Input>>,
}

impl Default for CommonDialogData {
    fn default() -> Self {
        Self {
            title: Text::text("Dialog"),
            external_title: None,
            can_close_with_escape: None,
            pause: None,
            after_action: None,
            body: None,
            inputs: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionButton {
    #[serde(flatten)]
    pub button: CommonButtonData,
    pub action: Option<Action>,
}

impl Default for ActionButton {
    fn default() -> Self {
        Self {
            button: Default::default(),
            action: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommonButtonData {
    pub label: Text,
    pub tooltip: Option<Text>,
    pub width: Option<i32>,
}

impl Default for CommonButtonData {
    fn default() -> Self {
        Self {
            label: Text::text("Button"),
            tooltip: None,
            width: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "open_url")]
    OpenUrl { url: String },
    #[serde(rename = "run_command")]
    RunCommand { command: String },
    #[serde(rename = "suggest_command")]
    SuggestCommand { command: String },
    #[serde(rename = "change_page")]
    ChangePage { page: i32 },
    #[serde(rename = "copy_to_clipboard")]
    CopyToClipboard { value: String },
    #[serde(rename = "dynamic/run_command")]
    DynamicRunCommand { template: String },
    #[serde(rename = "dynamic/custom")]
    DynamicCustom {
        id: Ident<String>,
        additions: NbtCompound,
    },
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub enum DialogAction {
    #[serde(rename = "close")]
    #[default]
    Close,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "wait_for_response")]
    WaitForResponse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DialogBody {
    #[serde(rename = "minecraft:plain_text")]
    PlainMessage { contents: Text, width: Option<i32> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub key: String,
    #[serde(flatten)]
    pub control: InputControl,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InputControl {
    #[serde(rename = "minecraft:text")]
    Text {
        width: Option<i32>,
        label: Text,
        label_visible: Option<bool>,
        initial: Option<String>,
        max_length: Option<u32>,
        multiline: Option<MultilineOptions>,
    },
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct MultilineOptions {
    max_lines: Option<u32>,
    height: Option<u32>,
}
