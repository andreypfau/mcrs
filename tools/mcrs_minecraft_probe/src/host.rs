use crate::PlayerBan;
use mcrs_minecraft_nbt::compound::NbtCompound;
use std::time::Instant;

pub trait Host {
    fn read_player_bans(&mut self) -> anyhow::Result<Vec<PlayerBan>>;

    fn read_player_file(&mut self, player: &str) -> anyhow::Result<NbtCompound>;

    fn console(&mut self, lines: &[String]) -> anyhow::Result<Vec<String>>;

    fn console_before(
        &mut self,
        _lines: &[String],
        _deadline: Instant,
    ) -> anyhow::Result<Vec<String>> {
        anyhow::bail!("this host does not support a console deadline")
    }

    fn restart_server(&mut self) -> anyhow::Result<()>;
}
