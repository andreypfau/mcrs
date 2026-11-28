use mcrs_minecraft_command_engine::Worlds;
use mcrs_minecraft_engine::dim_mail;
use mcrs_minecraft_engine::server_edit::ServerEdit;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerChange(pub ServerEdit);

pub(crate) fn apply(worlds: &mut Worlds<'_>, ServerChange(edit): ServerChange) {
    dim_mail::edit_server(worlds.world_mut(), edit);
}
