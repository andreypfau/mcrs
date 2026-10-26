use crate::command::{
    AppCommands, CommandContext, CommandFailure, CommandFeedback, CommandRegistrar, CommandResult,
    Invocation,
};
use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::IVec3;
use mcrs_command::{Arg, BuildError};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck, ResourceOrTag, ResourceOrTagArgument};
use mcrs_minecraft_core::{ResourceLocation, StaticResourceLocation, rl};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_structure::frozen::StructureId;

const STRUCTURE: StaticResourceLocation = rl!("minecraft:worldgen/structure");
const BIOME: StaticResourceLocation = rl!("minecraft:worldgen/biome");
const POINT_OF_INTEREST_TYPE: StaticResourceLocation = rl!("minecraft:point_of_interest_type");

#[derive(Resource, Clone, Copy)]
struct LocateCommand {
    structure: Arg<ResourceOrTagArgument>,
    biome: Arg<ResourceOrTagArgument>,
    poi: Arg<ResourceOrTagArgument>,
}

/// CMD-LOC-001.
fn build(r: &mut CommandRegistrar<'_>) -> Result<LocateCommand, BuildError> {
    let open = PermissionCheck::AlwaysPass;
    let locate = r.literal(
        CommandRegistrar::ROOT,
        "locate",
        PermissionCheck::Require(CommandLevel::Gamemasters),
    )?;
    let structure = r.literal(locate, "structure", open)?;
    let structure = r.argument(
        structure,
        "structure",
        ResourceOrTagArgument {
            registry: STRUCTURE,
            must_exist: false,
        },
        open,
    )?;
    r.executes(structure.node())?;
    let biome = r.literal(locate, "biome", open)?;
    let biome = r.argument(
        biome,
        "biome",
        ResourceOrTagArgument {
            registry: BIOME,
            must_exist: true,
        },
        open,
    )?;
    r.executes(biome.node())?;
    let poi = r.literal(locate, "poi", open)?;
    let poi = r.argument(
        poi,
        "poi",
        ResourceOrTagArgument {
            registry: POINT_OF_INTEREST_TYPE,
            must_exist: true,
        },
        open,
    )?;
    r.executes(poi.node())?;
    Ok(LocateCommand {
        structure,
        biome,
        poi,
    })
}

pub(super) fn register(app: &mut App) {
    app.add_command(build, run);
}

/// CMD-LOC-005: the message, and the distance that is the result of the command.
fn found(
    key: &'static str,
    value: ResourceOrTag<'_>,
    entry: &str,
    at: IVec3,
    origin: IVec3,
    with_y: bool,
) -> (Text, i32) {
    let name = match value {
        ResourceOrTag::Entry(_) => value.to_string(),
        ResourceOrTag::Tag(_) => format!("{value} ({entry})"),
    };
    let d = (at - origin).as_i64vec3();
    let squared = d.x * d.x + d.z * d.z + if with_y { d.y * d.y } else { 0 };
    let distance = i32::try_from(squared.isqrt()).unwrap_or(i32::MAX);
    let y = if with_y {
        at.y.to_string()
    } else {
        "~".to_owned()
    };
    let coordinates = Text::translate(
        "chat.square_brackets",
        vec![Text::translate(
            "chat.coordinates",
            vec![
                at.x.to_string().into_text(),
                y.clone().into_text(),
                at.z.to_string().into_text(),
            ],
        )],
    )
    .color(Color::GREEN)
    .on_click_suggest_command(format!("/tp @s {} {y} {}", at.x, at.z))
    .on_hover_show_text(Text::translate("chat.coordinates.tooltip", vec![]));
    let message = Text::translate(
        key,
        vec![
            name.into_text(),
            coordinates,
            distance.to_string().into_text(),
        ],
    );
    (message, distance)
}

fn not_found(key: &'static str, value: ResourceOrTag<'_>) -> CommandFailure {
    CommandFailure::translate(key, vec![value.to_string().into_text()])
}

/// The registry ids of an entry or of the members of a tag.
fn ids(
    registries: &CurrentRegistries,
    registry: StaticResourceLocation,
    value: ResourceOrTag<'_>,
) -> Option<Vec<(u16, ResourceLocation)>> {
    let names = registries.names_of(&registry.into())?;
    let named = |index: u16| {
        names
            .get(usize::from(index))
            .map(|name| (index, name.clone()))
    };
    match value {
        ResourceOrTag::Entry(id) => {
            let index = names.iter().position(|name| id.eq_str(name.as_str()))?;
            named(u16::try_from(index).ok()?).map(|entry| vec![entry])
        }
        ResourceOrTag::Tag(tag) => {
            let (_, members) = registries
                .tags_of(&registry.into())?
                .find(|(name, _)| tag.eq_str(name.as_str()))?;
            Some(members.iter().filter_map(|&index| named(index)).collect())
        }
    }
}

fn structure(
    fill: Option<&FillContext>,
    registries: Option<&CurrentRegistries>,
    value: ResourceOrTag<'_>,
    origin: IVec3,
) -> Result<(Text, i32), CommandFailure> {
    let invalid = || not_found("commands.locate.structure.invalid", value);
    let index = fill.and_then(|fill| fill.structures.as_deref());
    let wanted: Vec<ResourceLocation> = match value {
        ResourceOrTag::Entry(id) => {
            vec![ResourceLocation::new(id.namespace, id.path).map_err(|_| invalid())?]
        }
        ResourceOrTag::Tag(_) => registries
            .and_then(|registries| ids(registries, STRUCTURE, value))
            .ok_or_else(invalid)?
            .into_iter()
            .map(|(_, name)| name)
            .collect(),
    };
    let Some(index) = index else {
        return Err(not_found("commands.locate.structure.not_found", value));
    };
    let frozen = &index.tables().frozen;
    let structures: Vec<StructureId> = wanted
        .iter()
        .filter_map(|name| frozen.structure_ids.get(name).copied())
        .collect();
    if structures.is_empty() && matches!(value, ResourceOrTag::Entry(_)) {
        return Err(invalid());
    }
    let (at, structure) = index
        .locate(origin, &structures)
        .ok_or_else(|| not_found("commands.locate.structure.not_found", value))?;
    let entry = frozen
        .structures
        .get(usize::from(structure.0))
        .map_or_else(String::new, |structure| structure.id.to_string());
    Ok(found(
        "commands.locate.structure.success",
        value,
        &entry,
        at,
        origin,
        false,
    ))
}

const BIOME_RADIUS: i32 = 200;
const BIOME_STEP: i32 = 32;
const BIOME_Y_STEP: i32 = 64;

/// CMD-LOC-003: the nearest sample point with one of the biomes, searched ring by ring so
/// the search stops once no farther ring can be nearer.
// chisle: the search runs on the tick, as long as a few seconds for a biome that is absent; a
// task the tick polls lifts it.
fn nearest_biome(
    mut biome_at: impl FnMut(IVec3) -> Option<u16>,
    wanted: &[(u16, ResourceLocation)],
    origin: IVec3,
    min_y: i32,
    max_y: i32,
) -> Option<(IVec3, u16)> {
    let low = min_y + 1;
    let high = max_y + 1;
    let centre = origin.y.clamp(low, high);
    let first = centre - (centre - low) / BIOME_Y_STEP * BIOME_Y_STEP;
    let mut best: Option<(i64, IVec3, u16)> = None;
    for ring in 0..=BIOME_RADIUS {
        let nearest_possible = i64::from(ring * BIOME_STEP).pow(2);
        if best.is_some_and(|(distance, ..)| distance <= nearest_possible) {
            break;
        }
        for i in -ring..=ring {
            for j in -ring..=ring {
                if i.abs().max(j.abs()) != ring {
                    continue;
                }
                let x = origin.x + BIOME_STEP * i;
                let z = origin.z + BIOME_STEP * j;
                for y in (first..=high).step_by(BIOME_Y_STEP as usize) {
                    let at = IVec3::new(x, y, z);
                    let distance = (at - origin).as_i64vec3().length_squared();
                    if best.is_some_and(|(nearest, ..)| nearest <= distance) {
                        continue;
                    }
                    if let Some(biome) =
                        biome_at(at).filter(|biome| wanted.iter().any(|(id, _)| id == biome))
                    {
                        best = Some((distance, at, biome));
                    }
                }
            }
        }
    }
    best.map(|(_, at, biome)| (at, biome))
}

fn run(
    In(invocation): In<Invocation>,
    command: Res<LocateCommand>,
    context: CommandContext,
    fill: Option<Res<FillContext>>,
    registries: Option<Res<CurrentRegistries>>,
    dimension: Query<&DimensionTypeConfig, With<Dimension>>,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let lookup = context.lookup();
    let source = invocation.source(&lookup);
    let origin = invocation.source.position.floor().as_ivec3();
    let (message, distance) =
        if let Some(value) = invocation.get(&context.tree, command.structure, &source) {
            structure(fill.as_deref(), registries.as_deref(), value?, origin)?
        } else if let Some(value) = invocation.get(&context.tree, command.biome, &source) {
            let value = value?;
            let not_found = || not_found("commands.locate.biome.not_found", value);
            let wanted = registries
                .as_deref()
                .and_then(|registries| ids(registries, BIOME, value))
                .ok_or_else(not_found)?;
            let index = fill
                .as_deref()
                .and_then(|fill| fill.structures.as_deref())
                .ok_or_else(not_found)?;
            let config = dimension.iter().next().ok_or_else(not_found)?;
            let max_y = config.min_y + i32::try_from(config.height).unwrap_or(i32::MAX) - 1;
            let mut biomes = index.noise_biomes();
            let (at, biome) = nearest_biome(&mut biomes, &wanted, origin, config.min_y, max_y)
                .ok_or_else(not_found)?;
            let entry = wanted
                .iter()
                .find(|(id, _)| *id == biome)
                .map_or_else(String::new, |(_, name)| name.to_string());
            found(
                "commands.locate.biome.success",
                value,
                &entry,
                at,
                origin,
                true,
            )
        } else if let Some(value) = invocation.get(&context.tree, command.poi, &source) {
            // chisle: the server stores no points of interest yet, so the search finds none; a
            // point of interest index lifts it.
            return Err(not_found("commands.locate.poi.not_found", value?));
        } else {
            return Err(CommandFailure::translate("command.failed", vec![]));
        };
    feedback.success(&invocation, message, false);
    Ok(distance)
}
