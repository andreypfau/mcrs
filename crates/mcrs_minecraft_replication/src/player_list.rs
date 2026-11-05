//! The profile entries that each viewer has on its client, and which of them its player list
//! shows.
//!
//! A viewer has the entries of its visible set, which are not listed, and at most
//! [`LISTED_CAP`] listed entries: the nearest players. An entry that nothing needs any
//! more stays on the client for [`ENTRY_LINGER_TICKS`], so a subject at the edge of the track
//! radius does not send its profile again on each return.

use crate::list_packets::{
    EntryLatency, Profile, latency_packet, listed_packet, profile_entry, remove_packet,
    unlisted_packet,
};
use crate::nearest::nearest;
use bytes::Bytes;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use uuid::Uuid;

/// The most entries that the client shows in its player list, the viewer itself included.
pub const LISTED_CAP: usize = 80;

/// A viewer checks its listed entries at most this often, in ticks.
pub const LIST_INTERVAL_TICKS: u64 = 20;

/// The ticks that an entry which nothing needs stays on the client before its removal.
pub const ENTRY_LINGER_TICKS: u64 = 600;

/// A viewer gets the latency of its listed entries at most this often, in ticks: 60 seconds.
pub const LATENCY_INTERVAL_TICKS: u64 = 1200;

/// Whether a spawn must carry the profile entry of its subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileNeed {
    Send,
    Skip,
}

/// A player that the tracker holds, as the list sees it at one tick.
#[derive(Clone, Copy, Debug)]
pub struct Member<'a> {
    pub player: EntityId,
    pub position: DVec3,
    pub profile: Profile<'a>,
    pub game_mode: GameMode,
    /// The round trip in milliseconds.
    pub latency: i32,
}

/// The time of one update of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    /// The tracker tick. No interval of the list reads the wall clock.
    pub tick: u64,
    /// Changes whenever a player joins, leaves or changes section.
    pub generation: u64,
}

/// A packet for one viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct Dispatch {
    pub viewer: EntityId,
    pub packet: Bytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Listing {
    Listed,
    Unlisted,
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
    /// Spreads the periodic work of the viewers over the ticks.
    phase: u64,
    entries: BTreeMap<EntityId, Entry>,
    /// The generation of the last choice of the listed entries.
    chosen_at: Option<u64>,
}

#[derive(Debug)]
struct SubjectProfile {
    uuid: Uuid,
    /// The viewers that have the entry, the subject itself included.
    holders: BTreeSet<EntityId>,
    encoded: Option<EncodedProfile>,
}

#[derive(Debug)]
struct EncodedProfile {
    game_mode: GameMode,
    packet: Bytes,
}

/// The profile entries of the viewers of a world.
#[derive(Default, Debug)]
pub struct PlayerList {
    viewers: BTreeMap<EntityId, ViewerEntries>,
    subjects: BTreeMap<EntityId, SubjectProfile>,
    /// The entries that each viewer loses at the next update.
    removals: BTreeMap<EntityId, Vec<Uuid>>,
}

impl PlayerList {
    /// Adds `player`, whose own listed entry its login or its respawn gave its client. `phase`
    /// spreads the periodic work of the viewers over the ticks.
    pub fn join(&mut self, player: EntityId, phase: u64, uuid: Uuid) {
        let own = Entry {
            listing: Listing::Listed,
            idle_since: None,
            sent_latency: 0,
        };
        self.viewers.insert(
            player,
            ViewerEntries {
                phase,
                entries: BTreeMap::from([(player, own)]),
                chosen_at: None,
            },
        );
        self.subjects.insert(
            player,
            SubjectProfile {
                uuid,
                holders: BTreeSet::from([player]),
                encoded: None,
            },
        );
    }

    /// Records that `viewer` gains `subject` in its visible set.
    pub fn acquire(&mut self, viewer: EntityId, subject: EntityId) -> ProfileNeed {
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
    pub fn release(&mut self, viewer: EntityId, subject: EntityId, tick: u64) {
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
    pub fn leave(&mut self, player: EntityId) -> Result<Option<Bytes>> {
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
        let Some(entries) = self.viewers.remove(&player) else {
            return Ok(None);
        };
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
        if players.is_empty() {
            return Ok(None);
        }
        remove_packet(players).map(Some)
    }

    /// The viewers that have the entry of `subject`. The host sends them the game mode packet.
    pub fn holders(&self, subject: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.subjects
            .get(&subject)
            .into_iter()
            .flat_map(|profile| profile.holders.iter().copied())
    }

    /// The profile entry of `subject`, encoded once for each game mode that it has.
    pub fn profile_packet(
        &mut self,
        subject: EntityId,
        profile: &Profile<'_>,
        game_mode: GameMode,
    ) -> Result<Bytes> {
        let Some(record) = self.subjects.get_mut(&subject) else {
            return profile_entry(profile, game_mode);
        };
        cached_profile(record, profile, game_mode)
    }

    /// Chooses the listed entries of each viewer, gathers the latency of the listed entries and
    /// the removal of the entries that nothing needs any more. `roster` holds the tracked
    /// players sorted by player. `sees(viewer, subject)` tells whether the visible set of the
    /// viewer holds the subject. After an error, the dispatches in `out` stay valid.
    pub fn update<F: Fn(EntityId, EntityId) -> bool>(
        &mut self,
        clock: Clock,
        roster: &[Member<'_>],
        sees: F,
        out: &mut Vec<Dispatch>,
    ) -> Result<()> {
        let Clock { tick, generation } = clock;
        let Self {
            viewers,
            subjects,
            removals,
        } = self;
        for (&viewer, entries) in viewers.iter_mut() {
            let new = entries.chosen_at.is_none();
            if !new && !due(tick, entries.phase, LIST_INTERVAL_TICKS) {
                continue;
            }
            if entries.chosen_at != Some(generation)
                && let Some(own) = member(roster, viewer)
            {
                let listed = nearest(viewer, own.position, roster, LISTED_CAP.saturating_sub(1));
                entries.chosen_at = Some(generation);
                let relisting = Relisting {
                    viewer,
                    tick,
                    roster,
                    sees: &sees,
                };
                relisting.apply(entries, subjects, &listed, out)?;
            }
            if !new {
                evict_idle(viewer, entries, subjects, removals, tick);
            }
        }
        for (&viewer, entries) in viewers.iter_mut() {
            if due(tick, entries.phase, LATENCY_INTERVAL_TICKS) {
                send_latency(viewer, entries, subjects, roster, out)?;
            }
        }
        for (viewer, players) in mem::take(removals) {
            if viewers.contains_key(&viewer) {
                out.push(Dispatch {
                    viewer,
                    packet: remove_packet(players)?,
                });
            }
        }
        Ok(())
    }
}

fn cached_profile(
    record: &mut SubjectProfile,
    profile: &Profile<'_>,
    game_mode: GameMode,
) -> Result<Bytes> {
    if let Some(encoded) = &record.encoded
        && encoded.game_mode == game_mode
    {
        return Ok(encoded.packet.clone());
    }
    let packet = profile_entry(profile, game_mode)?;
    record.encoded = Some(EncodedProfile {
        game_mode,
        packet: packet.clone(),
    });
    Ok(packet)
}

fn member<'r, 'a>(roster: &'r [Member<'a>], player: EntityId) -> Option<&'r Member<'a>> {
    let index = roster
        .binary_search_by_key(&player, |member| member.player)
        .ok()?;
    roster.get(index)
}

/// Whether the work of a viewer that recurs every `interval` ticks falls on `tick`.
fn due(tick: u64, phase: u64, interval: u64) -> bool {
    tick.wrapping_add(phase)
        .checked_rem(interval)
        .is_some_and(|rest| rest == 0)
}

/// The choice of the listed entries of one viewer.
struct Relisting<'a, F> {
    viewer: EntityId,
    tick: u64,
    roster: &'a [Member<'a>],
    sees: &'a F,
}

impl<F: Fn(EntityId, EntityId) -> bool> Relisting<'_, F> {
    fn apply(
        &self,
        entries: &mut ViewerEntries,
        subjects: &mut BTreeMap<EntityId, SubjectProfile>,
        chosen: &BTreeSet<EntityId>,
        out: &mut Vec<Dispatch>,
    ) -> Result<()> {
        let mut listed: Vec<EntryLatency> = Vec::new();
        for &subject in chosen {
            let (Some(record), Some(member)) =
                (subjects.get_mut(&subject), member(self.roster, subject))
            else {
                continue;
            };
            let latency = member.latency;
            match entries.entries.get_mut(&subject) {
                Some(entry) if entry.listing == Listing::Listed => continue,
                Some(entry) => {
                    entry.listing = Listing::Listed;
                    entry.idle_since = None;
                    entry.sent_latency = latency;
                }
                None => {
                    let packet = cached_profile(record, &member.profile, member.game_mode)?;
                    out.push(Dispatch {
                        viewer: self.viewer,
                        packet,
                    });
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
            entry.idle_since = (!(self.sees)(self.viewer, subject)).then_some(self.tick);
            unlisted.push(record.uuid);
        }
        // The unlisting goes first, so the client never lists more than the cap.
        if !unlisted.is_empty() {
            out.push(Dispatch {
                viewer: self.viewer,
                packet: unlisted_packet(&unlisted)?,
            });
        }
        if !listed.is_empty() {
            out.push(Dispatch {
                viewer: self.viewer,
                packet: listed_packet(&listed)?,
            });
        }
        Ok(())
    }
}

fn evict_idle(
    viewer: EntityId,
    entries: &mut ViewerEntries,
    subjects: &mut BTreeMap<EntityId, SubjectProfile>,
    removals: &mut BTreeMap<EntityId, Vec<Uuid>>,
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
    viewer: EntityId,
    entries: &mut ViewerEntries,
    subjects: &BTreeMap<EntityId, SubjectProfile>,
    roster: &[Member<'_>],
    out: &mut Vec<Dispatch>,
) -> Result<()> {
    let mut changed: Vec<EntryLatency> = Vec::new();
    for (&subject, entry) in &mut entries.entries {
        if entry.listing != Listing::Listed {
            continue;
        }
        let latency = member(roster, subject).map_or(0, |member| member.latency);
        if latency == entry.sent_latency {
            continue;
        }
        let Some(record) = subjects.get(&subject) else {
            continue;
        };
        entry.sent_latency = latency;
        changed.push(EntryLatency {
            player_uuid: record.uuid,
            latency,
        });
    }
    if !changed.is_empty() {
        out.push(Dispatch {
            viewer,
            packet: latency_packet(&changed)?,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: EntityId = EntityId(1);
    const SECOND: EntityId = EntityId(2);

    fn member_at(player: EntityId, x: f64) -> Member<'static> {
        Member {
            player,
            position: DVec3::new(x, 64.0, 0.0),
            profile: Profile {
                id: Uuid::from_u128(u128::from(player.0)),
                username: "player",
                properties: &[],
            },
            game_mode: GameMode::default(),
            latency: 0,
        }
    }

    fn list_of_two() -> PlayerList {
        let mut list = PlayerList::default();
        list.join(FIRST, 0, Uuid::from_u128(1));
        list.join(SECOND, 0, Uuid::from_u128(2));
        list
    }

    #[test]
    fn a_held_entry_needs_no_second_profile() {
        let mut list = list_of_two();

        assert_eq!(list.acquire(FIRST, SECOND), ProfileNeed::Send);
        assert_eq!(list.acquire(FIRST, SECOND), ProfileNeed::Skip);
    }

    #[test]
    fn an_entry_that_nothing_needs_is_removed_after_the_linger() -> Result<()> {
        let mut list = list_of_two();
        let roster = [member_at(FIRST, 0.0)];
        let _ = list.acquire(FIRST, SECOND);
        list.release(FIRST, SECOND, 100);
        let mut out = Vec::new();
        list.update(
            Clock {
                tick: 1,
                generation: 1,
            },
            &roster,
            |_, _| false,
            &mut out,
        )?;
        out.clear();

        let before = Clock {
            tick: 100 + ENTRY_LINGER_TICKS - 20,
            generation: 1,
        };
        list.update(before, &roster, |_, _| false, &mut out)?;
        assert!(out.is_empty(), "{out:?}");

        let after = Clock {
            tick: 100 + ENTRY_LINGER_TICKS,
            generation: 1,
        };
        list.update(after, &roster, |_, _| false, &mut out)?;
        let expected = Dispatch {
            viewer: FIRST,
            packet: remove_packet(vec![Uuid::from_u128(2)])?,
        };
        assert_eq!(out, [expected]);
        Ok(())
    }
}
