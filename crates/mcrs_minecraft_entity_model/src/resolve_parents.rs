use crate::diagnostic::Diagnostic;
use crate::geometry::Geometry;
use crate::geometry_error::GeometryError;

/// Merges each geometry of the legacy shape with its parent geometry. Call it after all
/// geometry files of all pack roots are read. A geometry whose parent is missing, or whose
/// chain of parents has a cycle, is rejected with a diagnostic.
#[must_use]
pub fn resolve_parents(
    geometries: Vec<Geometry>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Geometry> {
    let mut resolved = Vec::with_capacity(geometries.len());
    for geometry in &geometries {
        match resolve(geometry, &geometries, 0) {
            Ok(merged) => resolved.push(merged),
            Err(error) => diagnostics.push(Diagnostic::RejectedGeometry {
                identifier: Some(geometry.identifier.clone()),
                error,
            }),
        }
    }
    resolved
}

fn resolve(geometry: &Geometry, all: &[Geometry], depth: usize) -> Result<Geometry, GeometryError> {
    let Some(parent_name) = &geometry.parent else {
        return Ok(geometry.clone());
    };
    if depth >= all.len() {
        return Err(GeometryError::ParentGeometryCycle);
    }
    let parent = all
        .iter()
        .find(|candidate| candidate.identifier.eq_ignore_ascii_case(parent_name))
        .ok_or_else(|| GeometryError::MissingParentGeometry {
            parent: parent_name.clone(),
        })?;
    let mut merged = resolve(parent, all, depth + 1)?;
    for bone in &geometry.bones {
        match merged
            .bones
            .iter_mut()
            .find(|held| held.name.eq_ignore_ascii_case(&bone.name))
        {
            Some(held) => *held = bone.clone(),
            None => merged.bones.push(bone.clone()),
        }
    }
    merged.identifier.clone_from(&geometry.identifier);
    merged.parent = None;
    merged.format = geometry.format;
    merged.texture_width = geometry.texture_width.or(merged.texture_width);
    merged.texture_height = geometry.texture_height.or(merged.texture_height);
    merged.visible_bounds_width = geometry
        .visible_bounds_width
        .or(merged.visible_bounds_width);
    merged.visible_bounds_height = geometry
        .visible_bounds_height
        .or(merged.visible_bounds_height);
    merged.visible_bounds_offset = geometry
        .visible_bounds_offset
        .or(merged.visible_bounds_offset);
    Ok(merged)
}
