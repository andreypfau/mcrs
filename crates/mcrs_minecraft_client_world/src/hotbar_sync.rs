use bevy::prelude::*;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_inventory_client::InventoryClientSet;
use mcrs_minecraft_item::{LAST_HOTBAR_SLOT, SelectedHotbarSlot};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundLogin, ClientboundSetHeldSlot};
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCarriedItem;

use crate::local_player::LocalPlayerTick;
use crate::player::Player;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LastSentHotbarSlot(pub(crate) u8);

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SendSelectedSlot;

pub struct HotbarSyncPlugin;

impl Plugin for HotbarSyncPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_login)
            .add_observer(on_set_held_slot)
            .add_systems(
                FixedUpdate,
                (
                    send_selected_slot
                        .in_set(SendSelectedSlot)
                        .before(LocalPlayerTick)
                        .run_if(any_with_component::<ClientConnection>),
                    project_selected,
                )
                    .before(InventoryClientSet::Predict),
            )
            .add_systems(
                Update,
                project_selected
                    .after(ClientNetworkSystems::Receive)
                    .before(InventoryClientSet::Predict),
            );
    }
}

fn on_login(
    event: On<Received<ClientboundLogin>>,
    mut player: Query<(&mut SelectedHotbarSlot, &mut LastSentHotbarSlot), With<Player>>,
) {
    if event.packet().is_none() {
        return;
    }
    if let Ok((mut selected, mut last_sent)) = player.single_mut() {
        selected.set_if_neq(SelectedHotbarSlot(0));
        last_sent.set_if_neq(LastSentHotbarSlot(0));
    }
}

fn on_set_held_slot(
    event: On<Received<ClientboundSetHeldSlot>>,
    mut player: Query<&mut SelectedHotbarSlot, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Some(slot) = u8::try_from(packet.slot.0)
        .ok()
        .filter(|slot| *slot <= LAST_HOTBAR_SLOT)
    else {
        return;
    };
    if let Ok(mut selected) = player.single_mut() {
        selected.set_if_neq(SelectedHotbarSlot(slot));
    }
}

fn send_selected_slot(
    player: Single<(&SelectedHotbarSlot, &mut LastSentHotbarSlot), With<Player>>,
    connection: Single<(&mut ClientConnection, &ConnectionState)>,
) {
    let (mut connection, state) = connection.into_inner();
    if *state != ConnectionState::Game {
        return;
    }
    let (selected, mut last_sent) = player.into_inner();
    if selected.0 == last_sent.0 {
        return;
    }
    connection.write_packet(&ServerboundSetCarriedItem {
        slot: u16::from(selected.0),
    });
    *last_sent = LastSentHotbarSlot(selected.0);
}

fn project_selected(
    mut players: Query<(Entity, &SelectedHotbarSlot), Changed<SelectedHotbarSlot>>,
    mut commands: Commands,
) {
    for (player, slot) in &mut players {
        commands.entity(player).try_insert(Selected(slot.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_session::{TestSession, login};
    use bevy::math::DVec3;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_protocol::VarInt;

    fn slots(session: &TestSession) -> (u8, u8) {
        let world = session.app.world();
        (
            world.get::<SelectedHotbarSlot>(session.player).unwrap().0,
            world.get::<LastSentHotbarSlot>(session.player).unwrap().0,
        )
    }

    #[test]
    fn set_held_slot_selects_a_hotbar_slot_and_a_login_resets_both_slots() {
        let mut session = TestSession::new(DVec3::ZERO);
        session.app.add_plugins(HotbarSyncPlugin);
        session.receive(login(GameMode::Survival));
        for (value, selected) in [(4, 4), (9, 4), (-1, 4), (100, 4), (8, 8), (0, 0), (6, 6)] {
            session.receive(ClientboundSetHeldSlot {
                slot: VarInt(value),
            });
            assert_eq!(slots(&session), (selected, 0), "set_held_slot {value}");
        }
        session
            .app
            .world_mut()
            .entity_mut(session.player)
            .insert(LastSentHotbarSlot(6));
        session.receive(login(GameMode::Survival));
        assert_eq!(slots(&session), (0, 0), "after a login");
    }

    #[cfg(not(target_family = "wasm"))]
    mod wire {
        use super::*;
        use crate::test_session::{Joined, Server, joined, receive};
        use mcrs_minecraft_protocol::{Decode, Packet};
        use mcrs_minecraft_protocol_game::serverbound::ServerboundClientTickEnd;

        fn carried_items(server: &mut Server, ticks: usize) -> Vec<Vec<u16>> {
            let mut sent = vec![Vec::new()];
            while sent.len() <= ticks {
                let frame = server.next_frame();
                if frame.id == ServerboundSetCarriedItem::ID {
                    let packet = ServerboundSetCarriedItem::decode(&mut &frame.body[..]).unwrap();
                    sent.last_mut().unwrap().push(packet.slot);
                } else if frame.id == ServerboundClientTickEnd::ID {
                    sent.push(Vec::new());
                }
            }
            sent.pop();
            sent
        }

        #[test]
        fn a_slot_from_the_server_goes_back_once() {
            let mut app = TestSession::new(DVec3::ZERO).app;
            app.add_plugins(HotbarSyncPlugin);
            let Joined {
                mut app,
                mut server,
                connection,
            } = joined(app);
            receive(&mut app, connection, login(GameMode::Survival));
            receive(
                &mut app,
                connection,
                ClientboundSetHeldSlot { slot: VarInt(4) },
            );
            for _ in 0..2 {
                app.world_mut().run_schedule(FixedUpdate);
            }
            app.update();
            assert_eq!(carried_items(&mut server, 2), [vec![4], vec![]]);
        }
    }
}
