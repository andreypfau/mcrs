use mcrs_minecraft_entity_animation::EntityQuery;
use mcrs_minecraft_pack_file::{ClientEntityFile, DiagnosticKind, FieldCompiler, PackDiagnostic};
use mcrs_molang::{
    Arguments, Expression, LiteralKey, Milestone, QueryDeclaration, QueryRegistry, QueryScope,
    QuerySlot, QueryValues, Rate, ResourceKind, ResourceSet, ScalarVm, Subject, Value, ValueType,
    VariableLayout, Variables, string_hash,
};

use crate::layer_choice::{LayerChoice, MAX_PART_PATTERNS, TextureChoice};
use crate::render_controller_file::RenderControllerLibrary;
use crate::render_query::RenderQuery;

/// One element of the render controller list of a client entity, compiled.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledLayer {
    pub controller: String,
    pub condition: Expression,
    pub geometry: Expression,
    pub texture: Expression,
    /// At most [`MAX_PART_PATTERNS`] bone patterns in file order, each with its value.
    pub part_visibility: Vec<(String, Expression)>,
}

/// The render controllers of one client entity, compiled against its resource maps.
#[derive(Clone, Debug)]
pub struct RenderControllers {
    pub layers: Vec<CompiledLayer>,
    /// The query that each query slot reads.
    slots: Vec<Option<RenderQuery>>,
    layout: VariableLayout,
}

impl RenderControllers {
    /// Compiles the render controller list of `entity`. An element with a load error goes to
    /// `diagnostics` and draws nothing.
    pub fn compile(
        entity: &ClientEntityFile,
        library: &RenderControllerLibrary,
        diagnostics: &mut Vec<PackDiagnostic>,
    ) -> Self {
        let mut resources = ResourceSet::default();
        for (id, (short, _)) in (0u32..).zip(&entity.geometries) {
            resources.insert(ResourceKind::Geometry, short, id);
        }
        for (id, (short, _)) in (0u32..).zip(&entity.textures) {
            resources.insert(ResourceKind::Texture, short, id);
        }
        for (id, (short, _)) in (0u32..).zip(&entity.materials) {
            resources.insert(ResourceKind::Material, short, id);
        }
        let mut compiler = FieldCompiler::new(registry());
        let mut layers = Vec::new();
        for entry in &entity.render_controllers {
            let context = format!("{}/{}", entity.identifier, entry.identifier);
            let Some(source) = library.get(&entry.identifier) else {
                compiler.report(
                    DiagnosticKind::ContentError,
                    &context,
                    "no render controller has this identifier",
                );
                continue;
            };
            if source.has_arrays {
                compiler.report(
                    DiagnosticKind::Unsupported,
                    &context,
                    "`arrays` is not compiled yet",
                );
            }
            let condition = match &entry.condition {
                Some(json) => compiler.try_compile(json, &format!("{context}/condition"), None),
                None => Some(Expression::Constant(Value::Number(1.0))),
            };
            let Some(geometry) = source.geometry.as_ref().and_then(|json| {
                compiler.try_compile(json, &format!("{context}/geometry"), Some(&resources))
            }) else {
                compiler.report(
                    DiagnosticKind::LoadError,
                    &context,
                    "the layer has no geometry",
                );
                continue;
            };
            let texture = source.textures.first().and_then(|json| {
                compiler.try_compile(json, &format!("{context}/textures"), Some(&resources))
            });
            let (Some(condition), Some(texture)) = (condition, texture) else {
                continue;
            };
            if source.part_visibility.len() > MAX_PART_PATTERNS {
                compiler.report(
                    DiagnosticKind::Unsupported,
                    &context,
                    format!("only the first {MAX_PART_PATTERNS} `part_visibility` patterns apply"),
                );
            }
            let part_visibility = source
                .part_visibility
                .iter()
                .take(MAX_PART_PATTERNS)
                .filter_map(|(pattern, json)| {
                    let context = format!("{context}/part_visibility/{pattern}");
                    Some((pattern.clone(), compiler.try_compile(json, &context, None)?))
                })
                .collect();
            layers.push(CompiledLayer {
                controller: entry.identifier.clone(),
                condition,
                geometry,
                texture,
                part_visibility,
            });
        }
        let slots = compiler
            .registry
            .slots()
            .map(|(_, key)| {
                let declaration = compiler.registry.declaration(key.declaration)?;
                if declaration.name.as_ref() == EQUIPMENT_TEXTURE {
                    let [LiteralKey::String(slot), LiteralKey::String(layer)] =
                        key.arguments.as_slice()
                    else {
                        return None;
                    };
                    return Some(RenderQuery::EquipmentTexture {
                        slot: slot.to_string(),
                        layer: layer.to_string(),
                    });
                }
                EntityQuery::from_declaration(&declaration.name, &key.arguments)
                    .map(RenderQuery::Entity)
            })
            .collect();
        diagnostics.append(&mut compiler.diagnostics);
        Self {
            layers,
            slots,
            layout: compiler.layout,
        }
    }

    /// The queries that the expressions read; a host gives a value for each one.
    pub fn queries(&self) -> impl Iterator<Item = &RenderQuery> {
        self.slots.iter().flatten()
    }

    /// The layers that the client entity draws with the query values of `source`. A layer
    /// whose geometry or texture gives no resource draws nothing.
    pub fn evaluate(
        &self,
        vm: &mut ScalarVm,
        mut source: impl FnMut(&RenderQuery) -> Option<Value>,
    ) -> Vec<LayerChoice> {
        let mut queries = QueryValues::new(self.slots.len());
        for (slot, query) in (0u16..).zip(&self.slots) {
            let value = query.as_ref().and_then(&mut source);
            queries.set(QuerySlot(slot), value.map(Value::to_raw));
        }
        let mut variables = Variables::new(&self.layout);
        let mut run = |expression: &Expression| {
            expression.evaluate(
                vm,
                &mut Subject {
                    variables: &mut variables,
                    queries: &queries,
                    this: 0.0,
                },
            )
        };
        let mut choices = Vec::new();
        for (entry, layer) in self.layers.iter().enumerate() {
            if run(&layer.condition).as_number() == 0.0 {
                continue;
            }
            let Value::Resource(geometry) = run(&layer.geometry) else {
                continue;
            };
            if geometry.kind != ResourceKind::Geometry {
                continue;
            }
            let texture = match run(&layer.texture) {
                Value::Resource(texture) if texture.kind == ResourceKind::Texture => {
                    TextureChoice::Named(texture.id as usize)
                }
                Value::String(hash) if hash != string_hash("") => TextureChoice::Identifier(hash),
                _ => continue,
            };
            let mut shown_parts = 0u64;
            for (bit, (_, expression)) in layer.part_visibility.iter().enumerate() {
                if run(expression).as_number() != 0.0 {
                    shown_parts |= 1 << bit;
                }
            }
            choices.push(LayerChoice {
                entry,
                geometry: geometry.id as usize,
                texture,
                shown_parts,
            });
        }
        choices
    }
}

const EQUIPMENT_TEXTURE: &str = "equipment_texture";

fn registry() -> QueryRegistry {
    let mut registry = EntityQuery::registry();
    registry.declare(QueryDeclaration {
        name: EQUIPMENT_TEXTURE.into(),
        arguments: Arguments::Literals(2),
        result: ValueType::String,
        rate: Rate::EntityTick,
        scope: QueryScope::Entity,
        milestone: Milestone::One,
    });
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIG_ENTITY: &str = include_str!("../../../assets/mcrs/entity/entity/pig.entity.json");
    const PIG_CONTROLLERS: &str =
        include_str!("../../../assets/mcrs/entity/render_controllers/pig.render_controllers.json");

    fn pig() -> (ClientEntityFile, RenderControllers) {
        let entity = ClientEntityFile::parse(PIG_ENTITY).unwrap();
        let mut library = RenderControllerLibrary::new();
        library.add_file(PIG_CONTROLLERS).unwrap();
        let mut diagnostics = Vec::new();
        let controllers = RenderControllers::compile(&entity, &library, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        (entity, controllers)
    }

    /// MPIG-VAR-002 and MPIG-LAYER-001 through the render controllers of the entity pack.
    #[test]
    fn the_pig_selects_its_geometry_by_age_and_model_and_draws_the_saddle_on_an_adult() {
        let (entity, controllers) = pig();
        let mut vm = ScalarVm::new();
        let cases = [
            (false, "normal", true, "geometry.pig", 2),
            (false, "cold", false, "geometry.pig.cold", 1),
            (true, "cold", true, "geometry.pig.baby", 1),
        ];
        for (baby, model, saddled, geometry, layers) in cases {
            let saddle = if saddled {
                "minecraft:entity/equipment/pig_saddle/saddle"
            } else {
                ""
            };
            let choices = controllers.evaluate(&mut vm, |query| {
                Some(match query {
                    RenderQuery::Entity(EntityQuery::IsBaby) => {
                        Value::Number(if baby { 1.0 } else { 0.0 })
                    }
                    RenderQuery::Entity(EntityQuery::VariantModel) => {
                        Value::String(string_hash(model))
                    }
                    RenderQuery::Entity(EntityQuery::VariantAsset) => {
                        Value::String(string_hash("minecraft:entity/pig/pig_cold"))
                    }
                    RenderQuery::EquipmentTexture { .. } => Value::String(string_hash(saddle)),
                    RenderQuery::Entity(_) => Value::Number(0.0),
                })
            });
            assert_eq!(choices.len(), layers, "{baby} {model} {saddled}");
            let body = &choices[0];
            assert_eq!(entity.geometries[body.geometry].1, geometry);
            assert_eq!(
                body.texture,
                TextureChoice::Identifier(string_hash("minecraft:entity/pig/pig_cold"))
            );
        }
    }

    #[test]
    fn a_versioned_expression_object_compiles_like_its_expression() {
        let entity = ClientEntityFile::parse(
            r#"{"minecraft:client_entity": {"description": {"identifier": "test:mob",
                "geometry": {"default": "geometry.mob"}, "textures": {"default": "mob"},
                "render_controllers": [{"controller.render.mob": {"expression": "q.is_baby"}}]}}}"#,
        )
        .unwrap();
        let mut library = RenderControllerLibrary::new();
        library
            .add_file(
                r#"{"render_controllers": {"controller.render.mob": {
                    "geometry": {"expression": "Geometry.default", "version": 1},
                    "textures": ["Texture.default"]}}}"#,
            )
            .unwrap();
        let mut diagnostics = Vec::new();
        let controllers = RenderControllers::compile(&entity, &library, &mut diagnostics);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.kind == DiagnosticKind::LoadWarning),
            "{diagnostics:#?}"
        );
        let mut vm = ScalarVm::new();
        for (baby, layers) in [(true, 1), (false, 0)] {
            let choices = controllers.evaluate(&mut vm, |query| {
                Some(Value::Number(match query {
                    RenderQuery::Entity(EntityQuery::IsBaby) if baby => 1.0,
                    _ => 0.0,
                }))
            });
            assert_eq!(choices.len(), layers, "baby {baby}");
        }
    }
}
