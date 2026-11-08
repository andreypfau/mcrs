use mcrs_minecraft_brain::Part;

/// The keyed random parts of the goals: each names the goal and the draw (GOAL-DEV-004).
pub const FLOAT_JUMP: Part = Part(0x0100);
pub const PANIC_SEARCH: Part = Part(0x0200);
pub const STROLL_WAIT: Part = Part(0x0300);
pub const STROLL_SEARCH: Part = Part(0x0301);
pub const LOOK_AT_PLAYER_WAIT: Part = Part(0x0400);
pub const LOOK_AT_PLAYER_COUNT: Part = Part(0x0401);
pub const LOOK_AROUND_WAIT: Part = Part(0x0500);
pub const LOOK_AROUND_START: Part = Part(0x0501);
