use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_visual::{ClientEntity, Motion, SyncedData, dimensions};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_physics::{Aabb, PlayerState};
use mcrs_minecraft_protocol::{VarInt, WritePacket};
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAttack, ServerboundPlayerAction, ServerboundPunch,
};
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::attack_input::AttackInput;
use crate::block_prediction::BlockPredictions;
use crate::block_target::{BlockHit, EntityBox, Reach, Target, target};
use crate::columns::{AIR, ColumnStore};
use crate::game_mode::{LocalGameMode, PermissionLevel};
use crate::hotbar_sync::SendSelectedSlot;
use crate::local_motion::{self, LocalMotion};
use crate::local_player::LocalPlayerTick;
use crate::player::{Player, look_direction};
use crate::scripted_input::ScriptedInput;

const BREAK_DELAY: u8 = 5;
const MISS_COOLDOWN: u32 = 10;
const SCREEN_MISS_COOLDOWN: u32 = 10_000;

const OPERATOR_BLOCKS: [Block; 7] = [
    Block::CommandBlock,
    Block::ChainCommandBlock,
    Block::RepeatingCommandBlock,
    Block::StructureBlock,
    Block::Jigsaw,
    Block::TestBlock,
    Block::TestInstanceBlock,
];

pub struct BlockBreakingPlugin;

impl Plugin for BlockBreakingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttackInput>()
            .init_resource::<Attack>()
            .init_resource::<ScriptedInput>()
            .add_systems(
                FixedUpdate,
                attack.after(SendSelectedSlot).before(LocalPlayerTick),
            );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outgoing {
    Start {
        pos: BlockPos,
        face: Direction,
        sequence: i32,
    },
    Attack(Entity),
    Punch,
}

pub trait BreakWorld {
    fn is_air(&self, pos: BlockPos) -> bool;

    fn predict_break(&mut self, pos: BlockPos) -> i32;

    fn send(&mut self, packet: Outgoing);
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attack {
    break_delay: u8,
    miss_cooldown: u32,
}

impl Attack {
    pub fn tick<W: BreakWorld>(
        &mut self,
        input: AttackInput,
        target: Option<Target>,
        mode: GameMode,
        world: &mut W,
    ) {
        if input.screen_open {
            self.miss_cooldown = SCREEN_MISS_COOLDOWN;
            return;
        }
        let mut broke_at_once = false;
        for _ in 0..input.presses {
            broke_at_once |= self.press(target, mode, world);
        }
        if input.down && !broke_at_once {
            if self.miss_cooldown == 0 {
                self.hold(target, mode, world);
            }
        } else {
            self.miss_cooldown = 0;
        }
        self.miss_cooldown = self.miss_cooldown.saturating_sub(1);
    }

    fn press<W: BreakWorld>(
        &mut self,
        target: Option<Target>,
        mode: GameMode,
        world: &mut W,
    ) -> bool {
        if self.miss_cooldown > 0 || mode == GameMode::Spectator {
            return false;
        }
        let mut broke_at_once = false;
        match target {
            Some(Target::Entity(hit)) => world.send(Outgoing::Attack(hit.entity)),
            Some(Target::Block(hit)) if !world.is_air(hit.pos) => {
                if mode == GameMode::Creative {
                    self.start(hit, world);
                }
                broke_at_once = world.is_air(hit.pos);
            }
            _ if mode != GameMode::Creative => self.miss_cooldown = MISS_COOLDOWN,
            _ => {}
        }
        world.send(Outgoing::Punch);
        broke_at_once
    }

    fn hold<W: BreakWorld>(&mut self, target: Option<Target>, mode: GameMode, world: &mut W) {
        let Some(Target::Block(hit)) = target else {
            return;
        };
        if mode != GameMode::Creative || world.is_air(hit.pos) {
            return;
        }
        if self.break_delay > 0 {
            self.break_delay -= 1;
        } else {
            self.start(hit, world);
        }
        world.send(Outgoing::Punch);
    }

    fn start<W: BreakWorld>(&mut self, hit: BlockHit, world: &mut W) {
        let sequence = world.predict_break(hit.pos);
        world.send(Outgoing::Start {
            pos: hit.pos,
            face: hit.face,
            sequence,
        });
        self.break_delay = BREAK_DELAY;
    }
}

fn broken_state(blocks: &BlockDefinitions, old: BlockStateId) -> BlockStateId {
    match blocks.state(old).fluid {
        Some(fluid) if fluid.fluid == Fluid::Water.id() => {
            blocks.default_state_of(Block::Water.id())
        }
        _ => AIR,
    }
}

fn knock_back_attacker(attacker: &mut PlayerState, attack_knockback: f32) {
    let sprint = if attacker.sprint0 { 0.5 } else { 0.0 };
    if attack_knockback / 2.0 + sprint > 0.0 {
        attacker.body.velocity *= DVec3::new(0.6, 1.0, 0.6);
        attacker.sprint0 = false;
    }
}

fn is_operator_block(blocks: &BlockDefinitions, state: BlockStateId) -> bool {
    blocks.block_index(state).is_some_and(|block| {
        OPERATOR_BLOCKS
            .iter()
            .any(|operator| operator.id() == block)
    })
}

pub type Targets = (
    Entity,
    &'static ClientEntity,
    &'static Motion,
    &'static SyncedData,
);

struct ClientWorld<'a, 'w, 's> {
    targets: &'a Query<'w, 's, Targets, Without<Player>>,
    attacker: Option<Mut<'a, LocalMotion>>,
    attack_knockback: f32,
    store: &'a mut ColumnStore,
    predictions: &'a mut BlockPredictions,
    blocks: &'a BlockDefinitions,
    connection: &'a mut ClientConnection,
    permission: PermissionLevel,
}

impl BreakWorld for ClientWorld<'_, '_, '_> {
    fn is_air(&self, pos: BlockPos) -> bool {
        self.store.block(pos).unwrap_or(AIR) == AIR
    }

    fn predict_break(&mut self, pos: BlockPos) -> i32 {
        let sequence = self.predictions.next_sequence();
        let Ok(old) = self.store.block(pos) else {
            return sequence;
        };
        let refused =
            is_operator_block(self.blocks, old) && self.permission < PermissionLevel::GAMEMASTERS;
        if old == AIR || refused {
            return sequence;
        }
        if let Some(replaced) = self.store.set_block(pos, broken_state(self.blocks, old)) {
            self.predictions.record(sequence, pos, replaced);
        }
        sequence
    }

    fn send(&mut self, packet: Outgoing) {
        match packet {
            Outgoing::Start {
                pos,
                face,
                sequence,
            } => self.connection.write_packet(&ServerboundPlayerAction {
                action: PlayerAction::StartDestroyBlock,
                pos,
                direction: face,
                sequence: VarInt(sequence),
            }),
            Outgoing::Attack(entity) => {
                let Ok((_, target, ..)) = self.targets.get(entity) else {
                    return;
                };
                self.connection.write_packet(&ServerboundAttack {
                    entity_id: VarInt(target.network_id),
                });
                if target.kind == EntityType::Player
                    && let Some(attacker) = self.attacker.as_mut()
                {
                    knock_back_attacker(&mut attacker.0, self.attack_knockback);
                }
            }
            Outgoing::Punch => self.connection.write_packet(&ServerboundPunch),
        }
    }
}

#[must_use]
pub fn pick(
    transform: &PhysicsTransform,
    motion: Option<&LocalMotion>,
    reach: Reach,
    blocks: &BlockDefinitions,
    store: &ColumnStore,
    targets: &Query<Targets, Without<Player>>,
) -> Option<Target> {
    let eye = transform.translation + DVec3::Y * f64::from(local_motion::eye_height(motion));
    let rotation = transform.rotation;
    let direction = look_direction(rotation.yaw(), rotation.pitch())
        .as_dvec3()
        .normalize();
    let boxes = targets.iter().filter_map(|(entity, client, motion, data)| {
        if matches!(client.kind, EntityType::Item | EntityType::ExperienceOrb) || data.is_dying() {
            return None;
        }
        let size = dimensions(client, data)?;
        let aabb = Aabb::of_body(motion.current.position, size.width, size.height);
        Some(EntityBox {
            entity,
            min: aabb.min,
            max: aabb.max,
        })
    });
    target(
        eye,
        direction,
        reach,
        |pos| {
            let state = store.block(pos).unwrap_or(AIR);
            blocks.shape(blocks.state(state).selection_shape)
        },
        boxes,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter"
)]
fn attack(
    mut input: ResMut<AttackInput>,
    mut attack: ResMut<Attack>,
    scripted: Res<ScriptedInput>,
    player: Single<
        (
            &PhysicsTransform,
            Option<&mut LocalMotion>,
            Option<&AttributeInstances>,
            Option<&LocalGameMode>,
            Option<&PermissionLevel>,
        ),
        With<Player>,
    >,
    connection: Option<Single<(&mut ClientConnection, &ConnectionState)>>,
    blocks: Option<Res<Blocks>>,
    store: Option<ResMut<ColumnStore>>,
    predictions: Option<ResMut<BlockPredictions>>,
    targets: Query<Targets, Without<Player>>,
) {
    let input = input.take().with(scripted.attack);
    let (transform, motion, instances, mode, permission) = player.into_inner();
    let (
        Some(reach),
        Some(connection),
        Some(mode),
        Some(blocks),
        Some(mut store),
        Some(mut predictions),
    ) = (
        instances.and_then(Reach::from_instances),
        connection,
        mode,
        blocks,
        store,
        predictions,
    )
    else {
        return;
    };
    let (mut connection, state) = connection.into_inner();
    if *state != ConnectionState::Game {
        return;
    }
    let target = pick(
        transform,
        motion.as_deref(),
        reach,
        &blocks,
        &store,
        &targets,
    );
    let attack_knockback = instances
        .and_then(|instances| instances.value(Attribute::AttackKnockback))
        .unwrap_or(0.0) as f32;
    let mut world = ClientWorld {
        targets: &targets,
        attacker: motion,
        attack_knockback,
        store: &mut store,
        predictions: &mut predictions,
        blocks: &blocks,
        connection: &mut connection,
        permission: permission.copied().unwrap_or_default(),
    };
    attack.tick(input, target, mode.current, &mut world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_target::EntityHit;
    use std::collections::HashSet;

    const STONE: BlockPos = BlockPos::new(0, 0, 0);
    const OTHER: BlockPos = BlockPos::new(1, 0, 0);

    #[derive(Default)]
    struct Fake {
        solid: HashSet<BlockPos>,
        stays: bool,
        sequence: i32,
        sent: Vec<Outgoing>,
    }

    impl BreakWorld for Fake {
        fn is_air(&self, pos: BlockPos) -> bool {
            !self.solid.contains(&pos)
        }

        fn predict_break(&mut self, pos: BlockPos) -> i32 {
            if !self.stays {
                self.solid.remove(&pos);
            }
            self.sequence += 1;
            self.sequence
        }

        fn send(&mut self, packet: Outgoing) {
            self.sent.push(packet);
        }
    }

    impl Fake {
        fn with(solid: &[BlockPos], stays: bool) -> Fake {
            Fake {
                solid: solid.iter().copied().collect(),
                stays,
                ..Fake::default()
            }
        }

        fn take(&mut self) -> Vec<Outgoing> {
            std::mem::take(&mut self.sent)
        }
    }

    fn hit(pos: BlockPos) -> Option<Target> {
        Some(Target::Block(BlockHit {
            pos,
            face: Direction::Up,
            point: pos.as_ivec3().as_dvec3(),
        }))
    }

    fn press() -> AttackInput {
        AttackInput {
            presses: 1,
            down: true,
            screen_open: false,
        }
    }

    fn hold() -> AttackInput {
        AttackInput {
            presses: 0,
            ..press()
        }
    }

    fn start(pos: BlockPos, sequence: i32) -> Outgoing {
        Outgoing::Start {
            pos,
            face: Direction::Up,
            sequence,
        }
    }

    fn start_ticks(stays: bool) -> Vec<usize> {
        let mut world = Fake::with(&[STONE], stays);
        let mut attack = Attack::default();
        let mut ticks = Vec::new();
        for tick in 0..13 {
            let input = if tick == 0 { press() } else { hold() };
            world.solid.insert(STONE);
            attack.tick(input, hit(STONE), GameMode::Creative, &mut world);
            if world
                .take()
                .iter()
                .any(|packet| matches!(packet, Outgoing::Start { .. }))
            {
                ticks.push(tick);
            }
        }
        ticks
    }

    #[test]
    fn cbk_cre_004_a_hold_starts_a_break_every_six_ticks() {
        assert_eq!(start_ticks(false), [0, 6, 12]);
    }

    #[test]
    fn cbk_cre_004_a_press_that_leaves_a_block_makes_the_first_interval_five_ticks() {
        assert_eq!(start_ticks(true), [0, 5, 11]);
    }

    #[test]
    fn cbk_cre_001_a_press_sends_the_start_and_then_the_punch() {
        let mut world = Fake::with(&[STONE], false);
        let mut attack = Attack::default();
        attack.tick(press(), hit(STONE), GameMode::Creative, &mut world);
        assert_eq!(world.take(), [start(STONE, 1), Outgoing::Punch]);

        let mut stays = Fake::with(&[STONE], true);
        Attack::default().tick(press(), hit(STONE), GameMode::Creative, &mut stays);
        assert_eq!(
            stays.take(),
            [start(STONE, 1), Outgoing::Punch, Outgoing::Punch],
            "the hold of the same tick swings again"
        );
    }

    #[test]
    fn cbk_cre_003_a_miss_does_not_count_down_the_break_delay() {
        let mut world = Fake::with(&[STONE, OTHER], false);
        let mut attack = Attack::default();
        attack.tick(press(), hit(STONE), GameMode::Creative, &mut world);
        world.take();
        for _ in 0..10 {
            attack.tick(hold(), None, GameMode::Creative, &mut world);
        }
        assert_eq!(world.take(), []);
        for _ in 0..5 {
            attack.tick(hold(), hit(OTHER), GameMode::Creative, &mut world);
        }
        assert_eq!(world.take(), [Outgoing::Punch; 5]);
        attack.tick(hold(), hit(OTHER), GameMode::Creative, &mut world);
        assert_eq!(world.take(), [start(OTHER, 2), Outgoing::Punch]);
    }

    #[test]
    fn cbk_cre_007_a_second_press_on_the_broken_block_only_swings() {
        let mut world = Fake::with(&[STONE], false);
        let input = AttackInput {
            presses: 2,
            ..press()
        };
        Attack::default().tick(input, hit(STONE), GameMode::Creative, &mut world);
        assert_eq!(
            world.take(),
            [start(STONE, 1), Outgoing::Punch, Outgoing::Punch]
        );
    }

    #[test]
    fn cbk_inp_004_a_press_on_an_entity_attacks_it_and_swings_and_the_hold_does_not_repeat() {
        let mut world = Fake::default();
        let mut attack = Attack::default();
        let pig = Some(Target::Entity(EntityHit {
            entity: Entity::PLACEHOLDER,
            point: DVec3::ZERO,
        }));
        let attacked = [Outgoing::Attack(Entity::PLACEHOLDER), Outgoing::Punch];
        attack.tick(press(), pig, GameMode::Survival, &mut world);
        attack.tick(hold(), pig, GameMode::Survival, &mut world);
        assert_eq!(world.take(), attacked);
        attack.tick(press(), pig, GameMode::Survival, &mut world);
        assert_eq!(world.take(), attacked, "an attack starts no miss cooldown");
    }

    #[test]
    fn atk_kb_003_only_an_attack_with_knockback_slows_the_attacker_and_stops_its_sprint() {
        let velocity = DVec3::new(1.0, 0.5, -1.0);
        let slowed = DVec3::new(0.6, 0.5, -0.6);
        for (sprinting, attack_knockback, expected) in [
            (false, 0.0, velocity),
            (true, 0.0, slowed),
            (false, 1.0, slowed),
        ] {
            let mut attacker = PlayerState::default();
            attacker.body.velocity = velocity;
            attacker.sprint0 = sprinting;
            knock_back_attacker(&mut attacker, attack_knockback);
            assert_eq!(
                (attacker.body.velocity, attacker.sprint0),
                (expected, false),
                "sprinting {sprinting}, attack knockback {attack_knockback}"
            );
        }
    }

    #[test]
    fn cbk_inp_007_after_a_screen_the_input_is_dead_until_it_is_released() {
        let mut world = Fake::with(&[STONE], false);
        let mut attack = Attack::default();
        let screen = AttackInput {
            screen_open: true,
            ..hold()
        };
        attack.tick(screen, hit(STONE), GameMode::Creative, &mut world);
        attack.tick(press(), hit(STONE), GameMode::Creative, &mut world);
        attack.tick(hold(), hit(STONE), GameMode::Creative, &mut world);
        assert_eq!(world.take(), []);

        attack.tick(
            AttackInput::default(),
            hit(STONE),
            GameMode::Creative,
            &mut world,
        );
        attack.tick(press(), hit(STONE), GameMode::Creative, &mut world);
        assert_eq!(world.take(), [start(STONE, 1), Outgoing::Punch]);
    }
}
