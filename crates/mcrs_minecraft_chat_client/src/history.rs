use std::collections::VecDeque;

const MAX_HISTORY: usize = 100;
const MAX_COMMANDS: usize = 50;

#[derive(Clone, Debug, Default)]
pub struct History {
    entries: VecDeque<String>,
    commands: VecDeque<String>,
}

fn push_bounded(list: &mut VecDeque<String>, text: &str, max: usize) -> bool {
    if list.back().is_some_and(|last| last == text) {
        return false;
    }
    if list.len() >= max {
        list.pop_front();
    }
    list.push_back(text.to_owned());
    true
}

impl History {
    pub fn from_command_file(file: &str) -> Self {
        let mut history = Self::default();
        for line in file.lines().filter(|line| !line.is_empty()) {
            push_bounded(&mut history.commands, line, MAX_COMMANDS);
        }
        history.reset();
        history
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(String::as_str)
    }

    pub fn add(&mut self, text: &str) -> bool {
        push_bounded(&mut self.entries, text, MAX_HISTORY);
        text.starts_with('/') && push_bounded(&mut self.commands, text, MAX_COMMANDS)
    }

    pub fn reset(&mut self) {
        self.entries.clone_from(&self.commands);
    }

    pub fn command_file(&self) -> String {
        let mut file = String::new();
        for command in &self.commands {
            file.push_str(command);
            file.push('\n');
        }
        file
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Recall {
    position: Option<usize>,
    saved: String,
}

impl Recall {
    pub fn step<'h>(
        &'h mut self,
        delta: isize,
        history: &'h History,
        input: &str,
    ) -> Option<&'h str> {
        let size = history.len();
        let current = self.position.unwrap_or(size).min(size);
        let next = current.saturating_add_signed(delta).min(size);
        if next == current {
            return None;
        }
        if current == size {
            self.saved.clear();
            self.saved.push_str(input);
        }
        self.position = Some(next);
        if next == size {
            Some(&self.saved)
        } else {
            history.get(next)
        }
    }
}
