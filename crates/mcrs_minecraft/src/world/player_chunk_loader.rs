// use bevy_ecs::prelude::*;
// use mcrs_engine::world::chunk::ChunkPos;
//
// #[derive(Debug, Clone, Component, PartialEq, Eq)]
// struct ViewDistances {
//     pub tick: Distances,
//     pub load: Distances,
//     pub send: Distances,
// }
//
// impl ViewDistances {
//     pub fn new(
//         player_distances: Option<&ViewDistances>,
//         dim_distances: &ViewDistances,
//         player_view_distance: Option<u8>,
//         auto_config: bool,
//     ) -> Self {
//         let tick_horizontal_distance = Self::get_tick_distance(
//             player_distances.map(|d| d.tick),
//             dim_distances.tick,
//             player_distances.map(|d| d.load),
//             dim_distances.load,
//         );
//         let load_horizontal_distance = Self::get_load_distance(
//             tick_horizontal_distance,
//             player_distances.map(|d| d.load),
//             dim_distances.load,
//         );
//         let send_distance = Self::get_send_distance(
//             load_horizontal_distance,
//             player_view_distance.map(|d| Distances::from(d)),
//             player_distances.map(|d| d.send),
//             dim_distances.send,
//             auto_config,
//         );
//         Self {
//             tick: tick_horizontal_distance,
//             load: load_horizontal_distance,
//             send: send_distance,
//         }
//     }
//
//     fn get_tick_distance(
//         player_tick_distance: Option<Distances>,
//         world_tick_distance: Distances,
//         player_load_distance: Option<Distances>,
//         world_load_distance: Distances,
//     ) -> Distances {
//         let tick_distance = player_tick_distance.unwrap_or_else(|| world_tick_distance);
//         let load_distance_horizontal = match player_load_distance {
//             Some(dist) => dist.horizontal.saturating_sub(1),
//             None => world_load_distance.horizontal.saturating_sub(1),
//         };
//         let load_distance_vertical = match player_load_distance {
//             Some(dist) => dist.vertical.saturating_sub(1),
//             None => world_load_distance.vertical.saturating_sub(1),
//         };
//         Distances {
//             horizontal: tick_distance.horizontal.min(load_distance_horizontal),
//             vertical: tick_distance.vertical.min(load_distance_vertical),
//         }
//     }
//
//     fn get_load_distance(
//         tick_distance: Distances,
//         player_load_distance: Option<Distances>,
//         world_load_distance: Distances,
//     ) -> Distances {
//         let load_distance = player_load_distance.unwrap_or_else(|| world_load_distance);
//         Distances {
//             horizontal: tick_distance
//                 .horizontal
//                 .saturating_add(1)
//                 .max(load_distance.horizontal),
//             vertical: tick_distance
//                 .vertical
//                 .saturating_add(1)
//                 .max(load_distance.vertical),
//         }
//     }
//
//     fn get_send_distance(
//         load_distance: Distances,
//         client_distance: Option<Distances>,
//         player_send_distance: Option<Distances>,
//         world_send_distance: Distances,
//         auto_config: bool,
//     ) -> Distances {
//         let horizontal = {
//             let base = load_distance.horizontal.saturating_sub(1);
//             let send_distance = match player_send_distance {
//                 Some(dist) => dist.horizontal,
//                 None => match client_distance {
//                     Some(dist) if dist.horizontal < base => {
//                         if !auto_config || dist.horizontal == 0 {
//                             world_send_distance.horizontal
//                         } else {
//                             dist.horizontal.saturating_add(1)
//                         }
//                     }
//                     _ => {
//                         if world_send_distance.horizontal == 0 {
//                             base
//                         } else {
//                             world_send_distance.horizontal
//                         }
//                     }
//                 },
//             };
//             base.min(send_distance)
//         };
//         let vertical = {
//             let base = load_distance.vertical.saturating_sub(1);
//             let send_distance = match player_send_distance {
//                 Some(dist) => dist.vertical,
//                 None => match client_distance {
//                     Some(dist) if dist.vertical < base => {
//                         if !auto_config || dist.vertical == 0 {
//                             world_send_distance.vertical
//                         } else {
//                             dist.vertical.saturating_add(1)
//                         }
//                     }
//                     _ => {
//                         if world_send_distance.vertical == 0 {
//                             base
//                         } else {
//                             world_send_distance.vertical
//                         }
//                     }
//                 },
//             };
//             base.min(send_distance)
//         };
//         Distances {
//             horizontal,
//             vertical,
//         }
//     }
// }
//
// impl Default for ViewDistances {
//     fn default() -> Self {
//         Self {
//             tick: Distances::from(8),
//             load: Distances::from(12),
//             send: Distances::from(10),
//         }
//     }
// }
//
// fn update(
//     view_distances: &ViewDistances,
//     last_view_distances: &mut Option<ViewDistances>,
//     chunk_pos: &ChunkPos,
//     last_chunk_pos: &mut Option<ChunkPos>,
//     player_chunk_commands: &mut PlayerChunkCommands,
// ) {
//     let same_view_distances = Some(view_distances) == last_view_distances.as_ref();
//     let same_chunk_pos = Some(chunk_pos) == last_chunk_pos.as_ref();
//     if same_view_distances && same_chunk_pos {
//         return;
//     }
//     if !same_chunk_pos {
//         player_chunk_commands.push(PlayerChunkCommand::UpdateClientChunkCenter(*chunk_pos));
//     }
//     if Some(view_distances.tick) != last_view_distances.as_ref().map(|d| d.tick) {
//         player_chunk_commands.push(PlayerChunkCommand::UpdateClientSimulationDistance(
//             view_distances.tick,
//         ));
//     }
//
//     let old_view_distance = last_view_distances.as_ref().unwrap_or(view_distances);
//     let from = last_chunk_pos.as_ref().unwrap_or(chunk_pos);
//
//     let delta = *chunk_pos - *from;
//     let total_x = from.x - chunk_pos.x;
//     let total_y = from.y - chunk_pos.y;
//     let total_z = from.z - chunk_pos.z;
// }
//
// #[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
// struct Distances {
//     horizontal: u8,
//     vertical: u8,
// }
//
// impl From<u8> for Distances {
//     fn from(value: u8) -> Self {
//         Self {
//             horizontal: value,
//             vertical: value,
//         }
//     }
// }
//
// #[derive(Debug, Default, Component)]
// struct PlayerChunkCommands {
//     vec: Vec<PlayerChunkCommand>,
// }
//
// impl PlayerChunkCommands {
//     pub fn push(&mut self, cmd: PlayerChunkCommand) {
//         self.vec.push(cmd);
//     }
// }
//
// #[derive(Debug, Clone)]
// enum PlayerChunkCommand {
//     UpdateClientChunkRadius(Distances),
//     UpdateClientSimulationDistance(Distances),
//     UpdateClientChunkCenter(ChunkPos),
// }
