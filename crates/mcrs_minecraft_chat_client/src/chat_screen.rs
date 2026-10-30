use crate::closed::Closed;
use crate::command_input::{CommandInput, Context, Request};
use crate::edit_box::{Anchor, EditBox};
use crate::history::{History, Recall};
use crate::history_change::HistoryChange;
use crate::input_style::InputStyle;
use crate::key::Key;
use crate::outcome::Outcome;
use crate::outgoing::Outgoing;
use crate::signing::Signing;
use crate::tab_direction::TabDirection;
use mcrs_minecraft_chat::{MAX_INPUT, normalize};
use mcrs_minecraft_command_argument_vanilla::VanillaLookup;
use mcrs_minecraft_wire::utf16_len;

/// The chat screen while it is open: the input box, the history position, the draft style and
/// the command state.
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
    /// Opens on `initial` in `style`. The suggestion list does not show until
    /// the player types or presses Tab.
    pub fn open<L: VanillaLookup>(
        &mut self,
        initial: &str,
        style: InputStyle,
        cx: &Context<'_, L>,
    ) {
        self.edit.set(initial);
        self.recall = Recall::default();
        self.draft = style == InputStyle::Draft;
        self.commands.auto = false;
        self.commands.hide_list();
        self.refresh(cx);
    }

    /// The server request the last call made, to send now.
    pub fn take_request(&mut self) -> Option<Request> {
        self.request.take()
    }

    /// The text of the input box.
    pub fn input(&self) -> &str {
        self.edit.text()
    }

    /// The input box.
    pub fn edit(&self) -> &EditBox {
        &self.edit
    }

    /// Whether the input still shows in the draft style.
    pub fn is_draft(&self) -> bool {
        self.draft
    }

    fn refresh<L: VanillaLookup>(&mut self, cx: &Context<'_, L>) {
        if let Some(request) = self
            .commands
            .refresh(self.edit.text(), self.edit.caret(), cx)
        {
            self.request = Some(request);
        }
    }

    /// The player changed the input.
    fn changed<L: VanillaLookup>(&mut self, cx: &Context<'_, L>) -> Outcome {
        self.draft = false;
        self.commands.auto = true;
        self.refresh(cx);
        Outcome::Handled
    }

    /// Typed or pasted text.
    pub fn type_text<L: VanillaLookup>(&mut self, text: &str, cx: &Context<'_, L>) -> Outcome {
        self.edited(|edit| edit.insert(text), cx)
    }

    /// Inserts the selected suggestion of the list in place.
    pub fn insert_suggestion<L: VanillaLookup>(&mut self, cx: &Context<'_, L>) -> Outcome {
        let Some(list) = self.commands.list() else {
            return Outcome::Ignored;
        };
        let caret = list.result_caret();
        let selected = list.selected() as isize;
        self.sent.clear();
        self.sent.push_str(list.result());
        self.edit.set(&self.sent);
        self.edit.set_caret(caret, Anchor::Move);
        if let Some(request) =
            self.commands
                .refresh_after_insert(self.edit.text(), self.edit.caret(), cx)
        {
            self.request = Some(request);
        }
        if let Some(list) = self.commands.list_mut() {
            list.select(selected);
            list.cycle = true;
        }
        Outcome::Handled
    }

    fn list_key<L: VanillaLookup>(&mut self, key: Key, cx: &Context<'_, L>) -> Option<Outcome> {
        let list = self.commands.list_mut()?;
        match key {
            Key::Up => {
                list.step(-1);
                list.cycle = false;
            }
            Key::Down => {
                list.step(1);
                list.cycle = false;
            }
            Key::Tab { direction } => {
                if list.cycle {
                    list.step(match direction {
                        TabDirection::Forward => 1,
                        TabDirection::Backward => -1,
                    });
                }
                return Some(self.insert_suggestion(cx));
            }
            Key::Escape => self.commands.hide_list(),
            _ => return None,
        }
        Some(Outcome::Handled)
    }

    pub fn key<L: VanillaLookup>(
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
            Key::Backspace { step } => self.edited(|edit| edit.backspace(step), cx),
            Key::Delete { step } => self.edited(|edit| edit.delete(step), cx),
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
            Key::Left { step, anchor } => self.moved(|edit| edit.left(step, anchor)),
            Key::Right { step, anchor } => self.moved(|edit| edit.right(step, anchor)),
            Key::Home { anchor } => self.moved(|edit| edit.home(anchor)),
            Key::End { anchor } => self.moved(|edit| edit.end(anchor)),
            Key::SelectAll => self.moved(EditBox::select_all),
            Key::Enter => self.enter(history, cx),
            Key::PageUp => Outcome::Page(1),
            Key::PageDown => Outcome::Page(-1),
            Key::Up => self.recall(-1, history, cx),
            Key::Down => self.recall(1, history, cx),
        }
    }

    fn edited<L: VanillaLookup>(
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

    /// A move of the caret does not parse again.
    fn moved(&mut self, edit: impl FnOnce(&mut EditBox)) -> Outcome {
        edit(&mut self.edit);
        Outcome::Handled
    }

    /// Up and Down with no list.
    fn recall<L: VanillaLookup>(
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
        self.refresh(cx);
        Outcome::Handled
    }

    /// The input is blocked: Enter does nothing.
    fn blocked<L: VanillaLookup>(&self, cx: &Context<'_, L>) -> bool {
        let input = self.edit.text();
        if input.starts_with('/') {
            !cx.abilities.send_commands
                || (self.commands.has_message() && !cx.abilities.send_messages)
        } else {
            !input.trim().is_empty() && !cx.abilities.send_messages
        }
    }

    /// Enter.
    fn enter<L: VanillaLookup>(&mut self, history: &mut History, cx: &Context<'_, L>) -> Outcome {
        if self.blocked(cx) {
            return Outcome::Handled;
        }
        normalize(self.edit.text(), &mut self.sent);
        if self.sent.is_empty() {
            return Outcome::Closed(Closed::Sent {
                send: None,
                history: HistoryChange::Unchanged,
            });
        }
        if !self.sent.starts_with('/') && utf16_len(&self.sent) > MAX_INPUT {
            return Outcome::Handled;
        }
        let history = if history.add(&self.sent) {
            HistoryChange::Changed
        } else {
            HistoryChange::Unchanged
        };
        let send = match self.sent.strip_prefix('/') {
            Some(line) => Outgoing::Command {
                line: line.to_owned(),
                signing: if self.commands.line_has_message(line, cx) {
                    Signing::Signed
                } else {
                    Signing::Unsigned
                },
            },
            None => Outgoing::Chat(self.sent.clone()),
        };
        Outcome::Closed(Closed::Sent {
            send: Some(send),
            history,
        })
    }
}
