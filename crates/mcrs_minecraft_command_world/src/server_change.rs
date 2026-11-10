use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_engine::dim_mail;
use mcrs_minecraft_engine::server_edit::ServerEdit;

/// The command effect that changes the state of the whole server: this world reads the change
/// at once, and the host and the other worlds after the tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerChange(pub ServerEdit);

pub(crate) fn apply(worlds: &mut Worlds<'_>, ServerChange(edit): ServerChange) {
    dim_mail::edit_server(worlds.world_mut(), edit);
}
