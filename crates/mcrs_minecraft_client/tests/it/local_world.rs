use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use mcrs_minecraft_client::local_server::LocalServerPlugin;
use mcrs_minecraft_client::test_support::{self, ServerAnswers};
use mcrs_minecraft_client_entity::ClientEntityPlugin;
use mcrs_minecraft_client_world::chat::ClientChatPlugin;
use mcrs_minecraft_client_world::columns::ColumnCachePlugin;
use mcrs_minecraft_client_world::game_mode::GameModePlugin;
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_engine_world::world::dimension_builders::{DimensionBuild, DimensionBuilders};
use mcrs_minecraft_game_mode::{Abilities, GameMode, PlayerGameMode};
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_network::client::ClientNetworkPlugin;
use mcrs_minecraft_protocol::{Bounded, WritePacket};
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_save::SaveLock;
use mcrs_minecraft_server_settings::ServerSettings;

use crate::boot::boot;
use crate::stacks::{self, Entry, Stack};

pub(crate) fn client() -> App {
    local_client(|_| {}, |_| {})
}

fn local_client(extend_server: fn(&mut App), extra: impl FnOnce(&mut App)) -> App {
    boot(move |app| {
        app.add_plugins((
            ClientNetworkPlugin {
                holds_vanilla_pack: true,
                play: Some(GAME_CLIENTBOUND),
            },
            ColumnCachePlugin::default(),
            GameModePlugin,
            ClientEntityPlugin,
            ClientChatPlugin {
                command_history: None,
            },
            LocalServerPlugin {
                asset_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_owned(),
                data_pack: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
                ops_file: None,
                username: "Player".to_owned(),
                open_to_lan: false,
                settings: ServerSettings::default(),
                column_traces: None,
                extend_server,
            },
        ))
        .init_resource::<Time<Real>>()
        .insert_resource(RequestedViewDistance(2));
        extra(app);
    })
}

pub(crate) fn world_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("mcrs-{name}-{}", std::process::id()));
    fs::remove_dir_all(&folder).ok();
    folder
}

pub(crate) const ENTRIES: std::ops::RangeInclusive<u16> = 9..=45;

pub(crate) struct LocalWorld {
    pub(crate) app: App,
    pub(crate) viewer: Entity,
    folder: PathBuf,
    expected: Vec<Option<Stack>>,
}

impl LocalWorld {
    pub(crate) fn join(
        name: &str,
        extend_server: fn(&mut App),
        extra: impl FnOnce(&mut App),
    ) -> Self {
        let folder = world_folder(name);
        let mut app = local_client(extend_server, extra);
        test_support::count_server_answers(&mut app);
        test_support::open(&mut app, &folder);
        test_support::update_until(&mut app, test_support::joined);
        test_support::update_until(&mut app, |app| {
            app.world_mut().run_schedule(FixedUpdate);
            app.world().resource::<ServerAnswers>().contents > 0
                && test_support::creative_player(app.world_mut()).is_some()
        });
        test_support::round_trip(&mut app);
        let viewer = test_support::creative_player(app.world_mut()).unwrap();
        Self {
            app,
            viewer,
            folder,
            expected: ENTRIES.map(|_| None).collect(),
        }
    }

    pub(crate) fn give(&mut self, entries: &[Entry]) {
        for given in entries {
            let value = stacks::value(self.app.world(), given.stack);
            test_support::give(self.app.world_mut(), self.viewer, given.entry, value);
            self.expect(*given);
        }
    }

    pub(crate) fn put_local(&mut self, entries: &[Entry]) {
        for local in entries {
            let value = stacks::value(self.app.world(), local.stack);
            test_support::put_local(self.app.world_mut(), self.viewer, local.entry, value);
        }
    }

    pub(crate) fn switch_game_mode(&mut self, game_mode: GameMode) {
        let (app, viewer) = (&mut self.app, self.viewer);
        if app.world().get::<PlayerGameMode>(viewer) == Some(&PlayerGameMode(game_mode)) {
            return;
        }
        test_support::round_trip(app);
        let command = match game_mode {
            GameMode::Survival => "gamemode survival",
            GameMode::Creative => "gamemode creative",
            GameMode::Adventure => "gamemode adventure",
            GameMode::Spectator => "gamemode spectator",
        };
        test_support::connection(app.world_mut()).write_packet(&ServerboundChatCommand {
            command: Bounded(command),
        });
        let creative = game_mode == GameMode::Creative;
        test_support::update_until(app, |app| {
            let world = app.world();
            world.get::<PlayerGameMode>(viewer) == Some(&PlayerGameMode(game_mode))
                && world
                    .get::<Abilities>(viewer)
                    .is_some_and(|abilities| abilities.instant_build == creative)
        });
    }

    pub(crate) fn clear_answers(&mut self) {
        *self.app.world_mut().resource_mut::<ServerAnswers>() = ServerAnswers::default();
    }

    pub(crate) fn assert_after_round_trip(
        &mut self,
        name: &str,
        answers: Option<ServerAnswers>,
        entries: &[Entry],
        carried: Option<Stack>,
    ) {
        test_support::round_trip(&mut self.app);
        for entry in entries {
            self.expect(*entry);
        }
        let world = self.app.world();
        if let Some(answers) = answers {
            assert_eq!(
                *world.resource::<ServerAnswers>(),
                answers,
                "{name}: the packets of the server in answer"
            );
        }
        let found: Vec<Option<Stack>> = ENTRIES
            .map(|entry| stacks::stack_at(world, self.viewer, test_support::player_slot(entry)))
            .collect();
        assert_eq!(found, self.expected, "{name}: the entries 9 to 45");
        assert_eq!(
            stacks::cursor(world, self.viewer),
            carried,
            "{name}: the cursor"
        );
    }

    fn expect(&mut self, entry: Entry) {
        let index = usize::from(entry.entry - ENTRIES.start());
        *self
            .expected
            .get_mut(index)
            .expect("the test names only the entries 9 to 45") = entry.stack;
    }

    pub(crate) fn leave(mut self) {
        self.app.world_mut().write_message(AppExit::Success);
        self.app.update();
        let lock = SaveLock::take(&self.folder).expect("the server released the world");
        drop(lock);
        fs::remove_dir_all(&self.folder).ok();
    }
}

pub(crate) fn discard_thrown_items(server: &mut App) {
    server
        .world_mut()
        .get_resource_or_init::<DimensionBuilders>()
        .add(add_item_discard);
}

fn add_item_discard(_: &World, _: &DimensionBuild<'_>, dimension: &mut SubApp) {
    dimension.add_systems(FixedUpdate, discard_items);
}

fn discard_items(items: Query<Entity, With<ItemEntity>>, mut commands: Commands) {
    for item in &items {
        commands.entity(item).despawn();
    }
}
