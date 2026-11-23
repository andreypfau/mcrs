#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test reports a failure by panicking"
)]
mod attributes;
mod entity_light;
mod moving_entities;
mod packets;
mod pig_walk;
mod replay;
mod simulated_step;

use bevy_app::{App, FixedUpdate, PostUpdate, Update};
use bevy_ecs::entity::Entity;
use glam::DVec3;
use mcrs_minecraft_client_entity::{ClientEntityPlugin, NetworkEntities};
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::joined_game::JoinedGame;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{RenderInputs, RenderInputsQuery, ServerTicking};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ConnectionEnd, close_connection_later};
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_protocol::{Encode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3};
use uuid::Uuid;

pub struct Client {
    pub app: App,
    connection: Entity,
}

impl Client {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(ClientEntityPlugin);
        let connection = app.world_mut().spawn(ConnectionState::Game).id();
        Self { app, connection }
    }

    pub fn receive<P: Packet + Encode>(&mut self, packet: P) {
        let mut data = Vec::new();
        packet.encode(&mut data).expect("the test packet encodes");
        let trigger = PacketDispatcher::clientbound()
            .with_play(GAME_CLIENTBOUND)
            .dispatch(P::STATE, P::ID, &data)
            .expect("the row decodes")
            .expect("the row has a type");
        trigger.raise(
            self.app.world_mut().commands().reborrow(),
            self.connection,
            data.into(),
        );
        self.app.world_mut().flush();
    }

    pub fn join(&mut self, joined: JoinedGame) {
        self.app
            .world_mut()
            .entity_mut(self.connection)
            .insert(joined);
    }

    pub fn tick(&mut self) {
        self.app.world_mut().run_schedule(FixedUpdate);
    }

    pub fn end_session(&mut self) {
        let world = self.app.world_mut();
        close_connection_later(
            world.commands().reborrow(),
            self.connection,
            ConnectionEnd::Left,
        );
        world.flush();
        world.run_schedule(Update);
        world.flush();
    }

    pub fn frame(&mut self) {
        self.app.world_mut().run_schedule(PostUpdate);
    }

    pub fn ticking(&self) -> ServerTicking {
        *self.app.world().resource::<ServerTicking>()
    }

    pub fn entity(&self, network_id: i32) -> Option<Entity> {
        self.app
            .world()
            .resource::<NetworkEntities>()
            .get(network_id)
    }

    pub fn inputs<T>(&mut self, network_id: i32, read: impl FnOnce(RenderInputs<'_>) -> T) -> T {
        let entity = self.entity(network_id).expect("the entity was added");
        let mut query = self.app.world_mut().query::<RenderInputsQuery>();
        let row = query
            .get(self.app.world(), entity)
            .expect("a client entity has its render inputs");
        read(RenderInputs::from(row))
    }
}

pub fn add_entity(
    id: i32,
    kind: EntityType,
    position: DVec3,
    yaw: f32,
    head_yaw: f32,
) -> ClientboundAddEntity {
    ClientboundAddEntity {
        id: VarInt(id),
        uuid: Uuid::from_u128(1),
        kind,
        pos: position,
        movement: LpVec3::default(),
        pitch: ByteAngle(0),
        yaw: ByteAngle::from_degrees(yaw),
        head_yaw: ByteAngle::from_degrees(head_yaw),
        data: VarInt(0),
    }
}
