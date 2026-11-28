use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_client_world::camera_section_ready::CameraSectionReady;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_client_world::screen::{Screen, ScreenMessage};
use mcrs_minecraft_client_world::session::{
    LevelChunksLoadStarted, LocalServer, LocalServerError, LocalServerPhase, LocalServerStopped,
    LocalWorldOrigin,
};
use mcrs_minecraft_client_world::world_list::{WorldStoreError, WorldStoreFailed};
use mcrs_minecraft_network::client::{ConnectionEnd, ConnectionEnded};

use crate::cursor_grab::grab;
use crate::title_draw::TitleScreenState;
use mcrs_minecraft_font::ClientLanguage;

const LOADING_TIMEOUT: Duration = Duration::from_secs(30);
const CREATED_WORLD_DELAY: Duration = Duration::from_millis(500);

#[derive(Resource, Default)]
pub(crate) struct DisconnectReason(pub(crate) Option<Reason>);

#[derive(Clone, Debug)]
pub(crate) enum Reason {
    Connection(ConnectionEnd),
    LocalServer(LocalServerError),
    WorldStore(WorldStoreError),
}

impl Reason {
    pub(crate) fn title_key(&self) -> &'static str {
        match self {
            Self::Connection(ConnectionEnd::JoinFailed(_)) => "connect.failed",
            Self::Connection(_) => "disconnect.lost",
            Self::LocalServer(_) | Self::WorldStore(_) => "selectWorld.access_failure",
        }
    }

    pub(crate) fn text(&self, language: &ClientLanguage) -> String {
        match self {
            Self::Connection(ConnectionEnd::Kicked(text)) => language.plain_text(text),
            Self::Connection(ConnectionEnd::JoinFailed(cause) | ConnectionEnd::Lost(cause)) => {
                cause.clone()
            }
            Self::Connection(end) => end.to_string(),
            Self::LocalServer(LocalServerError::Held) | Self::WorldStore(WorldStoreError::Held) => {
                language.get("selectWorld.locked").to_owned()
            }
            Self::LocalServer(error) => error.to_string(),
            Self::WorldStore(error) => error.to_string(),
        }
    }
}

#[derive(Default)]
pub(crate) struct LoadingTimes {
    joined_at: Option<Duration>,
    ready_at: Option<Duration>,
}

#[derive(SystemParam)]
pub(crate) struct SessionState<'w, 's> {
    local: Option<Res<'w, LocalServer>>,
    joined: Query<'w, 's, Has<LevelChunksLoadStarted>, With<JoinedGame>>,
    camera_ready: Option<Res<'w, CameraSectionReady>>,
    ended: MessageReader<'w, 's, ConnectionEnded>,
    stopped: MessageReader<'w, 's, LocalServerStopped>,
    time: Res<'w, Time<Real>>,
}

pub(crate) fn follow_session(
    mut screen: ResMut<Screen>,
    mut session: SessionState,
    mut reason: ResMut<DisconnectReason>,
    mut title: ResMut<TitleScreenState>,
    cursor: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
    mut loading: Local<LoadingTimes>,
) {
    let shown = *screen;
    if !matches!(
        shown,
        Screen::Disconnected | Screen::Message(ScreenMessage::SavingWorld)
    ) {
        reason.0 = None;
    }
    let next = ended_screen(&mut session, &mut reason)
        .or_else(|| stopped_screen(&mut session, &mut reason))
        .or_else(|| entry_screen(shown, &session, &mut loading));
    let Some(next) = next else {
        return;
    };
    if next != Screen::LevelLoading {
        *loading = LoadingTimes::default();
    }
    if next == Screen::Title {
        title.end_fade();
    }
    if let Some(mut cursor) = cursor {
        if next == Screen::None {
            grab(&mut cursor, true);
        } else if shown == Screen::None {
            grab(&mut cursor, false);
        }
    }
    screen.set_if_neq(next);
}

fn ended_screen(session: &mut SessionState, reason: &mut DisconnectReason) -> Option<Screen> {
    let mut next = None;
    let starting = session
        .local
        .as_deref()
        .is_some_and(|local| local.phase == LocalServerPhase::Starting);
    for ended in session.ended.read() {
        if starting {
            continue;
        }
        if !matches!(ended.reason, ConnectionEnd::Left) && reason.0.is_none() {
            reason.0 = Some(Reason::Connection(ended.reason.clone()));
        }
        next = Some(if session.local.is_some() {
            Screen::Message(ScreenMessage::SavingWorld)
        } else {
            after_session(reason)
        });
    }
    next
}

fn stopped_screen(session: &mut SessionState, reason: &mut DisconnectReason) -> Option<Screen> {
    let joined = !session.joined.is_empty();
    let mut next = None;
    for LocalServerStopped(result) in session.stopped.read() {
        if joined {
            continue;
        }
        if let Err(error) = result
            && reason.0.is_none()
        {
            reason.0 = Some(Reason::LocalServer(error.clone()));
        }
        next = Some(after_session(reason));
    }
    next
}

fn after_session(reason: &DisconnectReason) -> Screen {
    if reason.0.is_some() {
        Screen::Disconnected
    } else {
        Screen::Title
    }
}

fn entry_screen(
    shown: Screen,
    session: &SessionState,
    loading: &mut LoadingTimes,
) -> Option<Screen> {
    let local = session.local.as_deref();
    match shown {
        Screen::Title | Screen::SelectWorld | Screen::CreateWorld(_) => match local {
            Some(local) if local.phase != LocalServerPhase::Stopping => {
                Some(Screen::Message(match local.origin {
                    LocalWorldOrigin::Opened => ScreenMessage::ReadingWorldData,
                    LocalWorldOrigin::Created => ScreenMessage::PreparingWorldCreation,
                }))
            }
            None if !session.joined.is_empty() => {
                loading.joined_at = Some(session.time.elapsed());
                Some(Screen::LevelLoading)
            }
            _ => None,
        },
        Screen::Message(
            ScreenMessage::ReadingWorldData | ScreenMessage::PreparingWorldCreation,
        ) => local
            .is_some_and(|local| local.phase == LocalServerPhase::Running)
            .then_some(Screen::LevelLoading),
        Screen::LevelLoading => level_loading_closes(session, loading).then_some(Screen::None),
        _ => None,
    }
}

fn level_loading_closes(session: &SessionState, loading: &mut LoadingTimes) -> bool {
    let now = session.time.elapsed();
    let Some(chunks_started) = session.joined.iter().next() else {
        *loading = LoadingTimes::default();
        return false;
    };
    let joined_at = *loading.joined_at.get_or_insert(now);
    let camera_ready = session.camera_ready.as_deref().is_some_and(|ready| ready.0)
        || now.saturating_sub(joined_at) >= LOADING_TIMEOUT;
    if !(chunks_started && camera_ready) {
        return false;
    }
    let ready_at = *loading.ready_at.get_or_insert(now);
    let delay = match session.local.as_deref().map(|local| local.origin) {
        Some(LocalWorldOrigin::Created) => CREATED_WORLD_DELAY,
        Some(LocalWorldOrigin::Opened) | None => Duration::ZERO,
    };
    now.saturating_sub(ready_at) >= delay
}

pub(crate) fn show_failed_start(
    mut failures: MessageReader<WorldStoreFailed>,
    mut screen: ResMut<Screen>,
    mut reason: ResMut<DisconnectReason>,
) {
    let failed = failures.read().find_map(|failure| match failure {
        WorldStoreFailed::Create(error) | WorldStoreFailed::Play { error, .. } => Some(*error),
        WorldStoreFailed::Delete { .. } => None,
    });
    if let Some(error) = failed {
        reason.0 = Some(Reason::WorldStore(error));
        screen.set_if_neq(Screen::Disconnected);
    }
}

#[cfg(test)]
mod tests {
    mod jar {
        use bevy::window::CursorGrabMode;
        use mcrs_minecraft_client_widget::{GuiRect, WidgetId};
        use mcrs_minecraft_client_world::player::Player;

        use super::super::*;
        use crate::corpus_font::corpus_font;
        use crate::corpus_language::corpus_language;
        use crate::disconnected_screen::{self, disconnected_screen};
        use crate::gui_quad::WHITE;
        use crate::level_loading_screen::level_loading_screen;
        use crate::menu_test_app::{GUI, advance, caption, click, game_app, grab_mode, screen};
        use crate::message_screen;
        use mcrs_minecraft_font::Span;

        fn join(app: &mut App) -> Entity {
            app.world_mut()
                .spawn((
                    JoinedGame {
                        player_id: 0,
                        dimensions: Vec::new(),
                    },
                    LevelChunksLoadStarted,
                ))
                .id()
        }

        fn end(app: &mut App, session: Entity, reason: ConnectionEnd) {
            app.world_mut().despawn(session);
            app.world_mut().write_message(ConnectionEnded { reason });
            app.update();
        }

        #[test]
        fn the_screens_follow_the_session_into_a_world_and_out_of_it() {
            let mut app = game_app();
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<Player>>()
                .single(app.world())
                .unwrap();
            app.world_mut().despawn(player);
            app.insert_resource(Screen::Title)
                .insert_resource(CameraSectionReady(false));
            advance(&mut app, 0);

            app.insert_resource(LocalServer {
                phase: LocalServerPhase::Starting,
                origin: LocalWorldOrigin::Created,
            });
            app.update();
            let preparing = Screen::Message(ScreenMessage::PreparingWorldCreation);
            assert_eq!(screen(&app), preparing);
            app.world_mut()
                .resource_mut::<LocalServer>()
                .map_unchanged(|resource| &mut resource.phase)
                .set_if_neq(LocalServerPhase::Running);
            app.update();
            assert_eq!(screen(&app), Screen::LevelLoading);
            assert_eq!(caption(&app, WidgetId(0)), "Loading terrain...");
            let session = join(&mut app);
            advance(&mut app, 10_000);
            assert_eq!(screen(&app), Screen::LevelLoading);
            app.insert_resource(CameraSectionReady(true));
            advance(&mut app, 0);
            advance(&mut app, 499);
            assert_eq!(screen(&app), Screen::LevelLoading);
            advance(&mut app, 1);
            assert_eq!(screen(&app), Screen::None);
            assert_eq!(grab_mode(&mut app), CursorGrabMode::Locked);

            end(&mut app, session, ConnectionEnd::Left);
            assert_eq!(screen(&app), Screen::Message(ScreenMessage::SavingWorld));
            assert_eq!(grab_mode(&mut app), CursorGrabMode::None);
            app.world_mut().remove_resource::<LocalServer>();
            app.world_mut().write_message(LocalServerStopped(Ok(())));
            app.update();
            assert_eq!(screen(&app), Screen::Title);

            app.insert_resource(CameraSectionReady(false));
            let session = join(&mut app);
            app.update();
            assert_eq!(screen(&app), Screen::LevelLoading);
            advance(&mut app, 29_999);
            assert_eq!(screen(&app), Screen::LevelLoading);
            advance(&mut app, 1);
            assert_eq!(screen(&app), Screen::None);
            end(
                &mut app,
                session,
                ConnectionEnd::Lost("Timed out".to_owned()),
            );
            let shown = |app: &App| {
                (
                    screen(app),
                    caption(app, disconnected_screen::TITLE),
                    caption(app, disconnected_screen::REASON),
                )
            };
            let disconnected = |title: &str, reason: &str| {
                (Screen::Disconnected, title.to_owned(), reason.to_owned())
            };
            assert_eq!(shown(&app), disconnected("Connection Lost", "Timed out"));
            click(&mut app, 150, 145);
            assert_eq!(screen(&app), Screen::Title);

            app.world_mut()
                .write_message(LocalServerStopped(Err(LocalServerError::Held)));
            app.update();
            let locked = disconnected(
                "Failed to access world",
                "Locked by another running instance of Minecraft.",
            );
            assert_eq!(shown(&app), locked);
            let late = app.world_mut().spawn_empty().id();
            end(&mut app, late, ConnectionEnd::Lost("Timed out".to_owned()));
            assert_eq!(shown(&app), locked, "the screen keeps its reason");
            click(&mut app, 150, 145);

            let session = app.world_mut().spawn_empty().id();
            end(
                &mut app,
                session,
                ConnectionEnd::JoinFailed("Connection refused".to_owned()),
            );
            assert_eq!(
                shown(&app),
                disconnected("Failed to connect to the server", "Connection refused")
            );
        }

        #[test]
        fn a_local_server_that_stops_during_play_leaves_the_end_of_the_connection_to_decide() {
            let mut app = game_app();
            app.insert_resource(Screen::None);
            let session = join(&mut app);
            app.world_mut()
                .write_message(LocalServerStopped(Err(LocalServerError::Panicked)));
            app.update();
            assert_eq!(screen(&app), Screen::None);
            end(
                &mut app,
                session,
                ConnectionEnd::Lost("Connection closed".to_owned()),
            );
            assert_eq!(screen(&app), Screen::Disconnected);
            assert_eq!(caption(&app, disconnected_screen::TITLE), "Connection Lost");
            assert_eq!(
                caption(&app, disconnected_screen::REASON),
                "Connection closed"
            );
        }

        #[test]
        fn the_session_screens_sit_where_their_rules_place_them() {
            let language = corpus_language();
            let font = corpus_font();
            let saving = language.get(message_screen::message_key(ScreenMessage::SavingWorld));
            let message = message_screen::message_box(GUI, font.str_width(saving));
            let loading = level_loading_screen(GUI, language, font);
            let disconnected = disconnected_screen(
                GUI,
                language,
                font,
                "disconnect.lost",
                &[Span::new("Timed out", WHITE)],
            )
            .unwrap();
            let placed: Vec<(&str, GuiRect)> = [
                ("Saving world", message.rect),
                (
                    "Saving world",
                    GuiRect::new(message.text.x, message.text.y, font.str_width(saving), 9),
                ),
            ]
            .into_iter()
            .chain(
                std::iter::once(&loading)
                    .chain(&disconnected)
                    .map(|widget| (widget.caption.as_str(), widget.rect())),
            )
            .collect();
            let [title, reason, back] = &placed[3..] else {
                panic!("three widgets");
            };
            assert_eq!(
                placed[..3],
                [
                    ("Saving world", GuiRect::new(170, 116, 87, 33)),
                    ("Saving world", GuiRect::new(182, 128, 63, 9)),
                    ("Loading terrain...", GuiRect::new(171, 70, 84, 9)),
                ]
            );
            assert_eq!((title.0, title.1.y), ("Connection Lost", 89));
            assert_eq!((reason.0, reason.1.y), ("Timed out", 118));
            assert_eq!(
                *back,
                ("Back to Title Screen", GuiRect::new(113, 139, 200, 20))
            );
        }
    }
}
