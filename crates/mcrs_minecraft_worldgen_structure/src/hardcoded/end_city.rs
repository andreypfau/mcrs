use bevy_math::IVec3;
use mcrs_minecraft_core::{Mirror, Rotation, StaticResourceLocation, rl};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::template::{bounding_box, transform};

use super::lowest_corner_site;
use crate::frozen::FrozenStructures;
use crate::piece::{EndCityPiece, Piece};
use crate::site::{Context, Site, Stub};

const BASE_FLOOR: StaticResourceLocation = rl!("minecraft:end_city/base_floor");
const BASE_ROOF: StaticResourceLocation = rl!("minecraft:end_city/base_roof");
const SECOND_FLOOR_1: StaticResourceLocation = rl!("minecraft:end_city/second_floor_1");
const SECOND_FLOOR_2: StaticResourceLocation = rl!("minecraft:end_city/second_floor_2");
const SECOND_ROOF: StaticResourceLocation = rl!("minecraft:end_city/second_roof");
const THIRD_FLOOR_1: StaticResourceLocation = rl!("minecraft:end_city/third_floor_1");
const THIRD_FLOOR_2: StaticResourceLocation = rl!("minecraft:end_city/third_floor_2");
const THIRD_ROOF: StaticResourceLocation = rl!("minecraft:end_city/third_roof");
const TOWER_BASE: StaticResourceLocation = rl!("minecraft:end_city/tower_base");
const TOWER_PIECE: StaticResourceLocation = rl!("minecraft:end_city/tower_piece");
const TOWER_TOP: StaticResourceLocation = rl!("minecraft:end_city/tower_top");
const BRIDGE_PIECE: StaticResourceLocation = rl!("minecraft:end_city/bridge_piece");
const BRIDGE_END: StaticResourceLocation = rl!("minecraft:end_city/bridge_end");
const BRIDGE_STEEP_STAIRS: StaticResourceLocation = rl!("minecraft:end_city/bridge_steep_stairs");
const BRIDGE_GENTLE_STAIRS: StaticResourceLocation = rl!("minecraft:end_city/bridge_gentle_stairs");
const SHIP: StaticResourceLocation = rl!("minecraft:end_city/ship");
const FAT_TOWER_BASE: StaticResourceLocation = rl!("minecraft:end_city/fat_tower_base");
const FAT_TOWER_MIDDLE: StaticResourceLocation = rl!("minecraft:end_city/fat_tower_middle");
const FAT_TOWER_TOP: StaticResourceLocation = rl!("minecraft:end_city/fat_tower_top");

pub const TEMPLATES: &[StaticResourceLocation] = &[
    BASE_FLOOR,
    BASE_ROOF,
    SECOND_FLOOR_1,
    SECOND_FLOOR_2,
    SECOND_ROOF,
    THIRD_FLOOR_1,
    THIRD_FLOOR_2,
    THIRD_ROOF,
    TOWER_BASE,
    TOWER_PIECE,
    TOWER_TOP,
    BRIDGE_PIECE,
    BRIDGE_END,
    BRIDGE_STEEP_STAIRS,
    BRIDGE_GENTLE_STAIRS,
    SHIP,
    FAT_TOWER_BASE,
    FAT_TOWER_MIDDLE,
    FAT_TOWER_TOP,
];

const MAX_GEN_DEPTH: i32 = 8;

const TOWER_BRIDGES: [(Rotation, IVec3); 4] = [
    (Rotation::None, IVec3::new(1, -1, 0)),
    (Rotation::Clockwise90, IVec3::new(6, -1, 1)),
    (Rotation::Counterclockwise90, IVec3::new(0, -1, 5)),
    (Rotation::Clockwise180, IVec3::new(5, -1, 6)),
];

const FAT_TOWER_BRIDGES: [(Rotation, IVec3); 4] = [
    (Rotation::None, IVec3::new(4, -1, 0)),
    (Rotation::Clockwise90, IVec3::new(12, -1, 4)),
    (Rotation::Counterclockwise90, IVec3::new(0, -1, 8)),
    (Rotation::Clockwise180, IVec3::new(8, -1, 12)),
];

pub fn site(ctx: &mut Context<'_>, rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    lowest_corner_site(ctx, rng)
}

pub fn layout(ctx: &mut Context<'_>, site: Site) -> Vec<Piece> {
    let Stub::Rotated(rotation) = site.stub else {
        return Vec::new();
    };
    let mut city = City {
        frozen: ctx.frozen,
        rng: site.rng,
        ship_created: false,
    };
    let mut pieces = Vec::new();
    if city.start(site.position, rotation, &mut pieces).is_none() {
        return Vec::new();
    }
    pieces.into_iter().map(Piece::EndCity).collect()
}

#[derive(Clone, Copy)]
enum Section {
    HouseTower,
    Tower,
    TowerBridge,
    FatTower,
}

struct City<'a> {
    frozen: &'a FrozenStructures,
    rng: LegacyRandom,
    ship_created: bool,
}

fn add(pieces: &mut Vec<EndCityPiece>, piece: EndCityPiece) -> EndCityPiece {
    pieces.push(piece.clone());
    piece
}

impl City<'_> {
    fn start(
        &mut self,
        position: IVec3,
        rotation: Rotation,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        let mut last = add(pieces, self.piece(BASE_FLOOR, position, rotation, true)?);
        last = add(
            pieces,
            self.add_piece(
                &last,
                IVec3::new(-1, 0, -1),
                SECOND_FLOOR_1,
                rotation,
                false,
            )?,
        );
        last = add(
            pieces,
            self.add_piece(&last, IVec3::new(-1, 4, -1), THIRD_FLOOR_1, rotation, false)?,
        );
        last = add(
            pieces,
            self.add_piece(&last, IVec3::new(-1, 8, -1), THIRD_ROOF, rotation, true)?,
        );
        self.recursive_children(Section::Tower, 1, &last, IVec3::ZERO, pieces)
    }

    fn piece(
        &self,
        name: StaticResourceLocation,
        position: IVec3,
        rotation: Rotation,
        overwrite: bool,
    ) -> Option<EndCityPiece> {
        let (template, size) = self.frozen.sized_template(name.as_str())?;
        Some(EndCityPiece {
            template,
            position,
            rotation,
            overwrite,
            bounds: bounding_box(size, position, rotation, Mirror::None, IVec3::ZERO),
            gen_depth: 0,
        })
    }

    /// The child at `offset` in the parent's frame, both at pivot zero.
    fn add_piece(
        &self,
        parent: &EndCityPiece,
        offset: IVec3,
        name: StaticResourceLocation,
        rotation: Rotation,
        overwrite: bool,
    ) -> Option<EndCityPiece> {
        let position =
            parent.position + transform(offset, Mirror::None, parent.rotation, IVec3::ZERO);
        self.piece(name, position, rotation, overwrite)
    }

    /// A section's children join `pieces` only when none of them meets a piece
    /// tagged differently from the parent; every child then takes one fresh tag.
    fn recursive_children(
        &mut self,
        section: Section,
        depth: i32,
        parent: &EndCityPiece,
        offset: IVec3,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        if depth > MAX_GEN_DEPTH {
            return Some(false);
        }
        let mut children = Vec::new();
        if !self.generate(section, depth, parent, offset, &mut children)? {
            return Some(false);
        }
        let tag = self.rng.next_i32();
        let collision = children.iter().any(|child| {
            pieces
                .iter()
                .find(|piece| piece.bounds.intersects(child.bounds))
                .is_some_and(|hit| hit.gen_depth != parent.gen_depth)
        });
        if collision {
            return Some(false);
        }
        for child in &mut children {
            child.gen_depth = tag;
        }
        pieces.extend(children);
        Some(true)
    }

    fn generate(
        &mut self,
        section: Section,
        depth: i32,
        parent: &EndCityPiece,
        offset: IVec3,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        match section {
            Section::HouseTower => self.house_tower(depth, parent, offset, pieces),
            Section::Tower => self.tower(depth, parent, pieces),
            Section::TowerBridge => self.tower_bridge(depth, parent, pieces),
            Section::FatTower => self.fat_tower(depth, parent, pieces),
        }
    }

    fn house_tower(
        &mut self,
        depth: i32,
        parent: &EndCityPiece,
        offset: IVec3,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        if depth > MAX_GEN_DEPTH {
            return Some(false);
        }
        let rotation = parent.rotation;
        let mut last = add(
            pieces,
            self.add_piece(parent, offset, BASE_FLOOR, rotation, true)?,
        );
        match self.rng.next_i32_bound(3) {
            0 => {
                add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 4, -1), BASE_ROOF, rotation, true)?,
                );
            }
            1 => {
                last = add(
                    pieces,
                    self.add_piece(
                        &last,
                        IVec3::new(-1, 0, -1),
                        SECOND_FLOOR_2,
                        rotation,
                        false,
                    )?,
                );
                last = add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 8, -1), SECOND_ROOF, rotation, false)?,
                );
                self.recursive_children(Section::Tower, depth + 1, &last, IVec3::ZERO, pieces)?;
            }
            _ => {
                last = add(
                    pieces,
                    self.add_piece(
                        &last,
                        IVec3::new(-1, 0, -1),
                        SECOND_FLOOR_2,
                        rotation,
                        false,
                    )?,
                );
                last = add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 4, -1), THIRD_FLOOR_2, rotation, false)?,
                );
                last = add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 8, -1), THIRD_ROOF, rotation, true)?,
                );
                self.recursive_children(Section::Tower, depth + 1, &last, IVec3::ZERO, pieces)?;
            }
        }
        Some(true)
    }

    fn tower(
        &mut self,
        depth: i32,
        parent: &EndCityPiece,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        let rotation = parent.rotation;
        let base = IVec3::new(
            3 + self.rng.next_i32_bound(2),
            -3,
            3 + self.rng.next_i32_bound(2),
        );
        let mut last = add(
            pieces,
            self.add_piece(parent, base, TOWER_BASE, rotation, true)?,
        );
        last = add(
            pieces,
            self.add_piece(&last, IVec3::new(0, 7, 0), TOWER_PIECE, rotation, true)?,
        );
        let mut bridge_piece = (self.rng.next_i32_bound(3) == 0).then(|| last.clone());
        let tower_height = 1 + self.rng.next_i32_bound(3);
        for i in 0..tower_height {
            last = add(
                pieces,
                self.add_piece(&last, IVec3::new(0, 4, 0), TOWER_PIECE, rotation, true)?,
            );
            if i < tower_height - 1 && self.rng.next_bool() {
                bridge_piece = Some(last.clone());
            }
        }
        match bridge_piece {
            Some(bridge_piece) => {
                for (turn, offset) in TOWER_BRIDGES {
                    if self.rng.next_bool() {
                        let bridge_start = add(
                            pieces,
                            self.add_piece(
                                &bridge_piece,
                                offset,
                                BRIDGE_END,
                                rotation.rotated(turn),
                                true,
                            )?,
                        );
                        self.recursive_children(
                            Section::TowerBridge,
                            depth + 1,
                            &bridge_start,
                            IVec3::ZERO,
                            pieces,
                        )?;
                    }
                }
            }
            None if depth != 7 => {
                return self.recursive_children(
                    Section::FatTower,
                    depth + 1,
                    &last,
                    IVec3::ZERO,
                    pieces,
                );
            }
            None => {}
        }
        add(
            pieces,
            self.add_piece(&last, IVec3::new(-1, 4, -1), TOWER_TOP, rotation, true)?,
        );
        Some(true)
    }

    fn tower_bridge(
        &mut self,
        depth: i32,
        parent: &EndCityPiece,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        let rotation = parent.rotation;
        let bridge_length = self.rng.next_i32_bound(4) + 1;
        let mut first =
            self.add_piece(parent, IVec3::new(0, 0, -4), BRIDGE_PIECE, rotation, true)?;
        first.gen_depth = -1;
        let mut last = add(pieces, first);
        let mut next_y = 0;
        for _ in 0..bridge_length {
            if self.rng.next_bool() {
                last = add(
                    pieces,
                    self.add_piece(
                        &last,
                        IVec3::new(0, next_y, -4),
                        BRIDGE_PIECE,
                        rotation,
                        true,
                    )?,
                );
                next_y = 0;
            } else {
                let (name, offset) = if self.rng.next_bool() {
                    (BRIDGE_STEEP_STAIRS, IVec3::new(0, next_y, -4))
                } else {
                    (BRIDGE_GENTLE_STAIRS, IVec3::new(0, next_y, -8))
                };
                last = add(pieces, self.add_piece(&last, offset, name, rotation, true)?);
                next_y = 4;
            }
        }
        if !self.ship_created && self.rng.next_i32_bound(10 - depth) == 0 {
            let offset = IVec3::new(
                -8 + self.rng.next_i32_bound(8),
                next_y,
                -70 + self.rng.next_i32_bound(10),
            );
            add(pieces, self.add_piece(&last, offset, SHIP, rotation, true)?);
            self.ship_created = true;
        } else if !self.recursive_children(
            Section::HouseTower,
            depth + 1,
            &last,
            IVec3::new(-3, next_y + 1, -11),
            pieces,
        )? {
            return Some(false);
        }
        let mut end = self.add_piece(
            &last,
            IVec3::new(4, next_y, 0),
            BRIDGE_END,
            rotation.rotated(Rotation::Clockwise180),
            true,
        )?;
        end.gen_depth = -1;
        add(pieces, end);
        Some(true)
    }

    fn fat_tower(
        &mut self,
        depth: i32,
        parent: &EndCityPiece,
        pieces: &mut Vec<EndCityPiece>,
    ) -> Option<bool> {
        let rotation = parent.rotation;
        let mut last = add(
            pieces,
            self.add_piece(
                parent,
                IVec3::new(-3, 4, -3),
                FAT_TOWER_BASE,
                rotation,
                true,
            )?,
        );
        last = add(
            pieces,
            self.add_piece(&last, IVec3::new(0, 4, 0), FAT_TOWER_MIDDLE, rotation, true)?,
        );
        for _ in 0..2 {
            if self.rng.next_i32_bound(3) == 0 {
                break;
            }
            last = add(
                pieces,
                self.add_piece(&last, IVec3::new(0, 8, 0), FAT_TOWER_MIDDLE, rotation, true)?,
            );
            for (turn, offset) in FAT_TOWER_BRIDGES {
                if self.rng.next_bool() {
                    let bridge_start = add(
                        pieces,
                        self.add_piece(&last, offset, BRIDGE_END, rotation.rotated(turn), true)?,
                    );
                    self.recursive_children(
                        Section::TowerBridge,
                        depth + 1,
                        &bridge_start,
                        IVec3::ZERO,
                        pieces,
                    )?;
                }
            }
        }
        add(
            pieces,
            self.add_piece(&last, IVec3::new(-2, 8, -2), FAT_TOWER_TOP, rotation, true)?,
        );
        Some(true)
    }
}
