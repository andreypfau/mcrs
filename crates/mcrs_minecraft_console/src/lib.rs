//! The server console: each line of the standard input is a command that runs as the server, at
//! the highest command level, in the overworld. The answers to it go to the log in English.

// std: a thread reads the standard input.
mod answer_log;
mod console_answers;
mod console_lines;
mod console_plugin;
mod language;

pub use console_answers::log_console_answers;
pub use console_plugin::ConsolePlugin;
pub use language::Language;
