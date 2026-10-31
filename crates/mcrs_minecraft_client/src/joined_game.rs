use anyhow::Context as _;
use bevy::prelude::*;
use mcrs_minecraft_core::{Id, ResourceKey};
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_network::client::close_connection_later;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundLogin, ClientboundRespawn, ClientboundSetEntityData,
};
use mcrs_minecraft_registries::{CurrentRegistries, Namer, Registries};

#[derive(Component, Clone, Debug)]
pub struct JoinedGame {
    pub player_id: i32,
    pub dimensions: Vec<ResourceKey<Dimension>>,
}

/// The dimension the player is in. Written only when a login or a respawn
/// names it, as a whole.
#[derive(Component, Clone, Debug)]
pub struct CurrentDimension {
    pub key: ResourceKey<Dimension>,
    pub dimension_type: Id<DimensionType>,
}

impl CurrentDimension {
    pub fn of(spawn: &PlayerSpawnInfo) -> Self {
        CurrentDimension {
            key: spawn.dimension.clone(),
            dimension_type: spawn.dimension_type_id,
        }
    }
}

pub(crate) fn add_observers(app: &mut App) {
    app.add_observer(on_login)
        .add_observer(on_respawn)
        .add_observer(on_set_entity_data);
}

fn refuse_packet(commands: &mut Commands, connection: Entity, name: &str, error: anyhow::Error) {
    close_connection_later(
        commands,
        connection,
        format!("the server sent a {name} the client cannot read: {error:#}"),
    );
}

fn in_session(registries: Option<&Registries>, spawn: &PlayerSpawnInfo) -> anyhow::Result<()> {
    let id = spawn.dimension_type_id;
    anyhow::ensure!(
        registries.is_some_and(|registries| Namer::name(registries, id).is_some()),
        "dimension type {} is not an entry of the session registries",
        id.number()
    );
    Ok(())
}

fn on_login(
    event: On<Received<ClientboundLogin>>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let Some(login) = event.packet() else {
        return;
    };
    let session = registries.as_deref().map(|registries| &**registries);
    match in_session(session, &login.player_spawn_info) {
        Ok(()) => {
            commands.entity(event.entity).insert((
                JoinedGame {
                    player_id: login.player_id,
                    dimensions: login.dimensions,
                },
                CurrentDimension::of(&login.player_spawn_info),
            ));
        }
        Err(error) => refuse_packet(&mut commands, event.entity, ClientboundLogin::NAME, error),
    }
}

fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let Some(respawn) = event.packet() else {
        return;
    };
    let session = registries.as_deref().map(|registries| &**registries);
    match in_session(session, &respawn.player_spawn_info) {
        Ok(()) => {
            commands
                .entity(event.entity)
                .insert(CurrentDimension::of(&respawn.player_spawn_info));
        }
        Err(error) => refuse_packet(&mut commands, event.entity, ClientboundRespawn::NAME, error),
    }
}

fn on_set_entity_data(
    event: On<Received<ClientboundSetEntityData<'static>>>,
    registries: Option<Res<CurrentRegistries>>,
    mut commands: Commands,
) {
    let Some(data) = event.packet() else {
        return;
    };
    let checked = match registries.as_deref() {
        Some(registries) => data
            .metadata
            .check(registries)
            .context("an id is not an entry of the session registries"),
        None => Err(anyhow::anyhow!(
            "an id is not an entry of the session registries"
        )),
    };
    if let Err(error) = checked {
        refuse_packet(
            &mut commands,
            event.entity,
            ClientboundSetEntityData::NAME,
            error,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_dimension::keys::DIMENSION_TYPE;
    use mcrs_minecraft_dimension::keys::dimension::{OVERWORLD, THE_NETHER};
    use mcrs_minecraft_entity::keys::CAT_VARIANT;
    use mcrs_minecraft_entity::variant::CatVariant;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_network::ConnectionState;
    use mcrs_minecraft_network::metrics::PacketDispatcher;
    use mcrs_minecraft_protocol::entity::{MetaDataValue, Metadata, MetadataEntry};
    use mcrs_minecraft_protocol::{Encode, VarInt};
    use mcrs_minecraft_registries::Declaration;
    use std::sync::Arc;

    fn name(text: &str) -> ResourceLocation<Arc<str>> {
        ResourceLocation::read(text).unwrap()
    }

    fn session(cats: usize) -> CurrentRegistries {
        let types = [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
            "minecraft:beta",
        ]
        .map(name)
        .to_vec();
        let cats = (0..cats)
            .map(|n| name(&format!("minecraft:cat{n}")))
            .collect();
        let declarations = vec![
            Declaration::fixed(DIMENSION_TYPE.location().into(), types),
            Declaration::fixed(CAT_VARIANT.location().into(), cats),
        ];
        let (registries, _) = Registries::receive(
            declarations,
            &mcrs_minecraft_registries::Received::default(),
            None,
        )
        .unwrap();
        CurrentRegistries::new(registries)
    }

    fn dimension_type(number: u16) -> Id<DimensionType> {
        Id::checked(number, 8).unwrap()
    }

    fn login(player_id: i32, dimension_type: Id<DimensionType>) -> ClientboundLogin {
        ClientboundLogin {
            player_id,
            hardcore: false,
            dimensions: vec![OVERWORLD.into()],
            max_players: VarInt(1),
            chunk_radius: VarInt(8),
            simulation_distance: VarInt(8),
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
            player_spawn_info: PlayerSpawnInfo::new(
                dimension_type,
                OVERWORLD.into(),
                GameMode::Survival,
            ),
            online_mode: false,
            enforces_secure_chat: false,
        }
    }

    fn respawn(
        dimension: ResourceKey<Dimension>,
        dimension_type: Id<DimensionType>,
    ) -> ClientboundRespawn {
        ClientboundRespawn {
            player_spawn_info: PlayerSpawnInfo::new(dimension_type, dimension, GameMode::Survival),
            data_to_keep: 0,
        }
    }

    struct Client {
        app: App,
        connection: Entity,
    }

    impl Client {
        fn playing(registries: Option<CurrentRegistries>) -> Self {
            let mut app = App::new();
            add_observers(&mut app);
            if let Some(registries) = registries {
                app.insert_resource(registries);
            }
            let connection = app.world_mut().spawn(ConnectionState::Game).id();
            Client { app, connection }
        }

        fn deliver<P: Packet + Encode>(&mut self, packet: &P) {
            let mut data = Vec::new();
            packet.encode(&mut data).unwrap();
            let trigger = PacketDispatcher::clientbound()
                .dispatch(P::STATE, P::ID, &data)
                .unwrap()
                .unwrap();
            trigger.raise(
                &mut self.app.world_mut().commands(),
                self.connection,
                data.into(),
                mcrs_minecraft_network::Instant::now(),
            );
            self.app.world_mut().flush();
        }

        fn current(&self) -> Option<CurrentDimension> {
            self.app
                .world()
                .get::<CurrentDimension>(self.connection)
                .cloned()
        }

        fn dropped(&self) -> bool {
            self.app.world().get_entity(self.connection).is_err()
        }
    }

    #[test]
    fn a_respawn_moves_the_current_dimension_to_the_key_and_type_it_names() {
        let mut client = Client::playing(Some(session(0)));

        client.deliver(&login(7, dimension_type(3)));
        let first = client.current().expect("the login names type 3");
        assert_eq!(first.key, OVERWORLD);
        assert_eq!(first.dimension_type.index(), 3);

        client.deliver(&respawn(THE_NETHER.into(), dimension_type(1)));
        let second = client.current().expect("the respawn names type 1");
        assert_eq!(second.key, THE_NETHER);
        assert_eq!(second.dimension_type.index(), 1);
        assert_eq!(
            client
                .app
                .world()
                .get::<JoinedGame>(client.connection)
                .unwrap()
                .player_id,
            7
        );

        client.deliver(&respawn(OVERWORLD.into(), dimension_type(4)));
        assert!(client.dropped(), "a type number past the dimension types");
    }

    #[test]
    fn a_cat_variant_outside_the_session_registry_drops_the_connection() {
        let entity_data = |variant: u16| ClientboundSetEntityData {
            entity_id: VarInt(5),
            metadata: Metadata(vec![MetadataEntry {
                index: 20,
                value: MetaDataValue::CatVariant(Id::<CatVariant>::checked(variant, 3).unwrap()),
            }]),
        };

        let mut client = Client::playing(Some(session(2)));

        client.deliver(&entity_data(1));
        assert!(!client.dropped(), "the last cat variant of the session");

        client.deliver(&entity_data(2));
        assert!(client.dropped(), "one past the session's cat variants");
    }

    #[test]
    fn a_login_naming_a_type_outside_the_session_registries_drops_the_connection() {
        let mut client = Client::playing(Some(session(0)));
        client.deliver(&login(7, dimension_type(4)));
        assert!(client.dropped());

        let mut without = Client::playing(None);
        without.deliver(&login(7, dimension_type(0)));
        assert!(without.dropped(), "no session registries at all");
    }
}
