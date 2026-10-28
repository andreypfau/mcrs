use crate::command::{
    Call, CommandContext, CommandFailure, CommandOutput, CommandRegistrar, CommandResult,
};
use bevy_ecs::prelude::*;
use bevy_math::IVec3;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_biome::keys::BIOME;
use mcrs_minecraft_command::{
    CommandLevel, Lookup, PermissionCheck, ResourceOrTag, ResourceOrTagArgument,
};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_entity::keys::STRUCTURE;
use mcrs_minecraft_keys::POINT_OF_INTEREST_TYPE;
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_protocol::text::{Color, IntoText, Text};
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::structures::nearest_biome::nearest_biome;
use mcrs_minecraft_worldgen_structure::frozen::StructureId;

#[derive(Clone, Copy)]
pub(super) struct LocateCommand {
    structure: Arg<ResourceOrTagArgument>,
    biome: Arg<ResourceOrTagArgument>,
    poi: Arg<ResourceOrTagArgument>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Branch {
    Structure,
    Biome,
    Poi,
}

/// CMD-LOC-001.
pub(super) fn build(r: &mut CommandRegistrar<'_, Branch>) -> Result<LocateCommand, BuildError> {
    let locate = r
        .literal(NodeId::ROOT, "locate")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let structure = r.literal(locate, "structure").id()?;
    let structure = r
        .arg(
            structure,
            "structure",
            ResourceOrTagArgument {
                registry: STRUCTURE.location(),
                must_exist: false,
            },
        )
        .runs(Branch::Structure)
        .id()?;
    let biome = r.literal(locate, "biome").id()?;
    let biome = r
        .arg(
            biome,
            "biome",
            ResourceOrTagArgument {
                registry: BIOME.location(),
                must_exist: true,
            },
        )
        .runs(Branch::Biome)
        .id()?;
    let poi = r.literal(locate, "poi").id()?;
    let poi = r
        .arg(
            poi,
            "poi",
            ResourceOrTagArgument {
                registry: POINT_OF_INTEREST_TYPE,
                must_exist: true,
            },
        )
        .runs(Branch::Poi)
        .id()?;
    Ok(LocateCommand {
        structure,
        biome,
        poi,
    })
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

fn structure(
    fill: Option<&FillContext>,
    context: &CommandContext,
    value: ResourceOrTag<'_>,
    origin: IVec3,
) -> Result<(Text, i32), CommandFailure> {
    let invalid = || not_found("commands.locate.structure.invalid", value);
    let index = fill.and_then(|fill| fill.structures.as_deref());
    let frozen = index.map(|index| &index.tables().frozen);
    let id_of = |name: &ResourceLocation| frozen?.structure_ids.get(name).copied();
    let structures: Vec<StructureId> = match value {
        ResourceOrTag::Entry(id) => id_of(&id.to_location().ok_or_else(invalid)?)
            .into_iter()
            .collect(),
        ResourceOrTag::Tag(_) => context
            .registry_lookup()
            .ids(STRUCTURE.location(), value)
            .ok_or_else(invalid)?
            .iter()
            .filter_map(|&id| id_of(context.registry_lookup().name(STRUCTURE.location(), id)?))
            .collect(),
    };
    let (Some(index), Some(frozen)) = (index, frozen) else {
        return Err(not_found("commands.locate.structure.not_found", value));
    };
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

pub(super) fn run(
    In(call): In<Call<Branch, LocateCommand>>,
    context: CommandContext,
    fill: Option<Res<FillContext>>,
    dimension: Query<&DimensionTypeConfig, With<Dimension>>,
    mut feedback: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let origin = call.source.position.floor().as_ivec3();
    let (message, distance) = match call.branch {
        Branch::Structure => {
            let value = context.arg(&call, command.structure)?;
            structure(fill.as_deref(), &context, value, origin)?
        }
        Branch::Biome => {
            let value = context.arg(&call, command.biome)?;
            let not_found = || not_found("commands.locate.biome.not_found", value);
            let wanted = context
                .registry_lookup()
                .ids(BIOME.location(), value)
                .ok_or_else(not_found)?;
            let index = fill
                .as_deref()
                .and_then(|fill| fill.structures.as_deref())
                .ok_or_else(not_found)?;
            let config = dimension.iter().next().ok_or_else(not_found)?;
            let height = i32::try_from(config.height).unwrap_or(i32::MAX);
            let (at, biome) =
                nearest_biome(index.noise_biomes(), &wanted, origin, config.min_y, height)
                    .ok_or_else(not_found)?;
            let entry = context
                .registry_lookup()
                .name(BIOME.location(), biome)
                .map_or_else(String::new, ToString::to_string);
            found(
                "commands.locate.biome.success",
                value,
                &entry,
                at,
                origin,
                true,
            )
        }
        // chisle: the server stores no points of interest yet, so the search finds none; a
        // point of interest index lifts it.
        Branch::Poi => {
            let value = context.arg(&call, command.poi)?;
            return Err(not_found("commands.locate.poi.not_found", value));
        }
    };
    feedback.success(&call, message, false);
    Ok(distance)
}
