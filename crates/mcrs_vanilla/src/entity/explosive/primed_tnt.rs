use crate::entity::explosive::ExplosiveBundle;
use crate::entity::{EntityUuid, MinecraftEntity};
use crate::explosion::{Explosion, ExplosionRadius};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::{ContainsEntity, Entity};
use bevy_ecs::prelude::{Commands, Query, With, Without};
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::uuid::Uuid;

pub const DEFAULT_EXPLOSION_RADIUS: f32 = 4.0;
pub const DEFAULT_FUSE_DURATION: u16 = 80;

pub struct PrimedTntPlugin;

impl Plugin for PrimedTntPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, update_fuse_durations);
    }
}

#[derive(Bundle)]
pub struct PrimedTntBundle {
    pub dimension: InDimension,
    pub transform: Transform,
    pub uuid: EntityUuid,
    pub explosive: ExplosiveBundle,
    pub fuse: Fuse,
    marker: PrimedTnt,
    mc_entity_marker: MinecraftEntity,
}

impl PrimedTntBundle {
    pub fn new(dimension: InDimension, transform: Transform) -> Self {
        Self {
            explosive: ExplosiveBundle {
                explosion_radius: ExplosionRadius(DEFAULT_EXPLOSION_RADIUS),
                ..Default::default()
            },
            fuse: Fuse::default(),
            mc_entity_marker: MinecraftEntity,
            marker: PrimedTnt,
            uuid: EntityUuid(Uuid::new_v4()),
            transform,
            dimension,
        }
    }

    pub fn with_fuse(mut self, fuse: u16) -> Self {
        self.fuse = Fuse(fuse);
        self
    }
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct PrimedTnt;

/// The detonator entity
#[derive(Component, Debug, Deref, DerefMut)]
pub struct Detonator(pub Entity);

impl ContainsEntity for Detonator {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct Fuse(pub u16);

impl Default for Fuse {
    fn default() -> Self {
        Self(DEFAULT_FUSE_DURATION)
    }
}

fn update_fuse_durations(
    mut query: Query<(Entity, &mut Fuse), (With<PrimedTnt>, Without<Explosion>)>,
    mut commands: Commands,
) {
    query.iter_mut().for_each(|(e, mut fuse)| {
        **fuse = fuse.saturating_sub(1);
        if **fuse == 0 {
            let mut cmds = commands.entity(e);
            cmds.remove::<Fuse>();
            cmds.insert(Explosion);
        }
    })
}
