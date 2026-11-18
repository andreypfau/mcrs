use crate::offline_player_uuid;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Encode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundAnimate, ClientboundEntityPositionSync,
    ClientboundRemoveEntities, ClientboundSwingAnimation, ClientboundTeleportEntity, PositionPath,
    VecDelta,
};
use mcrs_minecraft_protocol_game::{
    Hand, Position, PositionFlag, SwingAnimationKind, position_delta,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeHand {
    #[default]
    Main,
    Off,
}

impl From<ProbeHand> for Hand {
    fn from(hand: ProbeHand) -> Self {
        match hand {
            ProbeHand::Main => Self::Main,
            ProbeHand::Off => Self::Off,
        }
    }
}

impl From<Hand> for ProbeHand {
    fn from(hand: Hand) -> Self {
        match hand {
            Hand::Main => Self::Main,
            Hand::Off => Self::Off,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwingObserved {
    pub client: String,
    pub subject: String,
    pub hand: ProbeHand,
    pub animation: SwingAnimationKind,
    pub duration: i32,
}

#[derive(Clone)]
pub(crate) struct CapturedSwing {
    pub(crate) observed: SwingObserved,
    pub(crate) entity_id: i32,
    pub(crate) canonical: bool,
    pub(crate) raw_body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimateObserved {
    pub client: String,
    pub subject: String,
    pub action: i32,
}

#[derive(Default)]
pub(crate) struct EntityNames {
    known_players: BTreeMap<Uuid, String>,
    self_entity: Option<(String, i32)>,
    id_to_name: BTreeMap<i32, String>,
    name_to_ids: BTreeMap<String, Vec<i32>>,
    positions: BTreeMap<i32, Position>,
}

impl EntityNames {
    pub(crate) fn with_players(players: &[String]) -> Self {
        let mut known_players = BTreeMap::new();
        for player in players {
            known_players.insert(
                offline_player_uuid::offline_player_uuid(player),
                player.clone(),
            );
        }
        Self {
            known_players,
            self_entity: None,
            id_to_name: BTreeMap::new(),
            name_to_ids: BTreeMap::new(),
            positions: BTreeMap::new(),
        }
    }

    pub(crate) fn record_login(&mut self, client: &str, player_id: i32) {
        self.self_entity = Some((client.to_owned(), player_id));
        self.insert_named(player_id, client.to_owned());
    }

    pub(crate) fn record_add_entity(&mut self, packet: &ClientboundAddEntity) {
        let id = packet.id.0;
        let label = self
            .known_players
            .get(&packet.uuid)
            .cloned()
            .unwrap_or_else(|| packet.kind.as_static_str().to_owned());
        self.insert_named(id, label);
        self.positions.insert(id, Position::from(packet.pos));
    }

    pub(crate) fn remove_entities(&mut self, packet: &ClientboundRemoveEntities) -> Vec<String> {
        packet
            .entity_ids
            .iter()
            .map(|entity_id| {
                let name = self.resolve_subject(entity_id.0);
                self.detach_id(entity_id.0);
                name
            })
            .collect()
    }

    pub(crate) fn clear_world_entities(&mut self) {
        self.id_to_name.clear();
        self.name_to_ids.clear();
        self.positions.clear();
        if let Some((client, id)) = self.self_entity.clone() {
            self.insert_named(id, client);
        }
    }

    pub(crate) fn record_teleport(&mut self, packet: &ClientboundTeleportEntity) {
        let id = packet.entity_id.0;
        let prev = self.positions.get(&id).copied().unwrap_or_default();
        let flags = packet.relatives;
        let next = Position::new(
            if flags.contains(PositionFlag::X) {
                prev.x + packet.position.x
            } else {
                packet.position.x
            },
            if flags.contains(PositionFlag::Y) {
                prev.y + packet.position.y
            } else {
                packet.position.y
            },
            if flags.contains(PositionFlag::Z) {
                prev.z + packet.position.z
            } else {
                packet.position.z
            },
        );
        self.positions.insert(id, next);
    }

    pub(crate) fn record_position_sync(&mut self, packet: &ClientboundEntityPositionSync) {
        let end_pos = match &packet.position {
            PositionPath::Linear(pos) => Some(Position::from(*pos)),
            PositionPath::Stepped { steps, .. } => {
                steps.last().map(|step| Position::from(step.position))
            }
        };
        if let Some(pos) = end_pos {
            self.positions.insert(packet.entity_id.0, pos);
        }
    }

    pub(crate) fn record_move_delta(&mut self, entity_id: i32, delta: &VecDelta) {
        let Some(pos) = self.positions.get_mut(&entity_id) else {
            return;
        };
        match delta {
            VecDelta::Linear(d) => *pos = Position::from(position_delta::decode((*pos).into(), *d)),
            VecDelta::Stepped { steps, .. } => {
                for step in steps {
                    *pos = Position::from(position_delta::decode((*pos).into(), step.delta));
                }
            }
        }
    }

    pub(crate) fn resolve_subject(&self, entity_id: i32) -> String {
        self.id_to_name
            .get(&entity_id)
            .cloned()
            .unwrap_or_else(|| format!("entity:{entity_id}"))
    }

    pub(crate) fn target_id(
        &self,
        target: &str,
        self_pos: Option<Position>,
    ) -> anyhow::Result<VarInt> {
        if let Some(ids) = self.name_to_ids.get(target).filter(|ids| !ids.is_empty()) {
            if let [single] = ids.as_slice() {
                return Ok(VarInt(*single));
            }
            let origin = self_pos.ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot rank multiple `{target}` targets without an accepted player position"
                )
            })?;
            let chosen = ids
                .iter()
                .copied()
                .min_by(|left, right| {
                    compare_target_distance(*left, *right, &self.positions, origin)
                })
                .ok_or_else(|| anyhow::anyhow!("unknown target entity `{target}`"))?;
            return Ok(VarInt(chosen));
        }
        if let Ok(id) = target.parse::<i32>() {
            return Ok(VarInt(id));
        }
        anyhow::bail!("unknown target entity `{target}`")
    }

    fn insert_named(&mut self, id: i32, label: String) {
        self.detach_id(id);
        self.id_to_name.insert(id, label.clone());
        let ids = self.name_to_ids.entry(label).or_default();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    fn detach_id(&mut self, id: i32) {
        self.positions.remove(&id);
        let Some(old_label) = self.id_to_name.remove(&id) else {
            return;
        };
        if let Some(ids) = self.name_to_ids.get_mut(&old_label) {
            ids.retain(|&existing| existing != id);
            if ids.is_empty() {
                self.name_to_ids.remove(&old_label);
            }
        }
    }
}

fn compare_target_distance(
    left: i32,
    right: i32,
    positions: &BTreeMap<i32, Position>,
    self_pos: Position,
) -> Ordering {
    match (positions.get(&left), positions.get(&right)) {
        (Some(lpos), Some(rpos)) => lpos
            .distance_squared(*self_pos)
            .total_cmp(&rpos.distance_squared(*self_pos)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

impl SwingObserved {
    pub(crate) fn is_packet(id: i32) -> bool {
        id == ClientboundSwingAnimation::ID
    }

    pub(crate) fn read(
        client: &str,
        names: &EntityNames,
        body: &[u8],
    ) -> anyhow::Result<CapturedSwing> {
        let reader = &mut &*body;
        let packet = ClientboundSwingAnimation::decode(reader)?;
        anyhow::ensure!(
            reader.is_empty(),
            "the swing_animation packet has trailing bytes"
        );
        let mut canonical_bytes = Vec::with_capacity(body.len());
        packet.encode(&mut canonical_bytes)?;
        Ok(CapturedSwing {
            observed: Self {
                client: client.to_owned(),
                subject: names.resolve_subject(packet.entity_id.0),
                hand: packet.hand.into(),
                animation: packet.animation,
                duration: packet.duration.0,
            },
            entity_id: packet.entity_id.0,
            canonical: canonical_bytes.as_slice() == body,
            raw_body: body.to_vec(),
        })
    }
}

impl AnimateObserved {
    pub(crate) fn is_packet(id: i32) -> bool {
        id == ClientboundAnimate::ID
    }

    pub(crate) fn read(client: &str, names: &EntityNames, mut body: &[u8]) -> anyhow::Result<Self> {
        let reader = &mut body;
        let packet = ClientboundAnimate::decode(reader)?;
        anyhow::ensure!(reader.is_empty(), "the animate packet has trailing bytes");
        Ok(Self {
            client: client.to_owned(),
            subject: names.resolve_subject(packet.entity_id.0),
            action: i32::from(packet.action),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_numeric_name_takes_precedence_and_multi_target_requires_pos() {
        let mut names = EntityNames::with_players(&["123".to_owned()]);
        names.record_login("123", 42);
        assert_eq!(names.target_id("123", None).unwrap().0, 42);
        assert_eq!(names.target_id("999", None).unwrap().0, 999);
        assert!(names.target_id("nobody", None).is_err());

        names.insert_named(10, "minecraft:pig".to_owned());
        names.positions.insert(10, Position::new(10.0, 0.0, 0.0));
        names.insert_named(20, "minecraft:pig".to_owned());
        names.positions.insert(20, Position::new(2.0, 0.0, 0.0));

        assert!(names.target_id("minecraft:pig", None).is_err());
        assert_eq!(
            names
                .target_id("minecraft:pig", Some(Position::new(0.0, 0.0, 0.0)))
                .unwrap()
                .0,
            20
        );
    }
}
