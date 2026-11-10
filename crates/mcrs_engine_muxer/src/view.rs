use crate::tile_radius::TileRadius;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

/// What a viewer subscribes to: the tiles within `radius` of `center`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct View {
    pub center: TileKey,
    pub radius: TileRadius,
    /// The own entity of the viewer, which the viewer never receives.
    pub subject: Option<EntityId>,
}

impl View {
    #[must_use]
    pub fn contains(&self, tile: TileKey) -> bool {
        within(self.center.x, tile.x, self.radius.x)
            && within(self.center.y, tile.y, self.radius.y)
            && within(self.center.z, tile.z, self.radius.z)
    }

    /// The tiles of the view, in the order of [`TileKey`].
    pub fn tiles(&self) -> impl Iterator<Item = TileKey> + use<> {
        let center = self.center;
        let radius = self.radius;
        span(center.x, radius.x).flat_map(move |x| {
            span(center.y, radius.y)
                .flat_map(move |y| span(center.z, radius.z).map(move |z| TileKey::new(x, y, z)))
        })
    }
}

fn within(center: i32, value: i32, radius: u16) -> bool {
    (i64::from(value) - i64::from(center)).abs() <= i64::from(radius)
}

fn span(center: i32, radius: u16) -> impl Iterator<Item = i32> + use<> {
    let radius = i32::from(radius);
    center.saturating_sub(radius)..=center.saturating_add(radius)
}
