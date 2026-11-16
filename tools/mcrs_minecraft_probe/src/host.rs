use crate::PlayerBan;
use mcrs_minecraft_nbt::compound::NbtCompound;

/// What the driver asks of the host of the target server.
pub trait Host {
    /// The player ban file of the target as it is now.
    fn read_player_bans(&mut self) -> anyhow::Result<Vec<PlayerBan>>;

    /// The saved player file of the player with this name, as it is on disk now.
    fn read_player_file(&mut self, player: &str) -> anyhow::Result<NbtCompound>;

    /// Stops the server so that it saves, and starts it again on the same world and address.
    /// The driver closes every client before the call.
    fn restart_server(&mut self) -> anyhow::Result<()>;
}
