use bevy_ecs::entity::Entity;
use mcrs_minecraft_level::session::{
    DimPlayerIndex, Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};

fn dim(index: u32) -> Entity {
    Entity::from_raw_u32(index).unwrap()
}

#[test]
fn counter_starts_at_one_and_never_repeats() {
    let mut counter = PlayerSessionCounter::default();
    let mut sessions = Vec::with_capacity(1000);
    for _ in 0..1000 {
        sessions.push(counter.next());
    }
    assert_eq!(
        sessions[0],
        PlayerSession(1),
        "first allocation must be PlayerSession(1)"
    );
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
fn joining_a_first_dimension_keeps_the_epoch() {
    let mut placement = SessionPlacement::default();
    placement.set(Place::Joining(dim(1)));
    placement.set(Place::InDim(dim(1)));
    assert_eq!(placement.place(), Place::InDim(dim(1)));
    assert_eq!(placement.epoch(), 0);
}

#[test]
fn leaving_a_dimension_advances_the_epoch() {
    let mut placement = SessionPlacement::new(Place::InDim(dim(1)), 3);
    placement.set(Place::Transferring {
        from: dim(1),
        to: dim(2),
    });
    assert_eq!(placement.epoch(), 4, "moving to another dimension");
    placement.set(Place::InDim(dim(2)));
    assert_eq!(placement.epoch(), 4, "arriving where the move was headed");
    placement.set(Place::Unplaced);
    assert_eq!(placement.epoch(), 5, "leaving for reconfiguration");
}

#[test]
fn a_move_within_one_dimension_keeps_the_epoch() {
    let mut placement = SessionPlacement::new(Place::InDim(dim(1)), 0);
    placement.set(Place::Transferring {
        from: dim(1),
        to: dim(1),
    });
    assert_eq!(placement.epoch(), 0);
}

#[test]
fn packets_belong_to_the_destination_but_only_an_attached_dimension_takes_them() {
    let (a, b) = (dim(1), dim(2));
    let transfer = Place::Transferring { from: a, to: b };
    assert_eq!(Place::Unplaced.dim(), None);
    assert_eq!(Place::Joining(a).dim(), Some(a));
    assert_eq!(transfer.dim(), Some(b));
    assert_eq!(Place::Joining(a).attached(), None);
    assert_eq!(transfer.attached(), None);
    assert_eq!(Place::InDim(a).attached(), Some(a));
}

#[test]
fn a_transfer_is_held_by_both_ends() {
    let (a, b) = (dim(1), dim(2));
    let holding = |place: Place| place.holding_dims().collect::<Vec<_>>();
    assert!(holding(Place::Unplaced).is_empty());
    assert_eq!(holding(Place::Joining(a)), vec![a]);
    assert_eq!(holding(Place::InDim(a)), vec![a]);
    assert_eq!(holding(Place::Transferring { from: a, to: b }), vec![b, a]);
    assert_eq!(holding(Place::Transferring { from: a, to: a }), vec![a]);
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
