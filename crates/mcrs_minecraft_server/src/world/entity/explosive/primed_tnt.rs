use crate::world::aoi::TrackedBy;
use crate::world::entity::explosive::ExplosiveBundle;
use crate::world::entity::{EntityUuid, MinecraftEntity};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Query};
use bevy_ecs::query::{With, Without};
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_level::entity::mob::EntityKind;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::explosion::{Explosion, ExplosionRadius};
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::lifecycle::level::SectionLevels;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_world::entity::minecraft::PRIMED_TNT;

pub struct PrimedTntPlugin;

impl Plugin for PrimedTntPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, update_fuse_durations);
    }
}

pub const DEFAULT_EXPLOSION_RADIUS: u16 = 4;
pub const DEFAULT_FUSE_DURATION: u16 = 80;

#[derive(Bundle)]
pub struct PrimedTntBundle {
    pub dimension: InDimension,
    pub transform: Transform,
    pub uuid: EntityUuid,
    pub explosive: ExplosiveBundle,
    pub fuse: Fuse,
    kind: EntityKind,
    tracked_by: TrackedBy,
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
            kind: EntityKind(&PRIMED_TNT),
            tracked_by: TrackedBy::default(),
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

#[derive(Component, Debug, Deref, DerefMut)]
pub struct Fuse(pub u16);

impl Default for Fuse {
    fn default() -> Self {
        Self(DEFAULT_FUSE_DURATION)
    }
}

fn update_fuse_durations(
    mut query: Query<
        (Entity, &mut Fuse, &Transform, &InDimension),
        (With<PrimedTnt>, Without<Explosion>),
    >,
    levels: Query<&SectionLevels>,
    mut commands: Commands,
) {
    query.iter_mut().for_each(|(e, mut fuse, transform, dim)| {
        if !levels
            .get(dim.0)
            .is_ok_and(|levels| levels.is_entity_ticking(SectionPos::from(transform.translation)))
        {
            return;
        }
        let f = **fuse;
        if f > 0 {
            **fuse -= 1;
        } else {
            let mut cmds = commands.entity(e);
            cmds.remove::<Fuse>();
            cmds.insert(Explosion);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::schedule::IntoScheduleConfigs;
    use mcrs_minecraft_level::world::lifecycle::ticket::{
        SectionTickets, Ticket, propagate_section_levels,
    };

    #[test]
    fn a_fuse_burns_only_where_entities_tick() {
        let mut app = App::new();
        app.add_systems(
            FixedUpdate,
            (propagate_section_levels, update_fuse_durations).chain(),
        );
        let mut tickets = SectionTickets::default();
        tickets.add(SectionPos::new(0, 0, 0), Ticket::player_simulation(1));
        let dim = app
            .world_mut()
            .spawn((tickets, SectionLevels::default()))
            .id();
        let near = app
            .world_mut()
            .spawn(PrimedTntBundle::new(InDimension(dim), Transform::default()).with_fuse(5))
            .id();
        let mut far_away = Transform::default();
        far_away.translation.x = 16.0 * 5.0;
        let far = app
            .world_mut()
            .spawn(PrimedTntBundle::new(InDimension(dim), far_away).with_fuse(5))
            .id();

        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(**app.world().get::<Fuse>(near).expect("fuse"), 4);
        assert_eq!(**app.world().get::<Fuse>(far).expect("fuse"), 5);
    }
}
