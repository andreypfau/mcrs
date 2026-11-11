use proc_macro2::{Ident, TokenStream};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, Field, Fields, GenericParam, Lifetime, Result, Type,
};

/// A derive input reduced to what the Bedrock codecs support.
pub(crate) enum Shape {
    Struct(StructShape),
    Enum(EnumShape),
}

pub(crate) struct StructShape {
    pub(crate) name: Ident,
    pub(crate) lifetime: Option<Lifetime>,
    pub(crate) id: Option<Expr>,
    pub(crate) fields: Vec<FieldShape>,
}

pub(crate) struct FieldShape {
    pub(crate) name: Ident,
    pub(crate) ty: Type,
    pub(crate) count: Option<Type>,
}

pub(crate) struct EnumShape {
    pub(crate) name: Ident,
    pub(crate) tag: Type,
    pub(crate) variants: Vec<Ident>,
}

#[derive(Default)]
struct ContainerArgs {
    id: Option<Expr>,
    tag: Option<Type>,
}

impl Shape {
    pub(crate) fn parse(item: TokenStream) -> Result<Self> {
        let input: DeriveInput = syn::parse2(item)?;
        let lifetime = single_lifetime(&input)?;
        let args = container_args(&input.attrs)?;
        match input.data {
            Data::Struct(data) => {
                if let Some(tag) = args.tag {
                    return Err(Error::new(tag.span(), "`tag` applies to an enum only"));
                }
                let Fields::Named(named) = data.fields else {
                    return Err(Error::new(
                        data.fields.span(),
                        "a Bedrock codec struct needs named fields",
                    ));
                };
                let fields = named
                    .named
                    .into_iter()
                    .map(field_shape)
                    .collect::<Result<Vec<FieldShape>>>()?;
                Ok(Self::Struct(StructShape {
                    name: input.ident,
                    lifetime,
                    id: args.id,
                    fields,
                }))
            }
            Data::Enum(data) => {
                if let Some(id) = args.id {
                    return Err(Error::new(id.span(), "`id` applies to a struct only"));
                }
                if let Some(lifetime) = lifetime {
                    return Err(Error::new(
                        lifetime.span(),
                        "a tagged enum takes no lifetime",
                    ));
                }
                let Some(tag) = args.tag else {
                    return Err(Error::new(
                        input.ident.span(),
                        "a Bedrock codec enum needs `#[bedrock(tag = <type>)]`",
                    ));
                };
                let mut variants = Vec::with_capacity(data.variants.len());
                for variant in data.variants {
                    if !matches!(variant.fields, Fields::Unit) {
                        return Err(Error::new(
                            variant.fields.span(),
                            "a Bedrock codec enum has fieldless variants only",
                        ));
                    }
                    variants.push(variant.ident);
                }
                Ok(Self::Enum(EnumShape {
                    name: input.ident,
                    tag,
                    variants,
                }))
            }
            Data::Union(data) => Err(Error::new(
                data.union_token.span(),
                "a union has no Bedrock codec",
            )),
        }
    }
}

fn single_lifetime(input: &DeriveInput) -> Result<Option<Lifetime>> {
    let mut lifetime = None;
    for param in &input.generics.params {
        match param {
            GenericParam::Lifetime(param) if lifetime.is_none() => {
                lifetime = Some(param.lifetime.clone());
            }
            _ => {
                return Err(Error::new(
                    param.span(),
                    "a Bedrock codec type takes at most one lifetime and no other parameter",
                ));
            }
        }
    }
    if let Some(where_clause) = &input.generics.where_clause {
        return Err(Error::new(
            where_clause.span(),
            "a Bedrock codec type takes no where clause",
        ));
    }
    Ok(lifetime)
}

fn container_args(attrs: &[Attribute]) -> Result<ContainerArgs> {
    let mut args = ContainerArgs::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("bedrock")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                args.id = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("tag") {
                args.tag = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("expected `id` or `tag`"))
            }
        })?;
    }
    Ok(args)
}

fn field_shape(field: Field) -> Result<FieldShape> {
    let mut count = None;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("bedrock"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("count") {
                count = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("expected `count`"))
            }
        })?;
    }
    let span = field.span();
    let Some(name) = field.ident else {
        return Err(Error::new(span, "a Bedrock codec field needs a name"));
    };
    Ok(FieldShape {
        name,
        ty: field.ty,
        count,
    })
}
