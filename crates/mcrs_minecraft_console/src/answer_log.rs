use std::thread::JoinHandle;

use mcrs_minecraft_text::RawText;

use crate::language::Language;

/// What the log of the console answers takes, in order.
pub(crate) enum LogEntry {
    Language(Language),
    Answer(RawText<'static>),
}

/// Renders the answers to the console in English and writes them to the log, in their order, on
/// its own thread, so that a tick with many answers does not wait for the log. Without the
/// thread it writes them at once.
pub(crate) struct AnswerLog {
    send: Option<flume::Sender<LogEntry>>,
    thread: Option<JoinHandle<()>>,
    language: Language,
}

impl AnswerLog {
    pub(crate) fn spawn() -> Self {
        let (send, receive) = flume::unbounded();
        let started = std::thread::Builder::new()
            .name("console-log".to_owned())
            .spawn(move || {
                let mut language = Language::default();
                for entry in receive.iter() {
                    write(&mut language, entry);
                }
            });
        match started {
            Ok(thread) => Self {
                send: Some(send),
                thread: Some(thread),
                language: Language::default(),
            },
            Err(error) => {
                tracing::warn!(%error, "the console answers go to the log on the tick thread");
                Self {
                    send: None,
                    thread: None,
                    language: Language::default(),
                }
            }
        }
    }

    pub(crate) fn push(&mut self, entry: LogEntry) {
        let entry = match &self.send {
            Some(send) => match send.send(entry) {
                Ok(()) => return,
                Err(flume::SendError(entry)) => entry,
            },
            None => entry,
        };
        write(&mut self.language, entry);
    }
}

impl Drop for AnswerLog {
    fn drop(&mut self) {
        self.send = None;
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("the log of the console answers stopped early");
        }
    }
}

fn write(language: &mut Language, entry: LogEntry) {
    match entry {
        LogEntry::Language(next) => *language = next,
        LogEntry::Answer(answer) => match answer.to_text() {
            Ok(text) => tracing::info!(target: "mcrs_console", "{}", language.render(&text)),
            Err(error) => tracing::error!(%error, "a console answer does not decode"),
        },
    }
}
