use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use bevy_math::IVec3;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_biome::keys::BIOME;
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::{ResourceOrTag, ResourceOrTagArgument};
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback,
};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_entity::keys::STRUCTURE;
use mcrs_minecraft_keys::POINT_OF_INTEREST_TYPE;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{Color, IntoText, Text};
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::structures::nearest_biome;
use mcrs_minecraft_worldgen_structure::frozen::StructureId;

#[derive(Clone, Copy)]
pub(crate) struct LocateCommand {
    structure: ArgumentHandle<ResourceOrTagArgument>,
    biome: ArgumentHandle<ResourceOrTagArgument>,
    poi: ArgumentHandle<ResourceOrTagArgument>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Structure,
    Biome,
    Poi,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<LocateCommand, BuildError> {
    let locate = registrar
        .literal(NodeId::ROOT, "locate")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let structure = registrar.literal(locate, "structure").id()?;
    let structure = registrar
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
    let biome = registrar.literal(locate, "biome").id()?;
    let biome = registrar
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
    let poi = registrar.literal(locate, "poi").id()?;
    let poi = registrar
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

/// Which axes the distance to a found position counts, and so whether its y is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Distance {
    Horizontal,
    Spatial,
}

/// The message, and the distance that is the result of the command.
fn found(
    key: &'static str,
    value: ResourceOrTag<'_>,
    entry: &str,
    at: IVec3,
    origin: IVec3,
    distance: Distance,
) -> (Text, i32) {
    let name = match value {
        ResourceOrTag::Entry(_) => value.to_string(),
        ResourceOrTag::Tag(_) => format!("{value} ({entry})"),
    };
    let offset = (at - origin).as_i64vec3();
    let vertical = match distance {
        Distance::Horizontal => 0,
        Distance::Spatial => offset.y * offset.y,
    };
    let squared = offset.x * offset.x + offset.z * offset.z + vertical;
    let length = i32::try_from(squared.isqrt()).unwrap_or(i32::MAX);
    let y = match distance {
        Distance::Horizontal => "~".to_owned(),
        Distance::Spatial => at.y.to_string(),
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
            length.to_string().into_text(),
        ],
    );
    (message, length)
}

fn not_found(key: &'static str, value: ResourceOrTag<'_>) -> CommandFailure {
    CommandFailure::translate(key, vec![value.to_string().into()])
}

fn structure(
    fill: Option<&FillContext>,
    context: &VanillaContext,
    value: ResourceOrTag<'_>,
    origin: IVec3,
) -> Result<(Text, i32), CommandFailure> {
    let invalid = || not_found("commands.locate.structure.invalid", value);
    let index = fill.and_then(|fill| fill.structures.as_deref());
    let frozen = index.map(|index| &index.tables().frozen);
    let id_of = |name: &ResourceLocationBuf| frozen?.structure_ids.get(name).copied();
    let lookup = context.lookup().registry_lookup();
    // chisle: `StructureIndex::locate` takes a slice and builds its own placement lists per search; an iterator parameter and caller-owned scratch lift both allocations.
    let structures: Vec<StructureId> = lookup
        .ids(STRUCTURE.location(), value)
        .ok_or_else(invalid)?
        .as_slice()
        .iter()
        .filter_map(|&id| id_of(lookup.name(STRUCTURE.location(), id)?))
        .collect();
    let (Some(index), Some(frozen)) = (index, frozen) else {
        return Err(not_found("commands.locate.structure.not_found", value));
    };
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
        Distance::Horizontal,
    ))
}

pub(crate) fn run(
    In(call): In<Call<Branch, LocateCommand>>,
    context: VanillaContext,
    fill: Option<Res<FillContext>>,
    dimension: Query<&DimensionTypeConfig, With<Dimension>>,
    mut feedback: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let origin = call.invocation.source.position.floor().as_ivec3();
    let (message, distance) = match call.branch {
        Branch::Structure => {
            let value = context.arg(&call.invocation, command.structure)?;
            structure(fill.as_deref(), &context, value, origin)?
        }
        Branch::Biome => {
            let value = context.arg(&call.invocation, command.biome)?;
            let not_found = || not_found("commands.locate.biome.not_found", value);
            let wanted = context
                .lookup()
                .registry_lookup()
                .ids(BIOME.location(), value)
                .ok_or_else(not_found)?;
            let index = fill
                .as_deref()
                .and_then(|fill| fill.structures.as_deref())
                .ok_or_else(not_found)?;
            let config = dimension.iter().next().ok_or_else(not_found)?;
            let height = i32::try_from(config.height).unwrap_or(i32::MAX);
            let (at, biome) = nearest_biome::nearest_biome(
                index.noise_biomes(),
                wanted.as_slice(),
                origin,
                config.min_y,
                height,
            )
            .ok_or_else(not_found)?;
            let entry = context
                .lookup()
                .registry_lookup()
                .name(BIOME.location(), biome)
                .map_or_else(String::new, ToString::to_string);
            found(
                "commands.locate.biome.success",
                value,
                &entry,
                at,
                origin,
                Distance::Spatial,
            )
        }
        // chisle: the server stores no points of interest yet, so the search finds none; a
        // point of interest index lifts it.
        Branch::Poi => {
            let value = context.arg(&call.invocation, command.poi)?;
            return Err(not_found("commands.locate.poi.not_found", value));
        }
    };
    feedback.success(&call.invocation, message, Feedback::Source);
    Ok(distance)
}
