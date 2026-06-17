use bevy_ecs::entity::Entity;
use mcrs_engine::session::{
    DimPlayerIndex, PlayerSession, PlayerSessionCounter, SessionEntry, SessionRegistry,
};

#[test]
fn counter_starts_at_one_and_never_repeats() {
    let mut counter = PlayerSessionCounter::default();
    let mut sessions = Vec::with_capacity(1000);
    for _ in 0..1000 {
        sessions.push(counter.next());
    }
    assert_eq!(sessions[0], PlayerSession(1), "first allocation must be PlayerSession(1)");
    for i in 1..sessions.len() {
        assert!(
            sessions[i].0 > sessions[i - 1].0,
            "session ids must be strictly increasing: {:?} <= {:?}",
            sessions[i],
            sessions[i - 1],
        );
    }
    assert!(
        sessions.iter().all(|s| s.0 != 0),
        "PlayerSession(0) must never be produced"
    );
}

#[test]
fn registry_insert_get_roundtrip() {
    let mut registry = SessionRegistry::default();
    let session = PlayerSession(1);
    let conn = Entity::from_raw_u32(10).unwrap();
    let dim = Entity::from_raw_u32(20).unwrap();
    let anchor = Entity::from_raw_u32(30).unwrap();
    registry.insert(
        session,
        SessionEntry {
            connection_entity: conn,
            host_anchor: anchor,
            dim,
            previous_dim: None,
            in_dim_entity: None,
            epoch: 3,
        },
    );
    let entry = registry.get(&session).expect("just inserted");
    assert_eq!(entry.connection_entity, conn);
    assert_eq!(entry.dim, dim);
    assert_eq!(entry.epoch, 3);
}

#[test]
fn registry_get_mut_mutates_epoch() {
    let mut registry = SessionRegistry::default();
    let session = PlayerSession(2);
    let conn = Entity::from_raw_u32(1).unwrap();
    let dim = Entity::from_raw_u32(2).unwrap();
    let anchor = Entity::from_raw_u32(3).unwrap();
    registry.insert(
        session,
        SessionEntry {
            connection_entity: conn,
            host_anchor: anchor,
            dim,
            previous_dim: None,
            in_dim_entity: None,
            epoch: 0,
        },
    );
    registry.get_mut(&session).expect("present").epoch = 7;
    assert_eq!(registry.get(&session).expect("still present").epoch, 7);
}

#[test]
fn registry_remove_returns_then_none() {
    let mut registry = SessionRegistry::default();
    let session = PlayerSession(3);
    let conn = Entity::from_raw_u32(5).unwrap();
    let dim = Entity::from_raw_u32(6).unwrap();
    let anchor = Entity::from_raw_u32(7).unwrap();
    registry.insert(
        session,
        SessionEntry {
            connection_entity: conn,
            host_anchor: anchor,
            dim,
            previous_dim: None,
            in_dim_entity: None,
            epoch: 0,
        },
    );
    assert!(registry.remove(&session).is_some());
    assert!(registry.remove(&session).is_none());
}

#[test]
fn registry_iter_in_dim_filters_by_dim() {
    let mut registry = SessionRegistry::default();
    let dim_a = Entity::from_raw_u32(100).unwrap();
    let dim_b = Entity::from_raw_u32(200).unwrap();
    let conn = Entity::from_raw_u32(1).unwrap();

    let s1 = PlayerSession(1);
    let s2 = PlayerSession(2);
    let s3 = PlayerSession(3);

    let a1 = Entity::from_raw_u32(11).unwrap();
    let a2 = Entity::from_raw_u32(12).unwrap();
    let a3 = Entity::from_raw_u32(13).unwrap();
    registry.insert(s1, SessionEntry { connection_entity: conn, host_anchor: a1, dim: dim_a, previous_dim: None, in_dim_entity: None, epoch: 0 });
    registry.insert(s2, SessionEntry { connection_entity: conn, host_anchor: a2, dim: dim_a, previous_dim: None, in_dim_entity: None, epoch: 0 });
    registry.insert(s3, SessionEntry { connection_entity: conn, host_anchor: a3, dim: dim_b, previous_dim: None, in_dim_entity: None, epoch: 0 });

    let in_a: Vec<PlayerSession> = registry.iter_in_dim(dim_a).map(|(s, _)| *s).collect();
    assert_eq!(in_a.len(), 2, "dim_a should have exactly 2 sessions");
    assert!(in_a.contains(&s1));
    assert!(in_a.contains(&s2));

    let in_b: Vec<PlayerSession> = registry.iter_in_dim(dim_b).map(|(s, _)| *s).collect();
    assert_eq!(in_b.len(), 1, "dim_b should have exactly 1 session");
    assert!(in_b.contains(&s3));
}

#[test]
fn dim_player_index_default_empty() {
    let index = DimPlayerIndex::default();
    assert!(index.0.is_empty());
}

#[test]
fn dim_player_index_insert_lookup_roundtrip() {
    let mut index = DimPlayerIndex::default();
    let session = PlayerSession(1);
    let entity = Entity::from_raw_u32(42).unwrap();
    index.0.insert(session, entity);
    assert_eq!(index.0.get(&session).copied(), Some(entity));
    index.0.remove(&session);
    assert!(index.0.get(&session).is_none());
}
