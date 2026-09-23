use bevy_math::IVec3;
use mcrs_minecraft_core::{Mirror, ResourceLocation, Rotation};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::template::{bounding_box, transform};

use super::lowest_corner_site;
use crate::frozen::{FrozenStructures, TemplateId};
use crate::piece::{END_CITY_TEMPLATE_PREFIX, EndCityPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &[
    "end_city/base_floor",
    "end_city/base_roof",
    "end_city/second_floor_1",
    "end_city/second_floor_2",
    "end_city/second_roof",
    "end_city/third_floor_1",
    "end_city/third_floor_2",
    "end_city/third_roof",
    "end_city/tower_base",
    "end_city/tower_piece",
    "end_city/tower_top",
    "end_city/bridge_piece",
    "end_city/bridge_end",
    "end_city/bridge_steep_stairs",
    "end_city/bridge_gentle_stairs",
    "end_city/ship",
    "end_city/fat_tower_base",
    "end_city/fat_tower_middle",
    "end_city/fat_tower_top",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

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
    let mut last = add(
        &mut pieces,
        city.piece("base_floor", site.position, rotation, true),
    );
    last = add(
        &mut pieces,
        city.add_piece(
            &last,
            IVec3::new(-1, 0, -1),
            "second_floor_1",
            rotation,
            false,
        ),
    );
    last = add(
        &mut pieces,
        city.add_piece(
            &last,
            IVec3::new(-1, 4, -1),
            "third_floor_1",
            rotation,
            false,
        ),
    );
    last = add(
        &mut pieces,
        city.add_piece(&last, IVec3::new(-1, 8, -1), "third_roof", rotation, true),
    );
    city.recursive_children(Section::Tower, 1, &last, IVec3::ZERO, &mut pieces);
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
    fn template(&self, name: &str) -> TemplateId {
        let location = ResourceLocation::minecraft(&format!("{END_CITY_TEMPLATE_PREFIX}{name}"));
        self.frozen.template_ids[&location]
    }

    fn piece(
        &self,
        name: &str,
        position: IVec3,
        rotation: Rotation,
        overwrite: bool,
    ) -> EndCityPiece {
        let template = self.template(name);
        let size = self.frozen.manifests[template.0 as usize].size;
        EndCityPiece {
            template,
            position,
            rotation,
            overwrite,
            bounds: bounding_box(size, position, rotation, Mirror::None, IVec3::ZERO),
            gen_depth: 0,
        }
    }

    /// The child at `offset` in the parent's frame, both at pivot zero.
    fn add_piece(
        &self,
        parent: &EndCityPiece,
        offset: IVec3,
        name: &str,
        rotation: Rotation,
        overwrite: bool,
    ) -> EndCityPiece {
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
    ) -> bool {
        if depth > MAX_GEN_DEPTH {
            return false;
        }
        let mut children = Vec::new();
        if !self.generate(section, depth, parent, offset, &mut children) {
            return false;
        }
        let tag = self.rng.next_i32();
        let collision = children.iter().any(|child| {
            pieces
                .iter()
                .find(|piece| piece.bounds.intersects(child.bounds))
                .is_some_and(|hit| hit.gen_depth != parent.gen_depth)
        });
        if collision {
            return false;
        }
        for child in &mut children {
            child.gen_depth = tag;
        }
        pieces.extend(children);
        true
    }

    fn generate(
        &mut self,
        section: Section,
        depth: i32,
        parent: &EndCityPiece,
        offset: IVec3,
        pieces: &mut Vec<EndCityPiece>,
    ) -> bool {
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
    ) -> bool {
        if depth > MAX_GEN_DEPTH {
            return false;
        }
        let rotation = parent.rotation;
        let mut last = add(
            pieces,
            self.add_piece(parent, offset, "base_floor", rotation, true),
        );
        match self.rng.next_i32_bound(3) {
            0 => {
                add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 4, -1), "base_roof", rotation, true),
                );
            }
            1 => {
                last = add(
                    pieces,
                    self.add_piece(
                        &last,
                        IVec3::new(-1, 0, -1),
                        "second_floor_2",
                        rotation,
                        false,
                    ),
                );
                last = add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 8, -1), "second_roof", rotation, false),
                );
                self.recursive_children(Section::Tower, depth + 1, &last, IVec3::ZERO, pieces);
            }
            _ => {
                last = add(
                    pieces,
                    self.add_piece(
                        &last,
                        IVec3::new(-1, 0, -1),
                        "second_floor_2",
                        rotation,
                        false,
                    ),
                );
                last = add(
                    pieces,
                    self.add_piece(
                        &last,
                        IVec3::new(-1, 4, -1),
                        "third_floor_2",
                        rotation,
                        false,
                    ),
                );
                last = add(
                    pieces,
                    self.add_piece(&last, IVec3::new(-1, 8, -1), "third_roof", rotation, true),
                );
                self.recursive_children(Section::Tower, depth + 1, &last, IVec3::ZERO, pieces);
            }
        }
        true
    }

    fn tower(&mut self, depth: i32, parent: &EndCityPiece, pieces: &mut Vec<EndCityPiece>) -> bool {
        let rotation = parent.rotation;
        let base = IVec3::new(
            3 + self.rng.next_i32_bound(2),
            -3,
            3 + self.rng.next_i32_bound(2),
        );
        let mut last = add(
            pieces,
            self.add_piece(parent, base, "tower_base", rotation, true),
        );
        last = add(
            pieces,
            self.add_piece(&last, IVec3::new(0, 7, 0), "tower_piece", rotation, true),
        );
        let mut bridge_piece = (self.rng.next_i32_bound(3) == 0).then(|| last.clone());
        let tower_height = 1 + self.rng.next_i32_bound(3);
        for i in 0..tower_height {
            last = add(
                pieces,
                self.add_piece(&last, IVec3::new(0, 4, 0), "tower_piece", rotation, true),
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
                                "bridge_end",
                                rotation.rotated(turn),
                                true,
                            ),
                        );
                        self.recursive_children(
                            Section::TowerBridge,
                            depth + 1,
                            &bridge_start,
                            IVec3::ZERO,
                            pieces,
                        );
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
            self.add_piece(&last, IVec3::new(-1, 4, -1), "tower_top", rotation, true),
        );
        true
    }

    fn tower_bridge(
        &mut self,
        depth: i32,
        parent: &EndCityPiece,
        pieces: &mut Vec<EndCityPiece>,
    ) -> bool {
        let rotation = parent.rotation;
        let bridge_length = self.rng.next_i32_bound(4) + 1;
        let mut first =
            self.add_piece(parent, IVec3::new(0, 0, -4), "bridge_piece", rotation, true);
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
                        "bridge_piece",
                        rotation,
                        true,
                    ),
                );
                next_y = 0;
            } else {
                let (name, offset) = if self.rng.next_bool() {
                    ("bridge_steep_stairs", IVec3::new(0, next_y, -4))
                } else {
                    ("bridge_gentle_stairs", IVec3::new(0, next_y, -8))
                };
                last = add(pieces, self.add_piece(&last, offset, name, rotation, true));
                next_y = 4;
            }
        }
        if !self.ship_created && self.rng.next_i32_bound(10 - depth) == 0 {
            let offset = IVec3::new(
                -8 + self.rng.next_i32_bound(8),
                next_y,
                -70 + self.rng.next_i32_bound(10),
            );
            add(
                pieces,
                self.add_piece(&last, offset, "ship", rotation, true),
            );
            self.ship_created = true;
        } else if !self.recursive_children(
            Section::HouseTower,
            depth + 1,
            &last,
            IVec3::new(-3, next_y + 1, -11),
            pieces,
        ) {
            return false;
        }
        let mut end = self.add_piece(
            &last,
            IVec3::new(4, next_y, 0),
            "bridge_end",
            rotation.rotated(Rotation::Clockwise180),
            true,
        );
        end.gen_depth = -1;
        add(pieces, end);
        true
    }

    fn fat_tower(
        &mut self,
        depth: i32,
        parent: &EndCityPiece,
        pieces: &mut Vec<EndCityPiece>,
    ) -> bool {
        let rotation = parent.rotation;
        let mut last = add(
            pieces,
            self.add_piece(
                parent,
                IVec3::new(-3, 4, -3),
                "fat_tower_base",
                rotation,
                true,
            ),
        );
        last = add(
            pieces,
            self.add_piece(
                &last,
                IVec3::new(0, 4, 0),
                "fat_tower_middle",
                rotation,
                true,
            ),
        );
        for _ in 0..2 {
            if self.rng.next_i32_bound(3) == 0 {
                break;
            }
            last = add(
                pieces,
                self.add_piece(
                    &last,
                    IVec3::new(0, 8, 0),
                    "fat_tower_middle",
                    rotation,
                    true,
                ),
            );
            for (turn, offset) in FAT_TOWER_BRIDGES {
                if self.rng.next_bool() {
                    let bridge_start = add(
                        pieces,
                        self.add_piece(&last, offset, "bridge_end", rotation.rotated(turn), true),
                    );
                    self.recursive_children(
                        Section::TowerBridge,
                        depth + 1,
                        &bridge_start,
                        IVec3::ZERO,
                        pieces,
                    );
                }
            }
        }
        add(
            pieces,
            self.add_piece(
                &last,
                IVec3::new(-2, 8, -2),
                "fat_tower_top",
                rotation,
                true,
            ),
        );
        true
    }
}
