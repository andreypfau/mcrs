use std::io::Write;
use crate::{Decode, Encode};

#[derive(Debug, Clone, Copy)]
pub enum GameEventKind {
    NoRespawnBlockAvailable,
    EndRaining,
    BeginRaining,
    ChangeGameMode,
    WinGame(EnterCredits),
    DemoEvent(DemoMessage),
    PlayArrowHitSound,
    RainLevelChange(f32),
    ThunderLevelChange(f32),
    PufferfishSting,
    GuardianElderEffect,
    ImmediateRespawn(bool),
    LimitedCrafting(bool),
    LevelChunksLoadStart
}

impl Encode for GameEventKind {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let a = match self {
            GameEventKind::NoRespawnBlockAvailable => (0, 0f32),
            GameEventKind::EndRaining => (1, 0f32),
            GameEventKind::BeginRaining => (2, 0f32),
            GameEventKind::ChangeGameMode => (3, 0f32),
            GameEventKind::WinGame(credits) => (4, match credits {
                EnterCredits::SeenBefore => 0f32,
                EnterCredits::FirstTime => 1f32,
            }),
            GameEventKind::DemoEvent(message) => (5, match message {
                DemoMessage::Welcome => 0f32,
                DemoMessage::MovementControls => 101f32,
                DemoMessage::JumpControl => 102f32,
                DemoMessage::InventoryControl => 103f32,
                DemoMessage::ScreenshotControl => 104f32,
            }),
            GameEventKind::PlayArrowHitSound => (6, 0f32),
            GameEventKind::RainLevelChange(strength) => (7, *strength),
            GameEventKind::ThunderLevelChange(strength) => (8, *strength),
            GameEventKind::PufferfishSting => (9, 0f32),
            GameEventKind::GuardianElderEffect => (10, 0f32),
            GameEventKind::ImmediateRespawn(immediate) => (11, if *immediate { 1f32 } else { 0f32 }),
            GameEventKind::LimitedCrafting(limited) => (12, if *limited { 1f32 } else { 0f32 }),
            GameEventKind::LevelChunksLoadStart => (13, 0f32),
        };
        i8::encode(&a.0, &mut w)?;
        f32::encode(&a.1, &mut w)?;
        Ok(())
    }
}

impl<'a> Decode<'a> for GameEventKind {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let event_id = i8::decode(r)?;
        let param = f32::decode(r)?;
        match event_id {
            0 => Ok(GameEventKind::NoRespawnBlockAvailable),
            1 => Ok(GameEventKind::EndRaining),
            2 => Ok(GameEventKind::BeginRaining),
            3 => Ok(GameEventKind::ChangeGameMode),
            4 => {
                let credits = match param as i32 {
                    0 => EnterCredits::SeenBefore,
                    1 => EnterCredits::FirstTime,
                    _ => return Err(anyhow::anyhow!("invalid EnterCredits value: {}", param)),
                };
                Ok(GameEventKind::WinGame(credits))
            }
            5 => {
                let message = match param as i32 {
                    0 => DemoMessage::Welcome,
                    101 => DemoMessage::MovementControls,
                    102 => DemoMessage::JumpControl,
                    103 => DemoMessage::InventoryControl,
                    104 => DemoMessage::ScreenshotControl,
                    _ => return Err(anyhow::anyhow!("invalid DemoMessage value: {}", param)),
                };
                Ok(GameEventKind::DemoEvent(message))
            }
            6 => Ok(GameEventKind::PlayArrowHitSound),
            7 => Ok(GameEventKind::RainLevelChange(param)),
            8 => Ok(GameEventKind::ThunderLevelChange(param)),
            9 => Ok(GameEventKind::PufferfishSting),
            10 => Ok(GameEventKind::GuardianElderEffect),
            11 => Ok(GameEventKind::ImmediateRespawn(param != 0.0)),
            12 => Ok(GameEventKind::LimitedCrafting(param != 0.0)),
            13 => Ok(GameEventKind::LevelChunksLoadStart),
            _ => Err(anyhow::anyhow!("invalid GameEventKind id: {}", event_id)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoMessage {
    Welcome,
    MovementControls,
    JumpControl,
    InventoryControl,
    ScreenshotControl
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterCredits {
    SeenBefore,
    FirstTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespawnScreen {
    EnableRespawnScreen,
    ImmediateRespawn,
}