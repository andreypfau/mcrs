use mcrs_minecraft_entity_key::EntityId;

use crate::host_mob::HostMob;

/// The index in `hosts`, which is sorted by id, of the host of each row of `ids`; `None` for a
/// row that has no host. One merge of the sorted rows with the hosts, in place of a binary search
/// a row over the large hosts.
pub(crate) fn row_hosts(ids: &[EntityId], hosts: &[HostMob]) -> Vec<Option<usize>> {
    let mut order: Vec<(EntityId, usize)> =
        ids.iter().enumerate().map(|(row, &id)| (id, row)).collect();
    order.sort_unstable();
    let mut slots = vec![None; ids.len()];
    let mut next = 0;
    for (id, row) in order {
        while hosts.get(next).is_some_and(|host| host.id < id) {
            next += 1;
        }
        if hosts.get(next).is_some_and(|host| host.id == id)
            && let Some(slot) = slots.get_mut(row)
        {
            *slot = Some(next);
        }
    }
    slots
}
