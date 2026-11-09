use std::io;
use std::thread::JoinHandle;

use mcrs_minecraft_lang::Language;
use mcrs_minecraft_text::RawText;

use crate::render::render;

/// Renders the answers to the console in English and writes them to the log, in their order, on
/// its own thread, so that a tick with many answers does not wait for the log.
pub(crate) struct AnswerLog {
    send: Option<flume::Sender<RawText<'static>>>,
    thread: Option<JoinHandle<()>>,
}

impl AnswerLog {
    pub(crate) fn spawn(language: Language) -> io::Result<Self> {
        let (send, receive) = flume::unbounded::<RawText<'static>>();
        let thread = std::thread::Builder::new()
            .name("console-log".to_owned())
            .spawn(move || {
                for answer in receive.iter() {
                    write(&language, &answer);
                }
            })?;
        Ok(Self {
            send: Some(send),
            thread: Some(thread),
        })
    }

    pub(crate) fn push(&self, answer: RawText<'static>) {
        let sent = self.send.as_ref().map(|send| send.send(answer));
        if !matches!(sent, Some(Ok(()))) {
            tracing::error!("the log of the console answers stopped; an answer is lost");
        }
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

fn write(language: &Language, answer: &RawText<'static>) {
    match answer.to_text() {
        Ok(text) => tracing::info!(target: "mcrs_console", "{}", render(language, &text)),
        Err(error) => tracing::error!(%error, "a console answer does not decode"),
    }
}
