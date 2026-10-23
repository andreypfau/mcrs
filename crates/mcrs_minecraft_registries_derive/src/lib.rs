mod dispatch;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{ToTokens, format_ident, quote};
use syn::meta::ParseNestedMeta;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{
    Attribute, Data, DeriveInput, Error, Expr, Field, Fields, GenericParam, Ident, LitStr, Result,
    Token, Type, TypeParamBound, parse_macro_input, parse_quote,
};

/// Put it above the value's `#[derive]`, which must still see the serde bound it adds.
#[proc_macro_attribute]
pub fn registry_value(args: TokenStream, item: TokenStream) -> TokenStream {
    parse_macro_input!(args as syn::parse::Nothing);
    let mut input = parse_macro_input!(item as DeriveInput);
    match bound_serde(&mut input) {
        Ok(()) => input.into_token_stream().into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Reads and writes an enum as one map whose `key` names an entry of the static `registry`, the
/// way `Codec.dispatch` does: the entry chooses the variant, the rest of the map is its body.
/// A variant reads the entry of its own name, `#[dispatch(Entry)]` another one and
/// `#[dispatch(id = "ns:path")]` a name outside the registry. Every entry has a variant or is
/// listed in `unsupported(..)`, unless `only` refuses the entries left out; `key_last` writes the
/// key after the body.
#[proc_macro_attribute]
pub fn dispatch(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    dispatch::dispatch(args, input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Resolve)]
pub fn derive_resolve(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    resolve(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn has_serde_bound(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
        .any(|attr| {
            let mut bound = false;
            let _ = attr.parse_nested_meta(|meta| {
                bound |= meta.path.is_ident("bound");
                skip_meta_value(&meta)
            });
            bound
        })
}

fn skip_meta_value(meta: &ParseNestedMeta) -> Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<Expr>()?;
    } else if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(|inner| {
            if inner.input.peek(Token![=]) {
                inner.value()?.parse::<Expr>()?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn bound_serde(input: &mut DeriveInput) -> Result<()> {
    let mode = mode_param(&input.generics)?;
    if !has_serde_bound(&input.attrs) {
        let bound = serde_bound(&input.data, &[&mode]);
        // A helper attribute placed before the derive that introduces it is rejected.
        let after_derives = input
            .attrs
            .iter()
            .rposition(|attr| attr.path().is_ident("derive"))
            .map_or(0, |at| at + 1);
        input.attrs.insert(after_derives, bound);
    }
    input.attrs.insert(
        0,
        parse_quote!(#[derive(::mcrs_minecraft_registries::Resolve)]),
    );
    Ok(())
}

fn serde_bound(data: &Data, params: &[&Ident]) -> Attribute {
    let mut types: Vec<String> = Vec::new();
    for field in all_fields(data) {
        if mentions_any(&field.ty, params) {
            let ty = field.ty.to_token_stream().to_string();
            if !types.contains(&ty) {
                types.push(ty);
            }
        }
    }
    let bounds = |of: &str| {
        let bounds: Vec<String> = types.iter().map(|ty| format!("{ty}: {of}")).collect();
        LitStr::new(&bounds.join(", "), Span::call_site())
    };
    let serialize = bounds("::serde::Serialize");
    let deserialize = bounds("::serde::Deserialize<'de>");
    parse_quote!(#[serde(bound(serialize = #serialize, deserialize = #deserialize))])
}

fn all_fields(data: &Data) -> Vec<&Field> {
    match data {
        Data::Struct(data) => data.fields.iter().collect(),
        Data::Enum(data) => data.variants.iter().flat_map(|v| v.fields.iter()).collect(),
        Data::Union(_) => Vec::new(),
    }
}

fn is_refs(bound: &TypeParamBound) -> bool {
    matches!(bound, TypeParamBound::Trait(bound)
        if bound.path.segments.last().is_some_and(|segment| segment.ident == "Refs"))
}

fn mode_param(generics: &syn::Generics) -> Result<Ident> {
    let mut mode = None;
    for param in &generics.params {
        match param {
            GenericParam::Type(param) if param.bounds.iter().any(is_refs) && mode.is_none() => {
                mode = Some(param.ident.clone());
            }
            other => {
                return Err(Error::new_spanned(
                    other,
                    "a registry value takes one generic parameter, its mode `M: Refs`",
                ));
            }
        }
    }
    mode.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "a registry value takes a mode parameter `M: Refs`",
        )
    })
}

struct Mentions<'a> {
    params: &'a [&'a Ident],
    found: bool,
}

impl Visit<'_> for Mentions<'_> {
    fn visit_ident(&mut self, ident: &Ident) {
        self.found |= self.params.contains(&ident);
    }
}

fn mentions_any(ty: &Type, params: &[&Ident]) -> bool {
    let mut visit = Mentions {
        params,
        found: false,
    };
    visit.visit_type(ty);
    visit.found
}

struct Substitute<'a> {
    mode: &'a Ident,
    with: Type,
}

impl VisitMut for Substitute<'_> {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(path) = ty
            && path.qself.is_none()
            && path.path.leading_colon.is_none()
            && let Some(first) = path.path.segments.first()
            && first.ident == *self.mode
            && first.arguments.is_none()
        {
            let rest: Punctuated<syn::PathSegment, Token![::]> =
                path.path.segments.iter().skip(1).cloned().collect();
            let with = &self.with;
            *ty = match rest.is_empty() {
                true => with.clone(),
                false => parse_quote!(<#with as ::mcrs_minecraft_registries::Refs>::#rest),
            };
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
}

fn substitute(ty: &Type, mode: &Ident, with: &Type) -> Type {
    let mut ty = ty.clone();
    Substitute {
        mode,
        with: with.clone(),
    }
    .visit_type_mut(&mut ty);
    ty
}

fn renamed(field: &Field) -> Option<String> {
    let mut rename = None;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
    {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") && meta.input.peek(Token![=]) {
                rename = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else {
                skip_meta_value(&meta)
            }
        });
    }
    rename
}

fn label(field: &Field, at: usize) -> String {
    renamed(field).unwrap_or_else(|| match &field.ident {
        Some(ident) => ident.to_string(),
        None => at.to_string(),
    })
}

struct Conversion {
    pattern: Tokens,
    resolved: Tokens,
    named: Tokens,
}

fn convert(fields: &Fields, mode: &Ident, resolved_mode: &Type) -> Conversion {
    let krate = quote!(::mcrs_minecraft_registries);
    let mut patterns = Vec::new();
    let mut resolved = Vec::new();
    let mut named = Vec::new();
    for (at, field) in fields.iter().enumerate() {
        let binding = format_ident!("__field{at}");
        let label = label(field, at);
        let (to_resolved, to_named) = match mentions_any(&field.ty, &[mode]) {
            true => {
                let ty = substitute(&field.ty, mode, resolved_mode);
                (
                    quote!(<#ty as #krate::Resolve>::resolve(#binding, ctx).map_err(|e| e.at(#label))?),
                    quote!(<#ty as #krate::Resolve>::name(#binding, names).map_err(|e| e.at(#label))?),
                )
            }
            false => (
                quote!(#binding),
                quote!(::core::clone::Clone::clone(#binding)),
            ),
        };
        match &field.ident {
            Some(ident) => {
                patterns.push(quote!(#ident: #binding));
                resolved.push(quote!(#ident: #to_resolved));
                named.push(quote!(#ident: #to_named));
            }
            None => {
                patterns.push(quote!(#binding));
                resolved.push(to_resolved);
                named.push(to_named);
            }
        }
    }
    let wrap = |items: Vec<Tokens>| match fields {
        Fields::Named(_) => quote!({ #(#items),* }),
        Fields::Unnamed(_) => quote!(( #(#items),* )),
        Fields::Unit => quote!(),
    };
    Conversion {
        pattern: wrap(patterns),
        resolved: wrap(resolved),
        named: wrap(named),
    }
}

fn resolve(input: &DeriveInput) -> Result<Tokens> {
    let mode = mode_param(&input.generics)?;
    let krate = quote!(::mcrs_minecraft_registries);
    let resolved_mode: Type = parse_quote!(#krate::Resolved);
    let name = &input.ident;
    let resolved_ty = quote!(#name<#krate::Resolved>);
    let named_ty = quote!(#name<#krate::Named>);
    let mut resolve_arms = Vec::new();
    let mut name_arms = Vec::new();
    match &input.data {
        Data::Struct(data) => {
            let c = convert(&data.fields, &mode, &resolved_mode);
            let (pattern, resolved, named) = (&c.pattern, &c.resolved, &c.named);
            resolve_arms.push(
                quote!(#name::<#krate::Named> #pattern => #name::<#krate::Resolved> #resolved),
            );
            name_arms
                .push(quote!(#name::<#krate::Resolved> #pattern => #name::<#krate::Named> #named));
        }
        Data::Enum(data) => {
            for variant in &data.variants {
                let v = &variant.ident;
                let c = convert(&variant.fields, &mode, &resolved_mode);
                let (pattern, resolved, named) = (&c.pattern, &c.resolved, &c.named);
                resolve_arms.push(quote!(#name::<#krate::Named>::#v #pattern => #name::<#krate::Resolved>::#v #resolved));
                name_arms.push(quote!(#name::<#krate::Resolved>::#v #pattern => #name::<#krate::Named>::#v #named));
            }
        }
        Data::Union(_) => {
            return Err(Error::new_spanned(
                input,
                "a registry value is a struct or an enum",
            ));
        }
    }
    Ok(quote! {
        #[automatically_derived]
        impl #krate::Resolve for #resolved_ty {
            type Named = #named_ty;

            fn resolve<C: #krate::Resolver>(
                named: #named_ty,
                ctx: &C,
            ) -> ::core::result::Result<Self, #krate::ResolveError> {
                let _ = ctx;
                ::core::result::Result::Ok(match named { #(#resolve_arms,)* })
            }

            fn name<C: #krate::Namer>(
                &self,
                names: &C,
            ) -> ::core::result::Result<#named_ty, #krate::ResolveError> {
                let _ = names;
                ::core::result::Result::Ok(match self { #(#name_arms,)* })
            }
        }
    })
}
