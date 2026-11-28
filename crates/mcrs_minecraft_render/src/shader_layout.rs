use std::fmt::Write;

#[derive(Copy, Clone)]
pub(super) enum FieldTy {
    U32,
    F32,
    AtomicU32,
    Vec2F32,
    Vec3F32,
    Vec4F32,
    Vec3I32,
    Mat4x4F32,
    Vec4F32Array(usize),
    ScalarI32x3([&'static str; 3]),
}

impl FieldTy {
    pub(super) const fn align(self) -> usize {
        match self {
            Self::U32 | Self::F32 | Self::AtomicU32 | Self::ScalarI32x3(_) => 4,
            Self::Vec2F32 => 8,
            Self::Vec3F32
            | Self::Vec4F32
            | Self::Vec3I32
            | Self::Mat4x4F32
            | Self::Vec4F32Array(_) => 16,
        }
    }

    pub(super) const fn size(self) -> usize {
        match self {
            Self::U32 | Self::F32 | Self::AtomicU32 => 4,
            Self::Vec2F32 => 8,
            Self::Vec3F32 | Self::Vec3I32 | Self::ScalarI32x3(_) => 12,
            Self::Vec4F32 => 16,
            Self::Mat4x4F32 => 64,
            Self::Vec4F32Array(len) => 16 * len,
        }
    }

    pub(super) const fn has_floats(self) -> bool {
        matches!(
            self,
            Self::F32
                | Self::Vec2F32
                | Self::Vec3F32
                | Self::Vec4F32
                | Self::Mat4x4F32
                | Self::Vec4F32Array(_)
        )
    }

    pub(super) fn rust_ty(self) -> String {
        match self {
            Self::U32 | Self::AtomicU32 => "u32".to_owned(),
            Self::F32 => "f32".to_owned(),
            Self::Vec2F32 => "[f32; 2]".to_owned(),
            Self::Vec3F32 => "[f32; 3]".to_owned(),
            Self::Vec4F32 => "[f32; 4]".to_owned(),
            Self::Vec3I32 | Self::ScalarI32x3(_) => "[i32; 3]".to_owned(),
            Self::Mat4x4F32 => "[f32; 16]".to_owned(),
            Self::Vec4F32Array(len) => format!("[[f32; 4]; {len}]"),
        }
    }

    pub(super) fn wgsl_ty(self) -> String {
        match self {
            Self::U32 => "u32".to_owned(),
            Self::F32 => "f32".to_owned(),
            Self::AtomicU32 => "atomic<u32>".to_owned(),
            Self::Vec2F32 => "vec2<f32>".to_owned(),
            Self::Vec3F32 => "vec3<f32>".to_owned(),
            Self::Vec4F32 => "vec4<f32>".to_owned(),
            Self::Vec3I32 => "vec3<i32>".to_owned(),
            Self::Mat4x4F32 => "mat4x4<f32>".to_owned(),
            Self::Vec4F32Array(len) => format!("array<vec4<f32>, {len}>"),
            Self::ScalarI32x3(_) => "i32".to_owned(),
        }
    }
}

pub(super) struct FieldDef {
    pub name: &'static str,
    pub ty: FieldTy,
}

const fn field(name: &'static str, ty: FieldTy) -> FieldDef {
    FieldDef { name, ty }
}

pub(super) struct ConstDef {
    pub name: &'static str,
    pub value: u32,
}

pub(super) struct StructDef {
    pub rust_name: &'static str,
    pub wgsl_name: &'static str,
    pub size_const: Option<(&'static str, &'static str)>,
    pub words_const: Option<&'static str>,
    pub min_stride_bytes: usize,
    pub uniform: bool,
    pub visibility: &'static str,
    pub fields: &'static [FieldDef],
    pub consts: &'static [ConstDef],
}

impl StructDef {
    pub(super) fn padded_size(&self) -> usize {
        let mut offset = 0usize;
        let mut max_align = 1usize;
        for field in self.fields {
            let align = field.ty.align();
            max_align = max_align.max(align);
            offset = offset.next_multiple_of(align);
            offset += field.ty.size();
        }
        let target = offset.max(self.min_stride_bytes);
        let align = if self.uniform { 16 } else { max_align };
        target.next_multiple_of(align)
    }

    pub(super) fn has_floats(&self) -> bool {
        self.fields.iter().any(|field| field.ty.has_floats())
    }

    pub(super) fn emit_rust(&self, out: &mut String) {
        for constant in self.consts {
            let _ = writeln!(
                out,
                "{} const {}: u32 = {};",
                self.visibility, constant.name, constant.value
            );
        }
        if !self.consts.is_empty() {
            out.push('\n');
        }
        let derives = if self.has_floats() {
            "Copy, Clone, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable"
        } else {
            "Copy, Clone, Default, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable"
        };
        let _ = writeln!(out, "#[derive({derives})]");
        let _ = writeln!(out, "#[repr(C)]");
        let _ = writeln!(out, "{} struct {} {{", self.visibility, self.rust_name);
        let mut offset = 0usize;
        for field in self.fields {
            let aligned = offset.next_multiple_of(field.ty.align());
            emit_rust_padding(out, offset, aligned);
            let _ = writeln!(out, "    pub {}: {},", field.name, field.ty.rust_ty());
            offset = aligned + field.ty.size();
        }
        let total = self.padded_size();
        emit_rust_padding(out, offset, total);
        out.push_str("}\n");
        if let Some((size_vis, size_const)) = self.size_const {
            let _ = writeln!(
                out,
                "\n{size_vis} const {size_const}: u64 = size_of::<{}>() as u64;",
                self.rust_name
            );
            if self.uniform {
                let _ = writeln!(out, "const _: () = assert!({size_const} > 0);");
            }
        }
        out.push('\n');
    }

    pub(super) fn emit_wgsl(&self, out: &mut String) {
        let _ = writeln!(out, "struct {} {{", self.wgsl_name);
        for field in self.fields {
            match field.ty {
                FieldTy::ScalarI32x3(names) => {
                    for name in names {
                        let _ = writeln!(out, "    {name}: i32,");
                    }
                }
                ty => {
                    let _ = writeln!(out, "    {}: {},", field.name, ty.wgsl_ty());
                }
            }
        }
        out.push_str("}\n");
        if let Some(words_const) = self.words_const {
            let words = self.padded_size() / 4;
            let _ = writeln!(out, "\nconst {words_const}: u32 = {words}u;");
        }
        for constant in self.consts {
            let _ = writeln!(out, "const {}: u32 = {}u;", constant.name, constant.value);
        }
        out.push('\n');
    }
}

fn emit_rust_padding(out: &mut String, from: usize, to: usize) {
    let bytes = to.saturating_sub(from);
    if bytes == 0 {
        return;
    }
    let words = bytes / 4;
    let ty = if words == 1 {
        "u32".to_owned()
    } else {
        format!("[u32; {words}]")
    };
    let _ = writeln!(out, "    pub(crate) _pad_{from}: {ty},");
}

pub(super) const DRAW_ARG_ROWS: &[(&str, &str)] = &[
    ("FirstPassDraws", "ROW_FIRST_PASS_DRAWS"),
    ("SecondPassDraws", "ROW_SECOND_PASS_DRAWS"),
    ("HiddenQuads", "ROW_HIDDEN_QUADS"),
    ("FirstPassSurvivors", "ROW_FIRST_PASS_SURVIVORS"),
    ("SecondPassSurvivors", "ROW_SECOND_PASS_SURVIVORS"),
    ("Candidates", "ROW_CANDIDATES"),
    ("FirstPassQuadDispatch", "ROW_FIRST_PASS_QUAD_DISPATCH"),
    ("SecondPassQuadDispatch", "ROW_SECOND_PASS_QUAD_DISPATCH"),
    ("CandidateDispatch", "ROW_CANDIDATE_DISPATCH"),
];

pub(super) const PARAMS_STRUCT: StructDef = StructDef {
    rust_name: "Params",
    wgsl_name: "Params",
    size_const: Some(("pub(crate)", "PARAMS_SIZE")),
    words_const: None,
    min_stride_bytes: 0,
    uniform: true,
    visibility: "pub",
    fields: &[
        field("group_base", FieldTy::U32),
        field("group_count", FieldTy::U32),
        field("visible_base", FieldTy::U32),
        field("args_index", FieldTy::U32),
        field("counter", FieldTy::U32),
        field("flags", FieldTy::U32),
    ],
    consts: &[
        ConstDef {
            name: "PARAMS_MODEL",
            value: 1,
        },
        ConstDef {
            name: "PARAMS_QUAD_CULL",
            value: 2,
        },
    ],
};

pub(super) const CAMERA_STRUCT: StructDef = StructDef {
    rust_name: "CameraUniform",
    wgsl_name: "Camera",
    size_const: Some(("pub(crate)", "CAMERA_SIZE")),
    words_const: None,
    min_stride_bytes: 0,
    uniform: true,
    visibility: "pub",
    fields: &[
        field("clip_from_relative", FieldTy::Mat4x4F32),
        field("frustum", FieldTy::Vec4F32Array(5)),
        field("section", FieldTy::Vec3I32),
        field("offset", FieldTy::Vec3F32),
        field("tint_origin", FieldTy::Vec2F32),
        field("tint_scale", FieldTy::Vec2F32),
        field("hiz_levels", FieldTy::U32),
        field("quad_cull", FieldTy::U32),
        field("viewport", FieldTy::Vec2F32),
        field("shade_table", FieldTy::U32),
        field("render_distance", FieldTy::U32),
        field("raster_viewport", FieldTy::Vec2F32),
        field("fog_color", FieldTy::Vec3F32),
        field("fog_environmental", FieldTy::Vec2F32),
        field("fog_render_distance", FieldTy::Vec2F32),
    ],
    consts: &[],
};

pub(super) const SECTION_DESC_STRUCT: StructDef = StructDef {
    rust_name: "SectionDesc",
    wgsl_name: "SectionDesc",
    size_const: Some(("pub(crate)", "SECTION_BYTES")),
    words_const: Some("SECTION_WORDS"),
    min_stride_bytes: 0,
    uniform: false,
    visibility: "pub",
    fields: &[
        field("section", FieldTy::ScalarI32x3(["x", "y", "z"])),
        field("scale", FieldTy::U32),
        field("face_base", FieldTy::U32),
    ],
    consts: &[],
};

pub(super) const SPRITE_ENTRY_STRUCT: StructDef = StructDef {
    rust_name: "SpriteEntry",
    wgsl_name: "SpriteEntry",
    size_const: None,
    words_const: Some("SPRITE_WORDS"),
    min_stride_bytes: 0,
    uniform: false,
    visibility: "pub",
    fields: &[
        field("array_layer", FieldTy::U32),
        field("animation", FieldTy::U32),
    ],
    consts: &[],
};

pub(super) const ANIMATION_FRAME_STRUCT: StructDef = StructDef {
    rust_name: "AnimationFrame",
    wgsl_name: "AnimationFrame",
    size_const: None,
    words_const: Some("ANIMATION_WORDS"),
    min_stride_bytes: 16,
    uniform: false,
    visibility: "pub(crate)",
    fields: &[
        field("layer", FieldTy::U32),
        field("next", FieldTy::U32),
        field("blend", FieldTy::F32),
    ],
    consts: &[],
};

pub(super) const RECONSTRUCTION_STRUCT: StructDef = StructDef {
    rust_name: "Reconstruction",
    wgsl_name: "Reconstruction",
    size_const: Some(("pub(crate)", "RECONSTRUCTION_SIZE")),
    words_const: None,
    min_stride_bytes: 0,
    uniform: true,
    visibility: "pub(crate)",
    fields: &[
        field("relative_from_clip", FieldTy::Mat4x4F32),
        field("viewport", FieldTy::Vec2F32),
    ],
    consts: &[],
};

pub(super) const SKY_STRUCT: StructDef = StructDef {
    rust_name: "SkyUniform",
    wgsl_name: "Sky",
    size_const: Some(("pub(crate)", "SKY_SIZE")),
    words_const: None,
    min_stride_bytes: 0,
    uniform: true,
    visibility: "pub",
    fields: &[
        field("disc", FieldTy::Vec4F32),
        field("sunrise", FieldTy::Vec4F32),
        field("angles", FieldTy::Vec4F32),
        field("moon", FieldTy::Vec4F32),
        field("fog", FieldTy::Vec4F32),
        field("cloud_color", FieldTy::Vec4F32),
        field("cloud", FieldTy::Vec4F32),
        field("sky_light", FieldTy::Vec4F32),
        field("block_light", FieldTy::Vec4F32),
        field("ambient", FieldTy::Vec4F32),
        field("occluder", FieldTy::Vec4F32),
    ],
    consts: &[],
};

pub(super) const TEXT_FRAME_STRUCT: StructDef = StructDef {
    rust_name: "TextFrame",
    wgsl_name: "TextFrame",
    size_const: Some(("pub(crate)", "TEXT_FRAME_SIZE")),
    words_const: None,
    min_stride_bytes: 0,
    uniform: true,
    visibility: "pub(crate)",
    fields: &[
        field("clip_from_relative", FieldTy::Mat4x4F32),
        field("section", FieldTy::Vec3I32),
    ],
    consts: &[],
};

pub(super) const DRAW_ARGS_WGSL_FIELDS: &[FieldDef] = &[
    field("vertex_count", FieldTy::AtomicU32),
    field("instance_count", FieldTy::U32),
    field("first_vertex", FieldTy::U32),
    field("first_instance", FieldTy::U32),
];

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum Stages {
    Vertex,
    Fragment,
    Compute,
    VertexFragment,
    All,
}

impl Stages {
    pub(super) const fn rust_expr(self) -> &'static str {
        match self {
            Self::Vertex => "ShaderStages::VERTEX",
            Self::Fragment => "ShaderStages::FRAGMENT",
            Self::Compute => "ShaderStages::COMPUTE",
            Self::VertexFragment => "ShaderStages::VERTEX_FRAGMENT",
            Self::All => "ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum BindingKind {
    UniformBuffer {
        size_expr: &'static str,
        dynamic: bool,
        wgsl_ty: &'static str,
    },
    StorageBuffer {
        read_only: bool,
        wgsl_ty: &'static str,
        words: bool,
    },
    Words,
    Texture2D {
        filterable: bool,
    },
    Texture2DArray {
        filterable: bool,
    },
    TextureDepth2D,
    StorageTextureR32Write,
    Sampler {
        filtering: bool,
    },
}

impl BindingKind {
    pub(super) fn layout_call(self) -> String {
        match self {
            Self::UniformBuffer {
                size_expr, dynamic, ..
            } => format!("uniform_buffer_sized({dynamic}, {size_expr})"),
            Self::StorageBuffer {
                read_only: true, ..
            } => "storage_buffer_read_only_sized(false, None)".to_owned(),
            Self::StorageBuffer {
                read_only: false, ..
            } => "storage_buffer_sized(false, None)".to_owned(),
            Self::Words => "words_layout".to_owned(),
            Self::Texture2D { filterable } => {
                format!("texture_2d(TextureSampleType::Float {{ filterable: {filterable} }})")
            }
            Self::Texture2DArray { filterable } => {
                format!("texture_2d_array(TextureSampleType::Float {{ filterable: {filterable} }})")
            }
            Self::TextureDepth2D => "texture_depth_2d()".to_owned(),
            Self::StorageTextureR32Write => {
                "texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly)"
                    .to_owned()
            }
            Self::Sampler { filtering: true } => {
                "sampler(SamplerBindingType::Filtering)".to_owned()
            }
            Self::Sampler { filtering: false } => {
                "sampler(SamplerBindingType::NonFiltering)".to_owned()
            }
        }
    }

    pub(super) const fn entry_param_ty(self) -> &'static str {
        match self {
            Self::UniformBuffer { .. } | Self::StorageBuffer { words: false, .. } => "&'a Buffer",
            Self::Words | Self::StorageBuffer { words: true, .. } => "&'a Words",
            Self::Texture2D { .. }
            | Self::Texture2DArray { .. }
            | Self::TextureDepth2D
            | Self::StorageTextureR32Write => "&'a TextureView",
            Self::Sampler { .. } => "&'a Sampler",
        }
    }

    pub(super) fn resource_expr(self, name: &str) -> String {
        match self {
            Self::UniformBuffer {
                size_expr,
                dynamic: true,
                ..
            } => format!(
                "BindingResource::Buffer(BufferBinding {{\n                    buffer: entries.{name},\n                    offset: 0,\n                    size: {size_expr},\n                }})"
            ),
            Self::UniformBuffer { dynamic: false, .. }
            | Self::StorageBuffer { words: false, .. } => {
                format!("entries.{name}.as_entire_binding()")
            }
            Self::Words | Self::StorageBuffer { words: true, .. } => {
                format!("entries.{name}.binding()")
            }
            Self::Texture2D { .. }
            | Self::Texture2DArray { .. }
            | Self::TextureDepth2D
            | Self::StorageTextureR32Write => {
                format!("BindingResource::TextureView(entries.{name})")
            }
            Self::Sampler { .. } => format!("BindingResource::Sampler(entries.{name})"),
        }
    }

    pub(super) fn wgsl_decl(
        self,
        group: u32,
        binding: u32,
        name: &str,
        cond: BindingCond,
    ) -> String {
        let prefix = match cond {
            BindingCond::FullOnly => "@if(!MINIMAL) ",
            BindingCond::Always => "",
        };
        match self {
            Self::UniformBuffer { wgsl_ty, .. } => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var<uniform> {name}: {wgsl_ty};\n"
                )
            }
            Self::StorageBuffer {
                read_only: true,
                wgsl_ty,
                ..
            } => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var<storage, read> {name}: {wgsl_ty};\n"
                )
            }
            Self::StorageBuffer {
                read_only: false,
                wgsl_ty,
                ..
            } => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var<storage, read_write> {name}: {wgsl_ty};\n"
                )
            }
            Self::Words => format!(
                "@if(MINIMAL) @group({group}) @binding({binding}) var {name}: texture_2d_array<u32>;\n\
                 @else @group({group}) @binding({binding}) var<storage, read> {name}: array<u32>;\n"
            ),
            Self::Texture2D { .. } => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var {name}: texture_2d<f32>;\n"
                )
            }
            Self::Texture2DArray { .. } => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var {name}: texture_2d_array<f32>;\n"
                )
            }
            Self::TextureDepth2D => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var {name}: texture_depth_2d;\n"
                )
            }
            Self::StorageTextureR32Write => {
                format!(
                    "{prefix}@group({group}) @binding({binding}) var {name}: texture_storage_2d<r32float, write>;\n"
                )
            }
            Self::Sampler { .. } => {
                format!("{prefix}@group({group}) @binding({binding}) var {name}: sampler;\n")
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum BindingCond {
    Always,
    FullOnly,
}

#[derive(Copy, Clone)]
pub(super) struct BindingDef {
    pub slot: u32,
    pub name: &'static str,
    pub const_name: &'static str,
    pub stages: Stages,
    pub kind: BindingKind,
    pub cond: BindingCond,
    pub variants: u8,
}

const fn bind(
    slot: u32,
    name: &'static str,
    const_name: &'static str,
    stages: Stages,
    kind: BindingKind,
) -> BindingDef {
    BindingDef {
        slot,
        name,
        const_name,
        stages,
        kind,
        cond: BindingCond::Always,
        variants: 0b11,
    }
}

pub(super) const TERRAIN_VIEW_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "params",
        "PARAMS",
        Stages::All,
        BindingKind::UniformBuffer {
            size_expr: "NonZeroU64::new(PARAMS_SIZE)",
            dynamic: true,
            wgsl_ty: "Params",
        },
    ),
    bind(
        1,
        "camera",
        "CAMERA",
        Stages::All,
        BindingKind::UniformBuffer {
            size_expr: "NonZeroU64::new(CAMERA_SIZE)",
            dynamic: false,
            wgsl_ty: "Camera",
        },
    ),
];

pub(super) const TERRAIN_DRAW_BINDINGS: &[BindingDef] = &[
    bind(0, "quads", "QUADS", Stages::Vertex, BindingKind::Words),
    bind(
        1,
        "vertices",
        "VERTICES",
        Stages::Vertex,
        BindingKind::Words,
    ),
    BindingDef {
        slot: 2,
        name: "visible",
        const_name: "VISIBLE",
        stages: Stages::Vertex,
        kind: BindingKind::StorageBuffer {
            read_only: true,
            wgsl_ty: "array<vec2<u32>>",
            words: false,
        },
        cond: BindingCond::FullOnly,
        variants: 0b11,
    },
    bind(
        3,
        "atlas0",
        "ATLAS0",
        Stages::Fragment,
        BindingKind::Texture2DArray { filterable: true },
    ),
    bind(
        4,
        "atlas1",
        "ATLAS1",
        Stages::Fragment,
        BindingKind::Texture2DArray { filterable: true },
    ),
    bind(
        5,
        "atlas2",
        "ATLAS2",
        Stages::Fragment,
        BindingKind::Texture2DArray { filterable: true },
    ),
    bind(
        6,
        "atlas3",
        "ATLAS3",
        Stages::Fragment,
        BindingKind::Texture2DArray { filterable: true },
    ),
    bind(
        7,
        "atlas_sampler",
        "ATLAS_SAMPLER",
        Stages::Fragment,
        BindingKind::Sampler { filtering: true },
    ),
    bind(
        8,
        "tints",
        "TINTS",
        Stages::Fragment,
        BindingKind::Texture2DArray { filterable: true },
    ),
    bind(
        9,
        "tint_sampler",
        "TINT_SAMPLER",
        Stages::Fragment,
        BindingKind::Sampler { filtering: true },
    ),
    bind(
        10,
        "animations",
        "ANIMATIONS",
        Stages::Fragment,
        BindingKind::Words,
    ),
    bind(
        11,
        "sprites",
        "SPRITES",
        Stages::Fragment,
        BindingKind::Words,
    ),
    bind(12, "faces", "FACES", Stages::Fragment, BindingKind::Words),
    bind(
        13,
        "sections",
        "SECTIONS",
        Stages::Vertex,
        BindingKind::Words,
    ),
    bind(
        14,
        "lightmap_levels",
        "LIGHTMAP",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: true },
    ),
    bind(
        15,
        "lightmap_sampler",
        "LIGHTMAP_SAMPLER",
        Stages::Fragment,
        BindingKind::Sampler { filtering: true },
    ),
];

pub(super) const CULL_ORDERED: u8 = 0b01;
pub(super) const CULL_QUAD: u8 = 0b10;

pub(super) const CULL_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "groups",
        "GROUPS",
        Stages::Compute,
        BindingKind::StorageBuffer {
            read_only: true,
            wgsl_ty: "array<Group>",
            words: true,
        },
    ),
    bind(
        1,
        "visible",
        "VISIBLE",
        Stages::Compute,
        BindingKind::StorageBuffer {
            read_only: false,
            wgsl_ty: "array<vec2<u32>>",
            words: false,
        },
    ),
    bind(
        2,
        "args",
        "ARGS",
        Stages::Compute,
        BindingKind::StorageBuffer {
            read_only: false,
            wgsl_ty: "array<DrawArgs>",
            words: false,
        },
    ),
    bind(
        3,
        "cave_visible",
        "CAVE_VISIBLE",
        Stages::Compute,
        BindingKind::StorageBuffer {
            read_only: false,
            wgsl_ty: "array<u32>",
            words: false,
        },
    ),
    bind(
        4,
        "sections",
        "SECTIONS",
        Stages::Compute,
        BindingKind::StorageBuffer {
            read_only: true,
            wgsl_ty: "array<SectionDesc>",
            words: true,
        },
    ),
    BindingDef {
        variants: CULL_ORDERED,
        ..bind(
            5,
            "batches",
            "BATCHES",
            Stages::Compute,
            BindingKind::StorageBuffer {
                read_only: false,
                wgsl_ty: "array<u32>",
                words: true,
            },
        )
    },
    bind(
        6,
        "hiz",
        "HIZ",
        Stages::Compute,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        7,
        "candidates",
        "CANDIDATES",
        Stages::Compute,
        BindingKind::StorageBuffer {
            read_only: false,
            wgsl_ty: "array<u32>",
            words: true,
        },
    ),
    BindingDef {
        variants: CULL_QUAD,
        ..bind(
            8,
            "quad_words",
            "QUAD_WORDS",
            Stages::Compute,
            BindingKind::StorageBuffer {
                read_only: true,
                wgsl_ty: "array<u32>",
                words: true,
            },
        )
    },
    BindingDef {
        variants: CULL_QUAD,
        ..bind(
            9,
            "model_words",
            "MODEL_WORDS",
            Stages::Compute,
            BindingKind::StorageBuffer {
                read_only: true,
                wgsl_ty: "array<u32>",
                words: true,
            },
        )
    },
];

pub(super) const GBUFFER_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "albedo_ao",
        "ALBEDO_AO",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        1,
        "normal_motion",
        "NORMAL_MOTION",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        2,
        "light",
        "LIGHT",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        3,
        "depth",
        "DEPTH",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        4,
        "reconstruction",
        "RECONSTRUCTION",
        Stages::Fragment,
        BindingKind::UniformBuffer {
            size_expr: "NonZeroU64::new(RECONSTRUCTION_SIZE)",
            dynamic: false,
            wgsl_ty: "Reconstruction",
        },
    ),
    bind(
        5,
        "nearest",
        "NEAREST",
        Stages::Fragment,
        BindingKind::Sampler { filtering: false },
    ),
];

pub(super) const TRANSPARENCY_BINDINGS: &[BindingDef] = &[
    bind(
        6,
        "sky_image",
        "SKY_IMAGE",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        7,
        "cloud_image",
        "CLOUD_IMAGE",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        8,
        "cloud_depth",
        "CLOUD_DEPTH",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        9,
        "cloud_nearest",
        "CLOUD_NEAREST",
        Stages::Fragment,
        BindingKind::Sampler { filtering: false },
    ),
];

pub(super) const SKY_VIEW_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "view",
        "VIEW",
        Stages::VertexFragment,
        BindingKind::UniformBuffer {
            size_expr: "Some(ViewUniform::min_size())",
            dynamic: true,
            wgsl_ty: "View",
        },
    ),
    bind(
        1,
        "sky",
        "SKY",
        Stages::VertexFragment,
        BindingKind::UniformBuffer {
            size_expr: "NonZeroU64::new(SKY_SIZE)",
            dynamic: false,
            wgsl_ty: "Sky",
        },
    ),
];

pub(super) const SKY_TEXTURES_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "celestials",
        "CELESTIALS",
        Stages::VertexFragment,
        BindingKind::Texture2DArray { filterable: true },
    ),
    bind(
        1,
        "celestial_sampler",
        "CELESTIAL_SAMPLER",
        Stages::VertexFragment,
        BindingKind::Sampler { filtering: true },
    ),
    bind(
        2,
        "clouds",
        "CLOUDS",
        Stages::VertexFragment,
        BindingKind::Texture2D { filterable: true },
    ),
];

pub(super) const HIZ_FROM_DEPTH: u8 = 0b01;
pub(super) const HIZ_DOWN: u8 = 0b10;

pub(super) const HIZ_BINDINGS: &[BindingDef] = &[
    BindingDef {
        variants: HIZ_FROM_DEPTH,
        ..bind(
            0,
            "depth",
            "DEPTH",
            Stages::Compute,
            BindingKind::TextureDepth2D,
        )
    },
    bind(
        1,
        "level_out",
        "LEVEL_OUT",
        Stages::Compute,
        BindingKind::StorageTextureR32Write,
    ),
    BindingDef {
        variants: HIZ_DOWN,
        ..bind(
            2,
            "level_in",
            "LEVEL_IN",
            Stages::Compute,
            BindingKind::Texture2D { filterable: false },
        )
    },
];

pub(super) const PRESENT_BINDINGS: &[BindingDef] = &[bind(
    0,
    "world",
    "WORLD",
    Stages::Fragment,
    BindingKind::Texture2D { filterable: false },
)];

pub(super) const SHOW_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "source",
        "SOURCE",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: false },
    ),
    bind(
        1,
        "nearest",
        "NEAREST",
        Stages::Fragment,
        BindingKind::Sampler { filtering: false },
    ),
];

pub(super) const WORLD_TEXT_VIEW_BINDINGS: &[BindingDef] = &[bind(
    0,
    "frame",
    "FRAME",
    Stages::Vertex,
    BindingKind::UniformBuffer {
        size_expr: "NonZeroU64::new(TEXT_FRAME_SIZE)",
        dynamic: false,
        wgsl_ty: "TextFrame",
    },
)];

pub(super) const WORLD_TEXT_TEXTURES_BINDINGS: &[BindingDef] = &[
    bind(
        0,
        "font_page",
        "FONT_PAGE",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: true },
    ),
    bind(
        1,
        "font_sampler",
        "FONT_SAMPLER",
        Stages::Fragment,
        BindingKind::Sampler { filtering: true },
    ),
    bind(
        2,
        "lightmap_levels",
        "LIGHTMAP_LEVELS",
        Stages::Fragment,
        BindingKind::Texture2D { filterable: true },
    ),
    bind(
        3,
        "lightmap_sampler",
        "LIGHTMAP_SAMPLER",
        Stages::Fragment,
        BindingKind::Sampler { filtering: true },
    ),
];
