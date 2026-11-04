//! The profile entries that each viewer has on its client, and which of them its player list
//! shows.
//!
//! A viewer has the entries of its visible set, which are not listed, and at most
//! [`LISTED_CAP`] listed entries that a [`ListPolicy`] chooses. An entry that nothing needs any
//! more stays on the client for [`ENTRY_LINGER_TICKS`], so a subject at the edge of the track
//! radius does not send its profile again on each return.

use crate::bus::{
    EntryLatency, ListChange, OutboundPlayerPacket, PacketPayload, PacketTarget, to, to_target,
};
use crate::entity_tracker::EntityTracker;
use crate::game_mode::ListedGameMode;
use crate::latency::Latency;
use crate::session::GameProfile;
use bevy_ecs::entity::{Entity, EntityHashMap, EntityHashSet};
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Query, Res, ResMut};
use bevy_ecs::resource::Resource;
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerInfoRemove, ClientboundPlayerInfoUpdate,
};
use mcrs_minecraft_protocol_game::player_list::{PlayerListActions, PlayerListEntry};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::mem;

/// The most entries that the client shows in its player list, the viewer itself included.
pub const LISTED_CAP: usize = 80;

/// A viewer checks its listed entries at most this often, in ticks.
pub const LIST_INTERVAL_TICKS: u64 = 20;

/// The ticks that an entry which nothing needs stays on the client before its removal.
pub const ENTRY_LINGER_TICKS: u64 = 600;

/// A viewer gets the latency of its listed entries at most this often, in ticks: 60 seconds.
pub const LATENCY_INTERVAL_TICKS: u64 = 1200;

/// A player that a [`ListPolicy`] can list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub player: Entity,
    pub position: DVec3,
}

/// What a [`ListPolicy`] chooses from.
#[derive(Clone, Copy, Debug)]
pub struct ListRequest<'a> {
    pub viewer: Candidate,
    /// Every player of the world, the viewer included, sorted by entity.
    pub candidates: &'a [Candidate],
    pub limit: usize,
}

/// Chooses the players that the list of a viewer shows besides the viewer: at most `limit`
/// candidates other than the viewer, written to the vector, which starts empty.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ListPolicy(pub fn(&ListRequest<'_>, &mut Vec<Entity>));

impl Default for ListPolicy {
    fn default() -> Self {
        Self(nearest)
    }
}

/// The policy that lists the nearest players. Equal distances go to the lower entity.
pub fn nearest(request: &ListRequest<'_>, listed: &mut Vec<Entity>) {
    let mut ranked: Vec<Ranked> = request
        .candidates
        .iter()
        .filter(|candidate| candidate.player != request.viewer.player)
        .map(|candidate| Ranked {
            distance: candidate.position.distance_squared(request.viewer.position),
            player: candidate.player,
        })
        .collect();
    let order = |first: &Ranked, second: &Ranked| {
        first
            .distance
            .total_cmp(&second.distance)
            .then(first.player.cmp(&second.player))
    };
    if ranked.len() > request.limit {
        ranked.select_nth_unstable_by(request.limit, order);
        ranked.truncate(request.limit);
    }
    listed.extend(ranked.iter().map(|ranked| ranked.player));
}

struct Ranked {
    distance: f64,
    player: Entity,
}

/// Whether the player list of a viewer shows an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listing {
    Listed,
    Unlisted,
}

/// Whether a spawn must carry the profile entry of its subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileNeed {
    Send,
    Skip,
}

#[derive(Debug)]
struct Entry {
    listing: Listing,
    /// The tick since which the entry is neither listed nor visible.
    idle_since: Option<u64>,
    /// The latency that the viewer last received for the entry.
    sent_latency: i32,
}

#[derive(Debug)]
struct ViewerEntries {
    session: PlayerSession,
    entries: EntityHashMap<Entry>,
    /// The tracker generation of the last choice of the listed entries.
    chosen_at: Option<u64>,
}

#[derive(Debug)]
struct SubjectProfile {
    uuid: Uuid,
    /// The viewers that have the entry, the subject itself included.
    holders: EntityHashSet,
    encoded: Option<EncodedProfile>,
}

#[derive(Debug)]
struct EncodedProfile {
    game_mode: GameMode,
    packet: Bytes,
}

/// The profile entries of the viewers of a world. The entity tracker system and the removal of
/// a player change it.
#[derive(Resource, Default, Debug)]
pub struct PlayerList {
    viewers: EntityHashMap<ViewerEntries>,
    subjects: EntityHashMap<SubjectProfile>,
    /// The entries that each viewer loses at the next update.
    removals: BTreeMap<Entity, Vec<Uuid>>,
}

impl PlayerList {
    /// Adds `player`, whose own listed entry its login or its respawn gave its client.
    pub fn join(&mut self, player: Entity, session: PlayerSession, uuid: Uuid) {
        let own = Entry {
            listing: Listing::Listed,
            idle_since: None,
            sent_latency: 0,
        };
        self.viewers.insert(
            player,
            ViewerEntries {
                session,
                entries: EntityHashMap::from_iter([(player, own)]),
                chosen_at: None,
            },
        );
        self.subjects.insert(
            player,
            SubjectProfile {
                uuid,
                holders: EntityHashSet::from_iter([player]),
                encoded: None,
            },
        );
    }

    /// Records that `viewer` gains `subject` in its visible set.
    pub fn acquire(&mut self, viewer: Entity, subject: Entity) -> ProfileNeed {
        let (Some(entries), Some(profile)) = (
            self.viewers.get_mut(&viewer),
            self.subjects.get_mut(&subject),
        ) else {
            return ProfileNeed::Send;
        };
        if let Some(entry) = entries.entries.get_mut(&subject) {
            entry.idle_since = None;
            return ProfileNeed::Skip;
        }
        entries.entries.insert(
            subject,
            Entry {
                listing: Listing::Unlisted,
                idle_since: None,
                sent_latency: 0,
            },
        );
        profile.holders.insert(viewer);
        ProfileNeed::Send
    }

    /// Records that `viewer` loses `subject` from its visible set at `tick`.
    pub fn release(&mut self, viewer: Entity, subject: Entity, tick: u64) {
        if let Some(entry) = self
            .viewers
            .get_mut(&viewer)
            .and_then(|entries| entries.entries.get_mut(&subject))
            && entry.listing == Listing::Unlisted
        {
            entry.idle_since = Some(tick);
        }
    }

    /// Removes `player`. Each viewer that has its entry loses it at the next update. Gives the
    /// packet that removes the entries of `player` from its own client, which keeps them across
    /// a respawn in another dimension.
    pub fn leave(&mut self, player: Entity) -> Option<OutboundPlayerPacket> {
        if let Some(profile) = self.subjects.remove(&player) {
            for holder in profile.holders {
                if holder == player {
                    continue;
                }
                if let Some(entries) = self.viewers.get_mut(&holder) {
                    entries.entries.remove(&player);
                    self.removals.entry(holder).or_default().push(profile.uuid);
                }
            }
        }
        let entries = self.viewers.remove(&player)?;
        let mut players = self.removals.remove(&player).unwrap_or_default();
        for subject in entries.entries.into_keys() {
            if subject == player {
                continue;
            }
            if let Some(profile) = self.subjects.get_mut(&subject) {
                profile.holders.remove(&player);
                players.push(profile.uuid);
            }
        }
        (!players.is_empty()).then(|| {
            to(
                entries.session,
                PacketPayload::PlayerInfoRemove(ClientboundPlayerInfoRemove { players }),
            )
        })
    }

    /// The sessions of the viewers that have the entry of `subject`.
    #[must_use]
    pub fn holder_sessions(&self, subject: Entity) -> SmallVec<[PlayerSession; 8]> {
        let Some(profile) = self.subjects.get(&subject) else {
            return SmallVec::new();
        };
        profile
            .holders
            .iter()
            .filter_map(|holder| self.viewers.get(holder).map(|entries| entries.session))
            .collect()
    }

    /// The profile entry of `subject`, encoded once for each game mode that it has.
    pub fn profile_packet(
        &mut self,
        subject: Entity,
        profile: &GameProfile,
        game_mode: GameMode,
    ) -> Option<Bytes> {
        let Some(record) = self.subjects.get_mut(&subject) else {
            return encode_profile(profile, game_mode);
        };
        cached_profile(record, profile, game_mode)
    }
}

fn cached_profile(
    record: &mut SubjectProfile,
    profile: &GameProfile,
    game_mode: GameMode,
) -> Option<Bytes> {
    if let Some(encoded) = &record.encoded
        && encoded.game_mode == game_mode
    {
        return Some(encoded.packet.clone());
    }
    let packet = encode_profile(profile, game_mode)?;
    record.encoded = Some(EncodedProfile {
        game_mode,
        packet: packet.clone(),
    });
    Some(packet)
}

/// The `player_info_update` packet that adds the entry of `profile`, with its id.
fn encode_profile(profile: &GameProfile, game_mode: GameMode) -> Option<Bytes> {
    encode_update(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new()
            .with_add_player(true)
            .with_update_game_mode(true),
        entries: Cow::Borrowed(&[PlayerListEntry {
            player_uuid: profile.id,
            username: profile.username.as_str(),
            properties: Cow::Borrowed(&profile.properties),
            game_mode,
            ..Default::default()
        }]),
    })
}

/// `packet` with its id.
pub(crate) fn encode_update(packet: &ClientboundPlayerInfoUpdate<'_>) -> Option<Bytes> {
    let mut bytes = Vec::new();
    match packet.encode_with_id(&mut bytes) {
        Ok(()) => Some(Bytes::from(bytes)),
        Err(error) => {
            tracing::warn!("a player list entry did not encode: {error:#}");
            None
        }
    }
}

/// Whether the work of `session` that recurs every `interval` ticks falls on `tick`. The
/// session spreads the work of the viewers over the ticks.
fn due(tick: u64, session: PlayerSession, interval: u64) -> bool {
    tick.wrapping_add(session.0)
        .checked_rem(interval)
        .is_some_and(|phase| phase == 0)
}

/// Chooses the listed entries of each viewer, sends the latency of the listed entries and
/// removes the entries that nothing needs any more.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "aoi::update_player_list", skip_all)
)]
pub fn update_player_list(
    tracker: Res<EntityTracker>,
    mut list: ResMut<PlayerList>,
    policy: Res<ListPolicy>,
    profiles: Query<(&GameProfile, Option<&ListedGameMode>, Option<&Latency>)>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let tick = tracker.tick();
    let generation = tracker.generation();
    let PlayerList {
        viewers,
        subjects,
        removals,
    } = &mut *list;
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut chosen: Vec<Entity> = Vec::new();
    for (&viewer, entries) in viewers.iter_mut() {
        let session = entries.session;
        let new = entries.chosen_at.is_none();
        if !new && !due(tick, session, LIST_INTERVAL_TICKS) {
            continue;
        }
        if entries.chosen_at != Some(generation)
            && let Some(own) = tracker.get(viewer)
        {
            if candidates.is_empty() {
                candidates.extend(tracker.iter().map(|(player, subject)| Candidate {
                    player,
                    position: subject.base.position,
                }));
                candidates.sort_unstable_by_key(|candidate| candidate.player);
            }
            chosen.clear();
            (policy.0)(
                &ListRequest {
                    viewer: Candidate {
                        player: viewer,
                        position: own.base.position,
                    },
                    candidates: &candidates,
                    limit: LISTED_CAP.saturating_sub(1),
                },
                &mut chosen,
            );
            entries.chosen_at = Some(generation);
            let listed: EntityHashSet = chosen
                .iter()
                .copied()
                .filter(|&player| player != viewer)
                .take(LISTED_CAP.saturating_sub(1))
                .collect();
            let change = Relisting {
                tracker: &tracker,
                viewer,
                tick,
            };
            change.apply(entries, subjects, &listed, &profiles, &mut writer);
        }
        if !new {
            evict_idle(viewer, entries, subjects, removals, tick);
        }
    }
    for entries in viewers.values_mut() {
        if due(tick, entries.session, LATENCY_INTERVAL_TICKS) {
            send_latency(entries, subjects, &profiles, &mut writer);
        }
    }
    for (viewer, players) in mem::take(removals) {
        if let Some(entries) = viewers.get(&viewer) {
            writer.write(to(
                entries.session,
                PacketPayload::PlayerInfoRemove(ClientboundPlayerInfoRemove { players }),
            ));
        }
    }
}

/// The choice of the listed entries of one viewer.
struct Relisting<'a> {
    tracker: &'a EntityTracker,
    viewer: Entity,
    tick: u64,
}

impl Relisting<'_> {
    fn apply(
        &self,
        entries: &mut ViewerEntries,
        subjects: &mut EntityHashMap<SubjectProfile>,
        chosen: &EntityHashSet,
        profiles: &Query<(&GameProfile, Option<&ListedGameMode>, Option<&Latency>)>,
        writer: &mut MessageWriter<OutboundPlayerPacket>,
    ) {
        let mut listed: Vec<EntryLatency> = Vec::new();
        for &subject in chosen {
            let (Some(record), Ok((profile, game_mode, latency))) =
                (subjects.get_mut(&subject), profiles.get(subject))
            else {
                continue;
            };
            let latency = latency.map_or(0, |latency| latency.0);
            match entries.entries.get_mut(&subject) {
                Some(entry) if entry.listing == Listing::Listed => continue,
                Some(entry) => {
                    entry.listing = Listing::Listed;
                    entry.idle_since = None;
                    entry.sent_latency = latency;
                }
                None => {
                    let game_mode = game_mode.map(|mode| mode.0).unwrap_or_default();
                    let Some(packet) = cached_profile(record, profile, game_mode) else {
                        continue;
                    };
                    writer.write(to(entries.session, PacketPayload::PlayerInfo(packet)));
                    record.holders.insert(self.viewer);
                    entries.entries.insert(
                        subject,
                        Entry {
                            listing: Listing::Listed,
                            idle_since: None,
                            sent_latency: latency,
                        },
                    );
                }
            }
            listed.push(EntryLatency {
                player_uuid: record.uuid,
                latency,
            });
        }
        let mut unlisted: Vec<Uuid> = Vec::new();
        for (&subject, entry) in &mut entries.entries {
            if subject == self.viewer
                || entry.listing == Listing::Unlisted
                || chosen.contains(&subject)
            {
                continue;
            }
            let Some(record) = subjects.get(&subject) else {
                continue;
            };
            entry.listing = Listing::Unlisted;
            entry.idle_since = (!self.tracker.sees(self.viewer, subject)).then_some(self.tick);
            unlisted.push(record.uuid);
        }
        if !listed.is_empty() {
            writer.write(to(
                entries.session,
                PacketPayload::PlayerListChange(ListChange::Listed(listed)),
            ));
        }
        if !unlisted.is_empty() {
            writer.write(to(
                entries.session,
                PacketPayload::PlayerListChange(ListChange::Unlisted(unlisted)),
            ));
        }
    }
}

fn evict_idle(
    viewer: Entity,
    entries: &mut ViewerEntries,
    subjects: &mut EntityHashMap<SubjectProfile>,
    removals: &mut BTreeMap<Entity, Vec<Uuid>>,
    tick: u64,
) {
    entries.entries.retain(|subject, entry| {
        let expired = entry
            .idle_since
            .is_some_and(|since| tick.wrapping_sub(since) >= ENTRY_LINGER_TICKS);
        if !expired {
            return true;
        }
        if let Some(record) = subjects.get_mut(subject) {
            record.holders.remove(&viewer);
            removals.entry(viewer).or_default().push(record.uuid);
        }
        false
    });
}

fn send_latency(
    entries: &mut ViewerEntries,
    subjects: &EntityHashMap<SubjectProfile>,
    profiles: &Query<(&GameProfile, Option<&ListedGameMode>, Option<&Latency>)>,
    writer: &mut MessageWriter<OutboundPlayerPacket>,
) {
    let mut changed: Vec<EntryLatency> = Vec::new();
    for (subject, entry) in &mut entries.entries {
        if entry.listing != Listing::Listed {
            continue;
        }
        let latency = profiles
            .get(*subject)
            .ok()
            .and_then(|(_, _, latency)| latency)
            .map_or(0, |latency| latency.0);
        if latency == entry.sent_latency {
            continue;
        }
        let Some(record) = subjects.get(subject) else {
            continue;
        };
        entry.sent_latency = latency;
        changed.push(EntryLatency {
            player_uuid: record.uuid,
            latency,
        });
    }
    if !changed.is_empty() {
        writer.write(to(
            entries.session,
            PacketPayload::PlayerListChange(ListChange::Latency(changed)),
        ));
    }
}

/// The packet that tells the viewers of the entry of a player its new game mode, encoded once.
pub(crate) fn game_mode_packet(
    list: &PlayerList,
    player: Entity,
    own: PlayerSession,
    uuid: Uuid,
    game_mode: GameMode,
) -> Option<OutboundPlayerPacket> {
    let packet = encode_update(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_game_mode(true),
        entries: Cow::Borrowed(&[PlayerListEntry {
            player_uuid: uuid,
            game_mode,
            ..Default::default()
        }]),
    })?;
    let mut sessions = list.holder_sessions(player);
    if !sessions.contains(&own) {
        sessions.push(own);
    }
    Some(to_target(
        PacketTarget::PlayerSet(sessions),
        PacketPayload::PlayerInfo(packet),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(index: u32, x: f64) -> Candidate {
        Candidate {
            player: Entity::from_raw_u32(index).unwrap(),
            position: DVec3::new(x, 64.0, 0.0),
        }
    }

    #[test]
    fn nearest_lists_the_closest_players_other_than_the_viewer() {
        let candidates = [
            candidate(1, 0.0),
            candidate(2, 50.0),
            candidate(3, -2.0),
            candidate(4, 5.0),
        ];
        let mut listed = Vec::new();

        nearest(
            &ListRequest {
                viewer: candidates[0],
                candidates: &candidates,
                limit: 2,
            },
            &mut listed,
        );

        let mut expected = [candidates[2].player, candidates[3].player];
        listed.sort_unstable();
        expected.sort_unstable();
        assert_eq!(listed, expected);
    }
}
