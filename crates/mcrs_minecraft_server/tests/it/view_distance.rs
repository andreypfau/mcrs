//! A client in play that changes its view distance gets the columns of the new view.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::muxer::muxer_link::MuxerLink;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
use mcrs_minecraft_protocol::setting::{ChatMode, DisplayedSkinParts, MainArm, ParticleStatus};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundClientInformation;

use crate::host_app::{self, Arrival, PlayServer, label_of, sent_to};
use crate::world_server::{OVERWORLD, WorldServer, lockstep_tick, wait_for_generation};

const QUIET_TICKS: usize = 30;
const PATIENCE: Duration = Duration::from_secs(20);

fn client_information(view_distance: u8) -> ServerboundClientInformation<'static> {
    ServerboundClientInformation(ClientInformation {
        locale: "en_us",
        view_distance,
        chat_mode: ChatMode::Enabled,
        chat_colors: true,
        displayed_skin_parts: DisplayedSkinParts::from_bits(0x7f),
        main_arm: MainArm::Right,
        enable_text_filtering: false,
        allow_server_listings: true,
        particle_status: ParticleStatus::All,
    })
}

/// The columns that a client got and lost while its view settled, in the order they came.
#[derive(Default)]
struct Shift {
    loaded: Vec<(i32, i32)>,
    forgotten: Vec<(i32, i32)>,
}

/// Ticks, the client taking each batch, until the view of `session` changed and then stayed
/// still for [`QUIET_TICKS`] ticks.
fn settle(
    app: &mut App,
    label: Entity,
    session: PlayerSession,
    held: &mut BTreeSet<(i32, i32)>,
) -> Shift {
    let deadline = Instant::now() + PATIENCE;
    let mut shift = Shift::default();
    let mut quiet = 0;
    while shift.loaded.is_empty() && shift.forgotten.is_empty() || quiet < QUIET_TICKS {
        assert!(Instant::now() < deadline, "the view never settled");
        quiet += 1;
        app.world_mut()
            .resource_mut::<MuxerLink>()
            .acknowledge_batch(session, 64.0);
        for packet in sent_to(session, lockstep_tick(app, label)) {
            match packet {
                PacketPayload::ChunkLoad { column, .. } => {
                    held.insert((column.x, column.z));
                    shift.loaded.push((column.x, column.z));
                    quiet = 0;
                }
                PacketPayload::ChunkUnload(forget) => {
                    held.remove(&(forget.x, forget.z));
                    shift.forgotten.push((forget.x, forget.z));
                    quiet = 0;
                }
                _ => {}
            }
        }
    }
    shift
}

#[test]
fn a_client_information_in_play_shrinks_and_grows_the_view() {
    let app = WorldServer::default().start();
    let label = label_of(&app, OVERWORLD);
    let mut server = PlayServer { app, dim: label };
    wait_for_generation(&mut server.app, label);
    let arrival = Arrival {
        uuid: Uuid::from_u128(1),
        name: "Viewer",
        position: DVec3::new(8.5, 100.0, 8.5),
        view_distance: 4,
    };
    let anchor = host_app::admit(&mut server.app, label, &arrival);
    let session = server.session_of(anchor).expect("a placed session");
    let mut held = BTreeSet::new();
    settle(&mut server.app, label, session, &mut held);
    let wide = held.clone();

    // A dragged slider sends several in one tick; only the last one counts.
    server.queue(anchor, &client_information(8));
    server.queue(anchor, &client_information(2));
    let shrunk = settle(&mut server.app, label, session, &mut held);
    assert_eq!(shrunk.loaded, [], "a smaller view sends no column");
    let lost: BTreeSet<(i32, i32)> = shrunk.forgotten.iter().copied().collect();
    assert_eq!(
        lost.len(),
        shrunk.forgotten.len(),
        "each column is forgotten once"
    );
    assert_eq!(
        lost,
        &wide - &held,
        "the client forgets what it held outside the view"
    );
    assert!(
        held.iter().all(|&(x, z)| x.abs() <= 3 && z.abs() <= 3),
        "the client keeps only the view of 2 and its ring: {held:?}"
    );
    let narrow = held.clone();

    server.queue(anchor, &client_information(4));
    let grown = settle(&mut server.app, label, session, &mut held);
    assert_eq!(grown.forgotten, [], "a larger view forgets no column");
    assert_eq!(held, wide, "the view of 4 comes back");
    let mut sent = grown.loaded;
    sent.sort_unstable();
    let ring: Vec<(i32, i32)> = (&wide - &narrow).into_iter().collect();
    assert_eq!(sent, ring, "only the columns that entered the view go out");
}
