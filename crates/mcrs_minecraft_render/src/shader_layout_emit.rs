use std::fmt::Write;
use std::mem;
use std::path::Path;

use mcrs_minecraft_mesh::DrawKind;
use naga::front::wgsl::parse_str;
use wesl::error::ResolveError;
use wesl::syntax::{ModulePath, PathOrigin};
use wesl::{CompileOptions, Resolver};

use crate::shader_layout::{
    ANIMATION_FRAME_STRUCT, BindingCond, BindingDef, CAMERA_STRUCT, CULL_BINDINGS, CULL_ORDERED,
    CULL_QUAD, DRAW_ARG_ROWS, DRAW_ARGS_WGSL_FIELDS, GBUFFER_BINDINGS, HIZ_BINDINGS, HIZ_DOWN,
    HIZ_FROM_DEPTH, PARAMS_STRUCT, PRESENT_BINDINGS, RECONSTRUCTION_STRUCT, SECTION_DESC_STRUCT,
    SHOW_BINDINGS, SKY_STRUCT, SKY_TEXTURES_BINDINGS, SKY_VIEW_BINDINGS, SPRITE_ENTRY_STRUCT,
    Stages, TERRAIN_DRAW_BINDINGS, TERRAIN_VIEW_BINDINGS, TEXT_FRAME_STRUCT, TRANSPARENCY_BINDINGS,
    WORLD_TEXT_TEXTURES_BINDINGS, WORLD_TEXT_VIEW_BINDINGS,
};
use crate::shared_types::{
    AnimationFrame, CameraUniform, DrawArgs, Params, Reconstruction, SectionDesc, SkyUniform,
    SpriteEntry, TextFrame,
};

fn generate_draw_arg_row(out: &mut String) {
    out.push_str("use mcrs_minecraft_mesh::DrawKind;\n");
    out.push_str("use wgpu::util::DrawIndirectArgs;\n\n");
    out.push_str("pub type DrawArgs = DrawIndirectArgs;\n\n");
    out.push_str("#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n");
    out.push_str("#[repr(u32)]\n");
    out.push_str("pub enum DrawArgRow {\n");
    for (index, (variant, _)) in DRAW_ARG_ROWS.iter().enumerate() {
        let _ = writeln!(out, "    {variant} = {index},");
    }
    out.push_str("}\n\nimpl DrawArgRow {\n");
    let _ = writeln!(out, "    pub const COUNT: usize = {};", DRAW_ARG_ROWS.len());
    out.push_str("    pub const TOTAL_SLOTS: usize = Self::COUNT * DrawKind::COUNT;\n");
    out.push_str("    pub(crate) const DRAW_ARGS_BYTES: u64 = size_of::<DrawArgs>() as u64;\n\n");
    out.push_str("    #[must_use]\n    pub const fn index(self) -> usize {\n        self as usize\n    }\n\n");
    out.push_str("    #[must_use]\n    pub const fn first_slot(self) -> usize {\n        self.index() * DrawKind::COUNT\n    }\n\n");
    out.push_str("    #[must_use]\n    pub const fn first_offset_bytes(self) -> u64 {\n        self.first_slot() as u64 * Self::DRAW_ARGS_BYTES\n    }\n\n");
    out.push_str("    #[must_use]\n    pub const fn slot(self, draw: DrawKind) -> usize {\n        self.first_slot() + draw.index()\n    }\n\n");
    out.push_str("    #[must_use]\n    pub const fn offset_bytes(self, draw: DrawKind) -> u64 {\n        self.first_offset_bytes() + draw.index() as u64 * Self::DRAW_ARGS_BYTES\n    }\n\n");
    out.push_str("    #[must_use]\n    pub(crate) const fn byte_len(rows: usize) -> u64 {\n        (rows * DrawKind::COUNT) as u64 * Self::DRAW_ARGS_BYTES\n    }\n");
    out.push_str("}\n\n");
}

fn generate_shared_types() -> String {
    let mut out = String::new();
    generate_draw_arg_row(&mut out);
    PARAMS_STRUCT.emit_rust(&mut out);
    CAMERA_STRUCT.emit_rust(&mut out);
    SECTION_DESC_STRUCT.emit_rust(&mut out);
    SPRITE_ENTRY_STRUCT.emit_rust(&mut out);
    ANIMATION_FRAME_STRUCT.emit_rust(&mut out);
    RECONSTRUCTION_STRUCT.emit_rust(&mut out);
    SKY_STRUCT.emit_rust(&mut out);
    TEXT_FRAME_STRUCT.emit_rust(&mut out);
    if out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn emit_binding_consts(out: &mut String, bindings: &[BindingDef]) {
    for binding in bindings {
        let _ = writeln!(
            out,
            "    pub const {}: u32 = {};",
            binding.const_name, binding.slot
        );
    }
    out.push('\n');
}

fn push_entry_build(out: &mut String, indent: &str, binding: &BindingDef) {
    let call = binding.kind.layout_call();
    let stages = binding.stages.rust_expr();
    let one_line = format!("{indent}{call}.build({}, {stages}),", binding.const_name);
    if one_line.len() <= 100 {
        out.push_str(&one_line);
        out.push('\n');
    } else if matches!(binding.stages, Stages::All) {
        let _ = writeln!(
            out,
            "{indent}{call}.build(\n{indent}    {},\n{indent}    {stages},\n{indent}),",
            binding.const_name
        );
    } else {
        let _ = writeln!(
            out,
            "{indent}{call}\n{indent}    .build({}, {stages}),",
            binding.const_name
        );
    }
}

fn emit_subset_layout(out: &mut String, label: &str, subset: &[BindingDef], layout_fn: &str) {
    let _ = writeln!(
        out,
        "    pub fn {layout_fn}() -> BindGroupLayoutDescriptor {{"
    );
    out.push_str("        BindGroupLayoutDescriptor::new(\n");
    let _ = writeln!(out, "            \"{label}\",");
    if let [only] = subset {
        let call = only.kind.layout_call();
        let stages = only.stages.rust_expr();
        let one_line = format!(
            "            &[{call}.build({}, {stages})],",
            only.const_name
        );
        if one_line.len() <= 100 {
            out.push_str(&one_line);
            out.push('\n');
        } else if call.len() <= 60 {
            let _ = writeln!(
                out,
                "            &[{call}\n                .build({}, {stages})],",
                only.const_name
            );
        } else {
            out.push_str("            &[\n");
            push_entry_build(out, "                ", only);
            out.push_str("            ],\n");
        }
    } else {
        out.push_str("            &[\n");
        for binding in subset {
            push_entry_build(out, "                ", binding);
        }
        out.push_str("            ],\n");
    }
    out.push_str("        )\n    }\n\n");
}

fn emit_subset_entries(
    out: &mut String,
    subset: &[BindingDef],
    entries_struct: &str,
    entries_fn: &str,
) {
    let count = subset.len();
    let _ = writeln!(
        out,
        "    pub fn {entries_fn}<'a>(entries: {entries_struct}<'a>) -> [BindGroupEntry<'a>; {count}] {{"
    );
    out.push_str(if count == 1 {
        "        ["
    } else {
        "        [\n"
    });
    for (idx, binding) in subset.iter().enumerate() {
        let resource = binding.kind.resource_expr(binding.name);
        if count == 1 && idx == 0 {
            let _ = writeln!(
                out,
                "BindGroupEntry {{\n            binding: {},\n            resource: {resource},\n        }}]",
                binding.const_name
            );
        } else {
            let _ = writeln!(
                out,
                "            BindGroupEntry {{\n                binding: {},\n                resource: {resource},\n            }},",
                binding.const_name
            );
        }
    }
    if count != 1 {
        out.push_str("        ]\n");
    }
    out.push_str("    }\n");
}

fn emit_group_subset(
    out: &mut String,
    label: &str,
    bindings: &[BindingDef],
    variant_mask: Option<u8>,
    entries_struct: &str,
    layout_fn: &str,
    entries_fn: &str,
) {
    let subset: Vec<BindingDef> = bindings
        .iter()
        .copied()
        .filter(|binding| variant_mask.is_none_or(|mask| binding.variants & mask != 0))
        .collect();
    let _ = writeln!(out, "    pub struct {entries_struct}<'a> {{");
    for binding in &subset {
        let _ = writeln!(
            out,
            "        pub {}: {},",
            binding.name,
            binding.kind.entry_param_ty()
        );
    }
    out.push_str("    }\n\n");
    emit_subset_layout(out, label, &subset, layout_fn);
    emit_subset_entries(out, &subset, entries_struct, entries_fn);
}

fn collect_identifiers(body: &str) -> std::collections::BTreeSet<String> {
    let mut tokens = std::collections::BTreeSet::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut word = String::new();
    for ch in body.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            if !word.is_empty() {
                tokens.insert(std::mem::take(&mut word));
            }
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' {
            word.push(ch);
        } else if !word.is_empty() {
            tokens.insert(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        tokens.insert(word);
    }
    tokens
}

fn emit_module_super_imports(out: &mut String, body: &str) {
    const SYMBOLS: &[&str] = &[
        "BindGroupEntry",
        "BindGroupLayoutDescriptor",
        "BindingResource",
        "Buffer",
        "BufferBinding",
        "CAMERA_SIZE",
        "NonZeroU64",
        "PARAMS_SIZE",
        "RECONSTRUCTION_SIZE",
        "RenderPath",
        "SKY_SIZE",
        "Sampler",
        "SamplerBindingType",
        "ShaderStages",
        "ShaderType",
        "StorageTextureAccess",
        "TEXT_FRAME_SIZE",
        "TextureFormat",
        "TextureSampleType",
        "TextureView",
        "ViewUniform",
        "Words",
        "sampler",
        "storage_buffer_read_only_sized",
        "storage_buffer_sized",
        "texture_2d",
        "texture_2d_array",
        "texture_depth_2d",
        "texture_storage_2d",
        "uniform_buffer_sized",
    ];
    let tokens = collect_identifiers(body);

    let used: Vec<&str> = SYMBOLS
        .iter()
        .copied()
        .filter(|sym| {
            tokens.contains(*sym) || (*sym == "ShaderType" && tokens.contains("ViewUniform"))
        })
        .collect();
    if used.is_empty() {
        return;
    }
    if used.len() == 1 {
        let _ = writeln!(out, "    use super::{};\n", used[0]);
        return;
    }
    let single = format!("    use super::{{{}}};", used.join(", "));
    if single.len() <= 100 {
        out.push_str(&single);
        out.push_str("\n\n");
        return;
    }
    out.push_str("    use super::{\n");
    let mut line = String::from("       ");
    for sym in used {
        if line.len() + 1 + sym.len() + 1 >= 100 {
            out.push_str(&line);
            out.push('\n');
            line.clear();
            line.push_str("       ");
        }
        line.push(' ');
        line.push_str(sym);
        line.push(',');
    }
    if line.len() > 7 {
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("    };\n\n");
}

fn emit_simple_group_module(
    out: &mut String,
    visibility: &str,
    mod_name: &str,
    label: &str,
    bindings: &[BindingDef],
) {
    let _ = writeln!(out, "{visibility} mod {mod_name} {{");
    let mut body = String::new();
    emit_binding_consts(&mut body, bindings);
    emit_group_subset(
        &mut body, label, bindings, None, "Entries", "layout", "entries",
    );
    emit_module_super_imports(out, &body);
    out.push_str(&body);
    out.push_str("}\n\n");
}

fn emit_terrain_draw_layout(out: &mut String) {
    out.push_str("    pub fn layout(path: RenderPath) -> BindGroupLayoutDescriptor {\n");
    out.push_str("        let words_layout = Words::layout(path);\n");
    out.push_str("        let mut entries = vec![\n");
    for binding in TERRAIN_DRAW_BINDINGS {
        if binding.cond == BindingCond::Always {
            push_entry_build(out, "            ", binding);
        }
    }
    out.push_str("        ];\n        if path.is_full() {\n");
    for binding in TERRAIN_DRAW_BINDINGS {
        if binding.cond == BindingCond::FullOnly {
            let line = format!(
                "            entries.push({}.build({}, {}));",
                binding.kind.layout_call(),
                binding.const_name,
                binding.stages.rust_expr()
            );
            if line.len() <= 100 {
                out.push_str(&line);
                out.push('\n');
            } else {
                let _ = writeln!(
                    out,
                    "            entries.push(\n                {}.build({}, {}),\n            );",
                    binding.kind.layout_call(),
                    binding.const_name,
                    binding.stages.rust_expr()
                );
            }
        }
    }
    out.push_str("        }\n        BindGroupLayoutDescriptor::new(\"terrain draw data\", &entries)\n    }\n\n");
}

fn emit_terrain_draw_entries(out: &mut String) {
    out.push_str("    pub fn entries<'a>(entries: Entries<'a>) -> Vec<BindGroupEntry<'a>> {\n");
    out.push_str("        let mut out = vec![\n");
    for binding in TERRAIN_DRAW_BINDINGS {
        if binding.cond == BindingCond::Always {
            let resource = binding.kind.resource_expr(binding.name);
            let _ = writeln!(
                out,
                "            BindGroupEntry {{\n                binding: {},\n                resource: {resource},\n            }},",
                binding.const_name
            );
        }
    }
    out.push_str("        ];\n");
    for binding in TERRAIN_DRAW_BINDINGS {
        if binding.cond == BindingCond::FullOnly {
            let _ = writeln!(
                out,
                "        if let Some(buf) = entries.{} {{",
                binding.name
            );
            let _ = writeln!(
                out,
                "            out.push(BindGroupEntry {{\n                binding: {},\n                resource: buf.as_entire_binding(),\n            }});",
                binding.const_name
            );
            out.push_str("        }\n");
        }
    }
    out.push_str("        out\n    }\n");
}

fn emit_terrain_draw_module(out: &mut String) {
    out.push_str("pub mod terrain_draw {\n");
    let mut body = String::new();
    emit_binding_consts(&mut body, TERRAIN_DRAW_BINDINGS);
    body.push_str("    pub struct Entries<'a> {\n");
    for binding in TERRAIN_DRAW_BINDINGS {
        let param_ty = binding.kind.entry_param_ty();
        if binding.cond == BindingCond::FullOnly {
            let _ = writeln!(body, "        pub {}: Option<{param_ty}>,", binding.name);
        } else {
            let _ = writeln!(body, "        pub {}: {param_ty},", binding.name);
        }
    }
    body.push_str("    }\n\n");
    emit_terrain_draw_layout(&mut body);
    emit_terrain_draw_entries(&mut body);
    emit_module_super_imports(out, &body);
    out.push_str(&body);
    out.push_str("}\n\n");
}

fn emit_shared_bindings_header(out: &mut String) {
    out.push_str("use std::num::NonZeroU64;\n\n");
    out.push_str("use bevy::render::render_resource::binding_types::{\n");
    out.push_str("    sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_2d, texture_2d_array,\n");
    out.push_str("    texture_depth_2d, texture_storage_2d, uniform_buffer_sized,\n");
    out.push_str("};\n");
    out.push_str("use bevy::render::render_resource::{\n");
    out.push_str("    BindGroupEntry, BindGroupLayoutDescriptor, BindingResource, Buffer, BufferBinding, Sampler,\n");
    out.push_str(
        "    SamplerBindingType, ShaderStages, ShaderType, StorageTextureAccess, TextureFormat,\n",
    );
    out.push_str("    TextureSampleType, TextureView,\n");
    out.push_str("};\n");
    out.push_str("use bevy::render::view::ViewUniform;\n\n");
    out.push_str("use crate::render_path::RenderPath;\n");
    out.push_str("use crate::shared_types::{\n");
    out.push_str("    CAMERA_SIZE, PARAMS_SIZE, RECONSTRUCTION_SIZE, SKY_SIZE, TEXT_FRAME_SIZE,\n");
    out.push_str("};\n");
    out.push_str("use crate::words::Words;\n\n");
}

fn emit_cull_bindings(out: &mut String) {
    out.push_str("pub mod cull {\n");
    let mut body = String::new();
    emit_binding_consts(&mut body, CULL_BINDINGS);
    emit_group_subset(
        &mut body,
        "terrain cull data",
        CULL_BINDINGS,
        Some(CULL_ORDERED),
        "OrderedEntries",
        "ordered_layout",
        "ordered_entries",
    );
    body.push('\n');
    emit_group_subset(
        &mut body,
        "terrain quad cull data",
        CULL_BINDINGS,
        Some(CULL_QUAD),
        "QuadEntries",
        "quad_layout",
        "quad_entries",
    );
    emit_module_super_imports(out, &body);
    out.push_str(&body);
    out.push_str("}\n\n");
}

fn emit_hiz_bindings(out: &mut String) {
    out.push_str("pub mod hiz {\n");
    let mut body = String::new();
    emit_binding_consts(&mut body, HIZ_BINDINGS);
    emit_group_subset(
        &mut body,
        "hiz from depth",
        HIZ_BINDINGS,
        Some(HIZ_FROM_DEPTH),
        "FromDepthEntries",
        "from_depth_layout",
        "from_depth_entries",
    );
    body.push('\n');
    emit_group_subset(
        &mut body,
        "hiz down",
        HIZ_BINDINGS,
        Some(HIZ_DOWN),
        "DownEntries",
        "down_layout",
        "down_entries",
    );
    emit_module_super_imports(out, &body);
    out.push_str(&body);
    out.push_str("}\n\n");
}

fn generate_shared_bindings() -> String {
    let mut out = String::new();
    emit_shared_bindings_header(&mut out);
    emit_simple_group_module(
        &mut out,
        "pub",
        "terrain_view",
        "terrain frame",
        TERRAIN_VIEW_BINDINGS,
    );
    emit_terrain_draw_module(&mut out);
    emit_cull_bindings(&mut out);
    emit_simple_group_module(
        &mut out,
        "pub",
        "gbuffer",
        "deferred gbuffer",
        GBUFFER_BINDINGS,
    );
    emit_simple_group_module(
        &mut out,
        "pub(crate)",
        "transparency",
        "improved transparency",
        TRANSPARENCY_BINDINGS,
    );
    let combined: Vec<BindingDef> = GBUFFER_BINDINGS
        .iter()
        .chain(TRANSPARENCY_BINDINGS.iter())
        .copied()
        .collect();
    emit_simple_group_module(
        &mut out,
        "pub(crate)",
        "transparent_gbuffer",
        "deferred gbuffer with improved transparency",
        &combined,
    );
    emit_simple_group_module(
        &mut out,
        "pub(crate)",
        "sky_view",
        "sky view",
        SKY_VIEW_BINDINGS,
    );
    emit_simple_group_module(
        &mut out,
        "pub(crate)",
        "sky_textures",
        "sky textures",
        SKY_TEXTURES_BINDINGS,
    );
    emit_hiz_bindings(&mut out);
    emit_simple_group_module(&mut out, "pub", "present", "present", PRESENT_BINDINGS);
    emit_simple_group_module(&mut out, "pub(crate)", "show", "show depth", SHOW_BINDINGS);
    emit_simple_group_module(
        &mut out,
        "pub",
        "world_text_view",
        "world text view",
        WORLD_TEXT_VIEW_BINDINGS,
    );
    emit_simple_group_module(
        &mut out,
        "pub",
        "world_text_textures",
        "world text texture",
        WORLD_TEXT_TEXTURES_BINDINGS,
    );
    if out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn emit_wgsl_bindings(out: &mut String, group: u32, bindings: &[BindingDef]) {
    for binding in bindings {
        out.push_str(
            &binding
                .kind
                .wgsl_decl(group, binding.slot, binding.name, binding.cond),
        );
    }
}

fn wgsl_module_with_bindings(imports: &[&str], group: u32, bindings: &[BindingDef]) -> String {
    let mut out = String::new();
    for import in imports {
        let _ = writeln!(out, "import {import};");
    }
    if !imports.is_empty() {
        out.push('\n');
    }
    emit_wgsl_bindings(&mut out, group, bindings);
    out
}

fn generate_cull_wesl() -> String {
    let mut out = String::new();
    out.push_str("import mcrs_minecraft_render::shaders::include::fields::Group;\n");
    out.push_str(
        "import mcrs_minecraft_render::shaders::include::section_layout::SectionDesc;\n\n",
    );
    out.push_str("struct DrawArgs {\n");
    for field in DRAW_ARGS_WGSL_FIELDS {
        let _ = writeln!(out, "    {}: {},", field.name, field.ty.wgsl_ty());
    }
    out.push_str("}\n\n");
    let _ = writeln!(out, "const DRAW_KINDS: u32 = {}u;", DrawKind::COUNT);
    for (index, (_, wgsl_const)) in DRAW_ARG_ROWS.iter().enumerate() {
        let _ = writeln!(out, "const {wgsl_const}: u32 = {index}u;");
    }
    out.push('\n');
    emit_wgsl_bindings(&mut out, 1, CULL_BINDINGS);
    out
}

fn generate_wesl_files() -> Vec<(&'static str, String)> {
    let mut frame = String::new();
    PARAMS_STRUCT.emit_wgsl(&mut frame);
    CAMERA_STRUCT.emit_wgsl(&mut frame);
    emit_wgsl_bindings(&mut frame, 0, TERRAIN_VIEW_BINDINGS);

    let mut section_layout = String::new();
    SECTION_DESC_STRUCT.emit_wgsl(&mut section_layout);

    let mut sprites_layout = String::new();
    SPRITE_ENTRY_STRUCT.emit_wgsl(&mut sprites_layout);
    ANIMATION_FRAME_STRUCT.emit_wgsl(&mut sprites_layout);
    let _ = writeln!(sprites_layout, "const STILL: u32 = 0xFFFFFFFFu;");

    let terrain_layout = wgsl_module_with_bindings(&[], 1, TERRAIN_DRAW_BINDINGS);
    let cull_layout = generate_cull_wesl();
    let mut reconstruct = String::new();
    RECONSTRUCTION_STRUCT.emit_wgsl(&mut reconstruct);

    let reconstruct_import = "mcrs_minecraft_render::shaders::include::reconstruct::Reconstruction";
    let gbuffer_layout = wgsl_module_with_bindings(&[reconstruct_import], 2, GBUFFER_BINDINGS);
    let gbuffer_views_layout =
        wgsl_module_with_bindings(&[reconstruct_import], 1, GBUFFER_BINDINGS);
    let transparency_layout = wgsl_module_with_bindings(&[], 2, TRANSPARENCY_BINDINGS);

    let mut sky = String::new();
    SKY_STRUCT.emit_wgsl(&mut sky);

    let mut sky_layout = wgsl_module_with_bindings(
        &[
            "bevy_render::view::View",
            "mcrs_minecraft_render::shaders::include::sky::Sky",
        ],
        0,
        SKY_VIEW_BINDINGS,
    );
    sky_layout.push('\n');
    emit_wgsl_bindings(&mut sky_layout, 1, SKY_TEXTURES_BINDINGS);

    let mut world_text_layout = String::new();
    TEXT_FRAME_STRUCT.emit_wgsl(&mut world_text_layout);
    emit_wgsl_bindings(&mut world_text_layout, 0, WORLD_TEXT_VIEW_BINDINGS);
    world_text_layout.push('\n');
    emit_wgsl_bindings(&mut world_text_layout, 1, WORLD_TEXT_TEXTURES_BINDINGS);

    vec![
        ("src/shaders/include/frame.wesl", frame),
        ("src/shaders/include/section_layout.wesl", section_layout),
        ("src/shaders/include/sprites_layout.wesl", sprites_layout),
        ("src/shaders/include/terrain_layout.wesl", terrain_layout),
        ("src/shaders/include/cull_layout.wesl", cull_layout),
        ("src/shaders/include/reconstruct.wesl", reconstruct),
        ("src/shaders/include/gbuffer_layout.wesl", gbuffer_layout),
        (
            "src/shaders/include/gbuffer_views_layout.wesl",
            gbuffer_views_layout,
        ),
        (
            "src/shaders/include/transparency_layout.wesl",
            transparency_layout,
        ),
        ("src/shaders/include/sky.wesl", sky),
        ("src/shaders/include/sky_layout.wesl", sky_layout),
        (
            "src/shaders/include/hiz_layout.wesl",
            wgsl_module_with_bindings(&[], 0, HIZ_BINDINGS),
        ),
        (
            "src/shaders/include/present_layout.wesl",
            wgsl_module_with_bindings(&[], 0, PRESENT_BINDINGS),
        ),
        (
            "src/shaders/include/show_layout.wesl",
            wgsl_module_with_bindings(&[], 0, SHOW_BINDINGS),
        ),
        (
            "src/shaders/include/world_text_layout.wesl",
            world_text_layout,
        ),
    ]
}

#[expect(
    clippy::disallowed_methods,
    reason = "MCRS_BLESS rewrites generated layout files only when requested by the developer"
)]
fn bless_or_check(path: &Path, generated: &str) {
    if std::env::var("MCRS_BLESS").is_ok_and(|value| value == "1") {
        std::fs::write(path, generated).expect("cannot rewrite generated layout file");
        return;
    }
    let checked_in = std::fs::read_to_string(path).unwrap_or_default();
    assert_eq!(
        checked_in,
        generated,
        "{} is stale; rerun `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_render`",
        path.display()
    );
}

#[test]
fn the_generated_shared_types_bindings_and_wesl_match_the_layout_tables() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    bless_or_check(&root.join("src/shared_types.rs"), &generate_shared_types());
    bless_or_check(
        &root.join("src/shared_bindings.rs"),
        &generate_shared_bindings(),
    );
    for (rel, content) in generate_wesl_files() {
        bless_or_check(&root.join(rel), &content);
    }
}

struct NagaCheck {
    struct_name: &'static str,
    rust_size: usize,
    uniform: bool,
    min_stride_bytes: usize,
    fields: &'static [(&'static str, usize)],
}

fn verify_naga_struct(wgsl_source: &str, check: &NagaCheck) {
    let module = parse_str(wgsl_source).expect("generated WGSL parses with naga");
    let Some(naga::TypeInner::Struct { members, span }) = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some(check.struct_name))
        .map(|(_, ty)| &ty.inner)
    else {
        panic!("missing WGSL struct {}", check.struct_name);
    };
    let shader_fields: Vec<(&str, usize)> = members
        .iter()
        .map(|member| (member.name.as_deref().unwrap_or(""), member.offset as usize))
        .collect();
    assert_eq!(
        shader_fields.as_slice(),
        check.fields,
        "field offsets mismatch for WGSL struct {}",
        check.struct_name
    );
    let expected_size = if check.uniform {
        (*span as usize)
            .max(check.min_stride_bytes)
            .next_multiple_of(16)
    } else {
        (*span as usize).max(check.min_stride_bytes)
    };
    assert_eq!(
        check.rust_size, expected_size,
        "size mismatch for {}",
        check.struct_name
    );
}

#[test]
fn terrain_frame_structs_match_naga_offsets_and_sizes() {
    verify_naga_struct(
        include_str!("shaders/include/frame.wesl"),
        &NagaCheck {
            struct_name: "Params",
            rust_size: mem::size_of::<Params>(),
            uniform: true,
            min_stride_bytes: 0,
            fields: &[
                ("group_base", mem::offset_of!(Params, group_base)),
                ("group_count", mem::offset_of!(Params, group_count)),
                ("visible_base", mem::offset_of!(Params, visible_base)),
                ("args_index", mem::offset_of!(Params, args_index)),
                ("counter", mem::offset_of!(Params, counter)),
                ("flags", mem::offset_of!(Params, flags)),
            ],
        },
    );
    verify_naga_struct(
        include_str!("shaders/include/frame.wesl"),
        &NagaCheck {
            struct_name: "Camera",
            rust_size: mem::size_of::<CameraUniform>(),
            uniform: true,
            min_stride_bytes: 0,
            fields: &[
                (
                    "clip_from_relative",
                    mem::offset_of!(CameraUniform, clip_from_relative),
                ),
                ("frustum", mem::offset_of!(CameraUniform, frustum)),
                ("section", mem::offset_of!(CameraUniform, section)),
                ("offset", mem::offset_of!(CameraUniform, offset)),
                ("tint_origin", mem::offset_of!(CameraUniform, tint_origin)),
                ("tint_scale", mem::offset_of!(CameraUniform, tint_scale)),
                ("hiz_levels", mem::offset_of!(CameraUniform, hiz_levels)),
                ("quad_cull", mem::offset_of!(CameraUniform, quad_cull)),
                ("viewport", mem::offset_of!(CameraUniform, viewport)),
                ("shade_table", mem::offset_of!(CameraUniform, shade_table)),
                (
                    "render_distance",
                    mem::offset_of!(CameraUniform, render_distance),
                ),
                (
                    "raster_viewport",
                    mem::offset_of!(CameraUniform, raster_viewport),
                ),
                ("fog_color", mem::offset_of!(CameraUniform, fog_color)),
                (
                    "fog_environmental",
                    mem::offset_of!(CameraUniform, fog_environmental),
                ),
                (
                    "fog_render_distance",
                    mem::offset_of!(CameraUniform, fog_render_distance),
                ),
            ],
        },
    );
    verify_naga_struct(
        include_str!("shaders/include/section_layout.wesl"),
        &NagaCheck {
            struct_name: "SectionDesc",
            rust_size: mem::size_of::<SectionDesc>(),
            uniform: false,
            min_stride_bytes: 0,
            fields: &[
                ("x", mem::offset_of!(SectionDesc, section)),
                ("y", mem::offset_of!(SectionDesc, section) + 4),
                ("z", mem::offset_of!(SectionDesc, section) + 8),
                ("scale", mem::offset_of!(SectionDesc, scale)),
                ("face_base", mem::offset_of!(SectionDesc, face_base)),
            ],
        },
    );
    assert_eq!(
        SECTION_DESC_STRUCT.padded_size(),
        mem::size_of::<SectionDesc>()
    );
}

#[test]
fn terrain_material_and_cull_structs_match_naga_offsets_and_sizes() {
    let sprites_wgsl = include_str!("shaders/include/sprites_layout.wesl");
    verify_naga_struct(
        sprites_wgsl,
        &NagaCheck {
            struct_name: "SpriteEntry",
            rust_size: mem::size_of::<SpriteEntry>(),
            uniform: false,
            min_stride_bytes: 0,
            fields: &[
                ("array_layer", mem::offset_of!(SpriteEntry, array_layer)),
                ("animation", mem::offset_of!(SpriteEntry, animation)),
            ],
        },
    );
    assert_eq!(
        SPRITE_ENTRY_STRUCT.padded_size(),
        mem::size_of::<SpriteEntry>()
    );
    verify_naga_struct(
        sprites_wgsl,
        &NagaCheck {
            struct_name: "AnimationFrame",
            rust_size: mem::size_of::<AnimationFrame>(),
            uniform: false,
            min_stride_bytes: ANIMATION_FRAME_STRUCT.padded_size(),
            fields: &[
                ("layer", mem::offset_of!(AnimationFrame, layer)),
                ("next", mem::offset_of!(AnimationFrame, next)),
                ("blend", mem::offset_of!(AnimationFrame, blend)),
            ],
        },
    );
    let cull_wgsl = wesl_resolve_file("shaders/include/cull_layout.wesl");
    verify_naga_struct(
        &cull_wgsl,
        &NagaCheck {
            struct_name: "DrawArgs",
            rust_size: mem::size_of::<DrawArgs>(),
            uniform: false,
            min_stride_bytes: 0,
            fields: &[
                ("vertex_count", mem::offset_of!(DrawArgs, vertex_count)),
                ("instance_count", mem::offset_of!(DrawArgs, instance_count)),
                ("first_vertex", mem::offset_of!(DrawArgs, first_vertex)),
                ("first_instance", mem::offset_of!(DrawArgs, first_instance)),
            ],
        },
    );
    verify_naga_struct(
        include_str!("shaders/include/fields.wesl"),
        &NagaCheck {
            struct_name: "Group",
            rust_size: mem::size_of::<mcrs_minecraft_mesh::Group>(),
            uniform: false,
            min_stride_bytes: 0,
            fields: &[
                (
                    "quad_base",
                    mem::offset_of!(mcrs_minecraft_mesh::Group, quad_base),
                ),
                (
                    "quad_count",
                    mem::offset_of!(mcrs_minecraft_mesh::Group, quad_count),
                ),
                (
                    "section",
                    mem::offset_of!(mcrs_minecraft_mesh::Group, section),
                ),
                ("face", mem::offset_of!(mcrs_minecraft_mesh::Group, face)),
                (
                    "bounds",
                    mem::offset_of!(mcrs_minecraft_mesh::Group, bounds),
                ),
            ],
        },
    );
}

#[test]
fn pass_and_view_structs_match_naga_offsets_and_sizes() {
    verify_naga_struct(
        include_str!("shaders/include/reconstruct.wesl"),
        &NagaCheck {
            struct_name: "Reconstruction",
            rust_size: mem::size_of::<Reconstruction>(),
            uniform: true,
            min_stride_bytes: 0,
            fields: &[
                (
                    "relative_from_clip",
                    mem::offset_of!(Reconstruction, relative_from_clip),
                ),
                ("viewport", mem::offset_of!(Reconstruction, viewport)),
            ],
        },
    );
    verify_naga_struct(
        include_str!("shaders/include/sky.wesl"),
        &NagaCheck {
            struct_name: "Sky",
            rust_size: mem::size_of::<SkyUniform>(),
            uniform: true,
            min_stride_bytes: 0,
            fields: &[
                ("disc", mem::offset_of!(SkyUniform, disc)),
                ("sunrise", mem::offset_of!(SkyUniform, sunrise)),
                ("angles", mem::offset_of!(SkyUniform, angles)),
                ("moon", mem::offset_of!(SkyUniform, moon)),
                ("fog", mem::offset_of!(SkyUniform, fog)),
                ("cloud_color", mem::offset_of!(SkyUniform, cloud_color)),
                ("cloud", mem::offset_of!(SkyUniform, cloud)),
                ("sky_light", mem::offset_of!(SkyUniform, sky_light)),
                ("block_light", mem::offset_of!(SkyUniform, block_light)),
                ("ambient", mem::offset_of!(SkyUniform, ambient)),
                ("occluder", mem::offset_of!(SkyUniform, occluder)),
            ],
        },
    );
    verify_naga_struct(
        include_str!("shaders/include/world_text_layout.wesl"),
        &NagaCheck {
            struct_name: "TextFrame",
            rust_size: mem::size_of::<TextFrame>(),
            uniform: true,
            min_stride_bytes: 0,
            fields: &[
                (
                    "clip_from_relative",
                    mem::offset_of!(TextFrame, clip_from_relative),
                ),
                ("section", mem::offset_of!(TextFrame, section)),
            ],
        },
    );
}

fn wesl_resolve_file(rel: &str) -> String {
    struct LocalResolver;
    impl Resolver for LocalResolver {
        fn resolve_source<'a>(
            &'a self,
            path: &ModulePath,
        ) -> Result<std::borrow::Cow<'a, str>, ResolveError> {
            let file_path = match &path.origin {
                PathOrigin::Package(pkg) if pkg == "mcrs_minecraft_render" => {
                    let tail: String = path.components.join("/");
                    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("src/{tail}.wesl"))
                }
                _ => {
                    return Err(ResolveError::ModuleNotFound(
                        path.clone(),
                        "unexpected package in layout check".to_owned(),
                    ));
                }
            };
            let src = std::fs::read_to_string(&file_path)
                .map_err(|err| ResolveError::ModuleNotFound(path.clone(), err.to_string()))?;
            Ok(std::borrow::Cow::Owned(src))
        }
    }
    let root = ModulePath {
        origin: PathOrigin::Package("mcrs_minecraft_render".to_owned()),
        components: rel
            .trim_end_matches(".wesl")
            .split('/')
            .map(ToOwned::to_owned)
            .collect(),
    };
    let options = CompileOptions {
        imports: true,
        condcomp: true,
        strip: false,
        visibility: false,
        ..CompileOptions::default()
    };
    let compiled = wesl::compile(&root, &options, &LocalResolver).expect("generated WESL compiles");
    compiled.to_string()
}
