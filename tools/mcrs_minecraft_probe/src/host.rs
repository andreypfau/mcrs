use crate::PlayerBan;
use mcrs_minecraft_nbt::compound::NbtCompound;

pub trait Host {
    fn read_player_bans(&mut self) -> anyhow::Result<Vec<PlayerBan>>;

    fn read_player_file(&mut self, player: &str) -> anyhow::Result<NbtCompound>;

    fn console(&mut self, lines: &[String]) -> anyhow::Result<Vec<String>>;

    fn restart_server(&mut self) -> anyhow::Result<()>;
}
