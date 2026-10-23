use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{Data, DeriveInput, Error, Ident, LitStr, Path, Result, WherePredicate, parse_quote};

use crate::{has_serde_bound, is_refs, serde_bound};

struct Args {
    registry: Path,
    key: LitStr,
    unsupported: Vec<Ident>,
    only: bool,
    key_last: bool,
}

fn args(tokens: proc_macro::TokenStream) -> Result<Args> {
    let mut registry = None;
    let mut key = None;
    let mut unsupported = Vec::new();
    let mut only = false;
    let mut key_last = false;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("registry") {
            registry = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("key") {
            key = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("unsupported") {
            meta.parse_nested_meta(|entry| {
                unsupported.push(entry.path.require_ident()?.clone());
                Ok(())
            })?;
        } else if meta.path.is_ident("only") {
            only = true;
        } else if meta.path.is_ident("key_last") {
            key_last = true;
        } else {
            return Err(
                meta.error("expected `registry`, `key`, `unsupported`, `only` or `key_last`")
            );
        }
        Ok(())
    });
    syn::parse::Parser::parse(parser, tokens)?;
    Ok(Args {
        registry: registry
            .ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "`registry` is required"))?,
        key: key.unwrap_or_else(|| LitStr::new("type", proc_macro2::Span::call_site())),
        unsupported,
        only,
        key_last,
    })
}

enum Target {
    Entry(Ident),
    Id(LitStr),
}

fn target(variant: &mut syn::Variant) -> Result<Target> {
    let mut target = Target::Entry(variant.ident.clone());
    let mut kept = Vec::with_capacity(variant.attrs.len());
    for attr in variant.attrs.drain(..) {
        if !attr.path().is_ident("dispatch") {
            kept.push(attr);
            continue;
        }
        attr.parse_nested_meta(|meta| {
            target = match meta.path.is_ident("id") {
                true => Target::Id(meta.value()?.parse()?),
                false => Target::Entry(meta.path.require_ident()?.clone()),
            };
            Ok(())
        })?;
    }
    variant.attrs = kept;
    Ok(target)
}

pub(crate) fn dispatch(
    args_tokens: proc_macro::TokenStream,
    mut input: DeriveInput,
) -> Result<Tokens> {
    let args = args(args_tokens)?;
    let Data::Enum(data) = &mut input.data else {
        return Err(Error::new_spanned(
            &input.ident,
            "a dispatched value is an enum",
        ));
    };
    let mut variants = Vec::with_capacity(data.variants.len());
    for variant in &mut data.variants {
        variants.push((variant.ident.clone(), target(variant)?));
    }

    let krate = quote!(::mcrs_minecraft_registries);
    let serde = quote!(::serde);
    // A registry value is read and written in its named form only. Bounds over its mode would
    // recurse through a field that holds the value itself.
    let named = input
        .generics
        .type_params()
        .any(|param| param.bounds.iter().any(is_refs));
    derive_body_codec(&mut input, named);

    let Args {
        registry,
        key,
        key_last,
        ..
    } = &args;
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let de = param_bounds(&input, quote!(::serde::Deserialize<'de>));
    let ser = param_bounds(&input, quote!(::serde::Serialize));
    let (codec_generics, codec_type) = match named {
        true => (quote!(), quote!(#name<#krate::Named>)),
        false => (quote!(#impl_generics), quote!(#name #ty_generics)),
    };
    let predicates = |extra: &[WherePredicate]| {
        if named {
            return quote!();
        }
        let own = where_clause.map(|clause| clause.predicates.iter().cloned().collect::<Vec<_>>());
        let all: Vec<WherePredicate> = own
            .into_iter()
            .flatten()
            .chain(extra.iter().cloned())
            .collect();
        quote!(where #(#all,)*)
    };

    let pick = pick(&args, &variants);
    let body = body(&codec_type, named || input.generics.params.is_empty());
    let de_generics = de_generics(input.generics.clone(), named);
    let de_where = predicates(&de);
    let deserialize = quote! {
        #[automatically_derived]
        impl #de_generics #serde::Deserialize<'de> for #codec_type #de_where {
            fn deserialize<__D: #serde::Deserializer<'de>>(
                deserializer: __D,
            ) -> ::core::result::Result<Self, __D::Error> {
                #pick
                #krate::dispatch::read(
                    deserializer,
                    #key,
                    <#registry as #krate::dispatch::__core::Registered>::REGISTRY
                        .location()
                        .as_static_str(),
                    pick,
                    #body,
                )
            }
        }
    };

    let ser_where = predicates(&ser);
    let serialize = {
        let location = location(registry, &variants);
        quote! {
            #[automatically_derived]
            impl #codec_generics #serde::Serialize for #codec_type #ser_where {
                fn serialize<__S: #serde::Serializer>(
                    &self,
                    serializer: __S,
                ) -> ::core::result::Result<__S::Ok, __S::Error> {
                    let location: &'static str = #location;
                    <#codec_type>::serialize(
                        self,
                        #krate::dispatch::Tagging::new(
                            serializer,
                            #krate::dispatch::Tag {
                                key: #key,
                                location,
                                last: #key_last,
                            },
                        ),
                    )
                }
            }
        }
    };

    let kind = kind(&input, registry, &variants);
    Ok(quote! {
        #input
        #deserialize
        #serialize
        #kind
    })
}

fn derive_body_codec(input: &mut DeriveInput, named: bool) {
    if named && !has_serde_bound(&input.attrs) {
        let params: Vec<&Ident> = input.generics.type_params().map(|p| &p.ident).collect();
        let bound = serde_bound(&input.data, &params);
        input.attrs.push(bound);
    }
    input.attrs.insert(
        0,
        parse_quote!(#[derive(::serde::Serialize, ::serde::Deserialize)]),
    );
    input.attrs.push(parse_quote!(#[serde(remote = "Self")]));
}

fn pick(args: &Args, variants: &[(Ident, Target)]) -> Tokens {
    let krate = quote!(::mcrs_minecraft_registries);
    let Args {
        registry,
        unsupported,
        only,
        ..
    } = args;
    let entry_arms = variants
        .iter()
        .filter_map(|(variant, target)| match target {
            Target::Entry(entry) => {
                let label = variant.to_string();
                Some(quote!(<#registry>::#entry => Pick::Variant(#label),))
            }
            Target::Id(_) => None,
        });
    let id_arms = variants
        .iter()
        .filter_map(|(variant, target)| match target {
            Target::Id(id) => {
                let label = variant.to_string();
                Some(quote!(#id => Pick::Variant(#label),))
            }
            Target::Entry(_) => None,
        });
    let rest = only.then(|| quote!(_ => Pick::Unsupported,));
    quote! {
        fn pick(location: &str) -> #krate::dispatch::Pick {
            use #krate::dispatch::Pick;
            match <#registry>::find(location) {
                ::core::option::Option::Some(entry) => match entry {
                    #(#entry_arms)*
                    #(<#registry>::#unsupported => Pick::Unsupported,)*
                    #rest
                },
                ::core::option::Option::None => match location {
                    #(#id_arms)*
                    _ => Pick::Unknown,
                },
            }
        }
    }
}

fn kind(input: &DeriveInput, registry: &Path, variants: &[(Ident, Target)]) -> Option<Tokens> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let entries: Option<Vec<(&Ident, &Ident)>> = variants
        .iter()
        .map(|(variant, target)| match target {
            Target::Entry(entry) => Some((variant, entry)),
            Target::Id(_) => None,
        })
        .collect();
    entries.map(|entries| {
        let kinds = entries.iter().map(|(_, entry)| quote!(<#registry>::#entry));
        let arms = entries
            .iter()
            .map(|(variant, entry)| quote!(Self::#variant { .. } => <#registry>::#entry,));
        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                pub const KINDS: &'static [#registry] = &[#(#kinds),*];

                pub fn kind(&self) -> #registry {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    })
}

fn location(registry: &Path, variants: &[(Ident, Target)]) -> Tokens {
    let arms = variants.iter().map(|(variant, target)| match target {
        Target::Entry(entry) => {
            quote!(Self::#variant { .. } => <#registry>::#entry.as_static_str(),)
        }
        Target::Id(id) => quote!(Self::#variant { .. } => #id,),
    });
    quote!(match self { #(#arms)* })
}

fn param_bounds(input: &DeriveInput, of: Tokens) -> Vec<WherePredicate> {
    let params = input.generics.type_params().map(|param| &param.ident);
    params.map(|param| parse_quote!(#param: #of)).collect()
}

fn body(codec_type: &Tokens, monomorphic: bool) -> Tokens {
    let krate = quote!(::mcrs_minecraft_registries);
    match monomorphic {
        true => quote! {{
            fn body(
                chosen: #krate::dispatch::Chosen<'_>,
            ) -> ::core::result::Result<#codec_type, #krate::dispatch::Error> {
                <#codec_type>::deserialize(chosen)
            }
            body
        }},
        false => quote!(|chosen| <#codec_type>::deserialize(chosen)),
    }
}

fn de_generics(mut generics: syn::Generics, named: bool) -> Tokens {
    if named {
        return quote!(<'de>);
    }
    generics.params.insert(0, parse_quote!('de));
    let (impl_generics, _, _) = generics.split_for_impl();
    quote!(#impl_generics)
}
