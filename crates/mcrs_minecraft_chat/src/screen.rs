use crate::command::{CommandInput, Context, Request};
use crate::edit::EditBox;
use crate::history::{History, Recall};
use crate::normalize::normalize;
use mcrs_minecraft_command::Lookup;

/// A key of the chat screen, after the platform mapped Cmd or Ctrl.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Tab { back: bool },
    Escape,
    Enter,
    Backspace { word: bool },
    Delete { word: bool },
    Left { word: bool, select: bool },
    Right { word: bool, select: bool },
    Home { select: bool },
    End { select: bool },
    PageUp,
    PageDown,
    SelectAll,
    Copy,
    Cut,
}

/// What a send carries (CHAT-SEND-002 to CHAT-SEND-004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Send {
    Chat(String),
    /// The command line without its `/`; `signed` picks `chat_command_signed`.
    Command {
        line: String,
        signed: bool,
    },
}

/// Why the chat screen closed (CHAT-OPEN-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Closed {
    Escape,
    /// Enter closed the screen, with what to send, if anything. `history_changed` asks the
    /// caller to write the command history again.
    Sent {
        send: Option<Send>,
        history_changed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ignored,
    Handled,
    Closed(Closed),
    /// Page Up (`1`) or Page Down (`-1`) scrolls the chat log by `perPage − 1` lines.
    Page(isize),
    Copy(String),
}

/// The input the client keeps after the screen closed, if any (CHAT-OPEN-002).
pub fn keeps_draft(input: &str, closed: Option<&Closed>, save_unsent: bool) -> bool {
    if input.trim().is_empty() {
        return false;
    }
    match closed {
        Some(Closed::Sent { .. }) => false,
        Some(Closed::Escape) => save_unsent,
        None => true,
    }
}

/// The chat screen while it is open: the input box, the history position, the draft style and
/// the command state (CHAT-OPEN-001 to CHAT-KEY-001).
#[derive(Debug, Default)]
pub struct ChatScreen {
    edit: EditBox,
    recall: Recall,
    draft: bool,
    pub commands: CommandInput,
    sent: String,
    request: Option<Request>,
}

impl ChatScreen {
    /// Opens on `initial`; a draft shows in its own style until the first change
    /// (CHAT-OPEN-001). The suggestion list does not show until the player types or presses
    /// Tab (CCMD-LIST-001).
    pub fn open<L: Lookup>(&mut self, initial: &str, draft: bool, cx: &Context<'_, L>) {
        self.edit.set(initial);
        self.recall = Recall::default();
        self.draft = draft;
        self.commands.auto = false;
        self.commands.hide_list();
        self.refresh(cx, false);
    }

    /// The server request the last call made, to send now (CCMD-REQ-001).
    pub fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }

    pub fn input(&self) -> &str {
        self.edit.text()
    }

    pub fn edit(&self) -> &EditBox {
        &self.edit
    }

    pub fn is_draft(&self) -> bool {
        self.draft
    }

    fn refresh<L: Lookup>(&mut self, cx: &Context<'_, L>, keep_list: bool) {
        if let Some(request) =
            self.commands
                .refresh(self.edit.text(), self.edit.caret(), cx, keep_list)
        {
            self.request = Some(request);
        }
    }

    /// The player changed the input.
    fn changed<L: Lookup>(&mut self, cx: &Context<'_, L>) -> Outcome {
        self.draft = false;
        self.commands.auto = true;
        self.refresh(cx, false);
        Outcome::Handled
    }

    /// Typed or pasted text.
    pub fn type_text<L: Lookup>(&mut self, text: &str, cx: &Context<'_, L>) -> Outcome {
        self.edited(|edit| edit.insert(text), cx)
    }

    /// Inserts the selected suggestion of the list in place (CCMD-LIST-005).
    pub fn insert_suggestion<L: Lookup>(&mut self, cx: &Context<'_, L>) -> Outcome {
        let Some(list) = self.commands.list() else {
            return Outcome::Ignored;
        };
        let caret = list.result_caret();
        let selected = list.selected() as isize;
        self.sent.clear();
        self.sent.push_str(list.result());
        self.edit.set(&self.sent);
        self.edit.set_caret(caret, false);
        self.refresh(cx, true);
        if let Some(list) = self.commands.list_mut() {
            list.select(selected);
            list.set_cycle(true);
        }
        Outcome::Handled
    }

    fn list_key<L: Lookup>(&mut self, key: Key, cx: &Context<'_, L>) -> Option<Outcome> {
        let list = self.commands.list_mut()?;
        match key {
            Key::Up => {
                list.step(-1);
                list.set_cycle(false);
            }
            Key::Down => {
                list.step(1);
                list.set_cycle(false);
            }
            Key::Tab { back } => {
                if list.cycling() {
                    list.step(if back { -1 } else { 1 });
                }
                return Some(self.insert_suggestion(cx));
            }
            Key::Escape => self.commands.hide_list(),
            _ => return None,
        }
        Some(Outcome::Handled)
    }

    /// Handles one key in the order of CHAT-KEY-001.
    pub fn key<L: Lookup>(
        &mut self,
        key: Key,
        history: &mut History,
        cx: &Context<'_, L>,
    ) -> Outcome {
        if let Some(outcome) = self.list_key(key, cx) {
            return outcome;
        }
        match key {
            Key::Tab { .. } => {
                if self.commands.show_list() {
                    Outcome::Handled
                } else {
                    Outcome::Ignored
                }
            }
            Key::Backspace { .. } if self.draft => {
                self.edit.set("");
                self.changed(cx)
            }
            Key::Escape => Outcome::Closed(Closed::Escape),
            Key::Backspace { word } => self.edited(|edit| edit.backspace(word), cx),
            Key::Delete { word } => self.edited(|edit| edit.delete(word), cx),
            Key::Cut => {
                let selected = self.edit.selected().to_owned();
                if self.edit.cut() {
                    self.changed(cx);
                    Outcome::Copy(selected)
                } else {
                    Outcome::Ignored
                }
            }
            Key::Copy => Outcome::Copy(self.edit.selected().to_owned()),
            Key::Left { word, select } => self.moved(|edit| edit.left(word, select)),
            Key::Right { word, select } => self.moved(|edit| edit.right(word, select)),
            Key::Home { select } => self.moved(|edit| edit.home(select)),
            Key::End { select } => self.moved(|edit| edit.end(select)),
            Key::SelectAll => self.moved(EditBox::select_all),
            Key::Enter => self.enter(history, cx),
            Key::PageUp => Outcome::Page(1),
            Key::PageDown => Outcome::Page(-1),
            Key::Up => self.recall(-1, history, cx),
            Key::Down => self.recall(1, history, cx),
        }
    }

    fn edited<L: Lookup>(
        &mut self,
        edit: impl FnOnce(&mut EditBox) -> bool,
        cx: &Context<'_, L>,
    ) -> Outcome {
        if edit(&mut self.edit) {
            self.changed(cx)
        } else {
            Outcome::Ignored
        }
    }

    /// A move of the caret does not parse again (CCMD-PARSE-001, postconditions).
    fn moved(&mut self, edit: impl FnOnce(&mut EditBox)) -> Outcome {
        edit(&mut self.edit);
        Outcome::Handled
    }

    /// Up and Down with no list (CHAT-HIST-002).
    fn recall<L: Lookup>(
        &mut self,
        delta: isize,
        history: &History,
        cx: &Context<'_, L>,
    ) -> Outcome {
        let Some(text) = self.recall.step(delta, history, self.edit.text()) else {
            return Outcome::Ignored;
        };
        self.edit.set(text);
        self.draft = false;
        self.commands.auto = false;
        self.refresh(cx, false);
        Outcome::Handled
    }

    /// The input is blocked: Enter does nothing (CHAT-OPT-002).
    fn blocked<L: Lookup>(&self, cx: &Context<'_, L>) -> bool {
        let input = self.edit.text();
        if input.starts_with('/') {
            !cx.abilities.send_commands
                || (self.commands.has_message() && !cx.abilities.send_messages)
        } else {
            !input.trim().is_empty() && !cx.abilities.send_messages
        }
    }

    /// Enter (CHAT-SEND-002).
    fn enter<L: Lookup>(&mut self, history: &mut History, cx: &Context<'_, L>) -> Outcome {
        if self.blocked(cx) {
            return Outcome::Handled;
        }
        normalize(self.edit.text(), &mut self.sent);
        if self.sent.is_empty() {
            return Outcome::Closed(Closed::Sent {
                send: None,
                history_changed: false,
            });
        }
        let history_changed = history.add(&self.sent);
        let send = match self.sent.strip_prefix('/') {
            Some(line) => Send::Command {
                line: line.to_owned(),
                signed: self.commands.line_has_message(line, cx),
            },
            None => Send::Chat(self.sent.clone()),
        };
        Outcome::Closed(Closed::Sent {
            send: Some(send),
            history_changed,
        })
    }
}
