use core::fmt;

use mcrs_minecraft_keys::creative_mode_tab;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;

use crate::ListKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Group {
    PlaceablePaintings,
    GoatHorns,
    TippedArrows,
    Potions,
    SplashPotions,
    LingeringPotions,
    MaxLevelEnchantedBooks,
    EnchantedBooks,
    UnplaceablePaintings,
}

const ALL: [Group; 9] = [
    Group::PlaceablePaintings,
    Group::GoatHorns,
    Group::TippedArrows,
    Group::Potions,
    Group::SplashPotions,
    Group::LingeringPotions,
    Group::MaxLevelEnchantedBooks,
    Group::EnchantedBooks,
    Group::UnplaceablePaintings,
];

const POTION_CONTENTS: ResourceLocation<'static> = rl!("minecraft:potion_contents");

impl Group {
    pub(crate) fn of_list(
        tab: ResourceLocation<'_>,
        list: ListKind,
    ) -> impl Iterator<Item = Self> + use<'_> {
        ALL.into_iter()
            .filter(move |group| group.tab() == tab && group.in_list(list))
    }

    #[must_use]
    pub fn tab(self) -> ResourceLocation<'static> {
        match self {
            Self::PlaceablePaintings => creative_mode_tab::FUNCTIONAL_BLOCKS,
            Self::GoatHorns => creative_mode_tab::TOOLS_AND_UTILITIES,
            Self::TippedArrows => creative_mode_tab::COMBAT,
            Self::Potions | Self::SplashPotions | Self::LingeringPotions => {
                creative_mode_tab::FOOD_AND_DRINKS
            }
            Self::MaxLevelEnchantedBooks | Self::EnchantedBooks => creative_mode_tab::INGREDIENTS,
            Self::UnplaceablePaintings => creative_mode_tab::OP_BLOCKS,
        }
    }

    #[must_use]
    pub fn in_list(self, list: ListKind) -> bool {
        match self {
            Self::MaxLevelEnchantedBooks => list == ListKind::TabList,
            Self::EnchantedBooks => list == ListKind::SearchList,
            _ => true,
        }
    }

    #[must_use]
    pub fn item(self) -> ResourceLocation<'static> {
        match self {
            Self::PlaceablePaintings | Self::UnplaceablePaintings => rl!("minecraft:painting"),
            Self::GoatHorns => rl!("minecraft:goat_horn"),
            Self::TippedArrows => rl!("minecraft:tipped_arrow"),
            Self::Potions => rl!("minecraft:potion"),
            Self::SplashPotions => rl!("minecraft:splash_potion"),
            Self::LingeringPotions => rl!("minecraft:lingering_potion"),
            Self::MaxLevelEnchantedBooks | Self::EnchantedBooks => rl!("minecraft:enchanted_book"),
        }
    }

    #[must_use]
    pub fn component(self) -> ResourceLocation<'static> {
        match self {
            Self::PlaceablePaintings | Self::UnplaceablePaintings => {
                rl!("minecraft:painting/variant")
            }
            Self::GoatHorns => rl!("minecraft:instrument"),
            Self::TippedArrows | Self::Potions | Self::SplashPotions | Self::LingeringPotions => {
                POTION_CONTENTS
            }
            Self::MaxLevelEnchantedBooks | Self::EnchantedBooks => {
                rl!("minecraft:stored_enchantments")
            }
        }
    }

    #[must_use]
    pub(crate) fn holds(self, saved: &NbtCompound) -> bool {
        let Some(NbtTag::String(id)) = saved.get("id") else {
            return false;
        };
        if ResourceLocation::read(id).ok() != Some(self.item()) {
            return false;
        }
        let Some(NbtTag::Compound(components)) = saved.get("components") else {
            return false;
        };
        components
            .child_tags
            .iter()
            .any(|(key, _)| ResourceLocation::read(key).ok() == Some(self.component()))
    }
}

impl fmt::Display for Group {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} with {}", self.item(), self.component())
    }
}
