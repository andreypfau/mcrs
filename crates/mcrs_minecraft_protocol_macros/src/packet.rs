use heck::ToSnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Attribute, DeriveInput, Error, Expr, Result, parse_quote, parse2};

use crate::add_trait_bounds;

pub(super) fn derive_packet(item: TokenStream) -> Result<TokenStream> {
    let mut input = parse2::<DeriveInput>(item)?;

    let packet_attr = parse_packet_helper_attr(&input.attrs)?.unwrap_or_default();

    let name = input.ident.clone();

    let name_str = name.to_string();

    let Some(packet_id) = packet_attr.id else {
        return Err(Error::new(
            packet_attr.span,
            "missing `id = ...` value from `packet` attr",
        ));
    };

    add_trait_bounds(&mut input.generics, quote!(::std::fmt::Debug));

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let side = if let Some(side_attr) = packet_attr.side {
        side_attr
    } else if name_str.to_lowercase().starts_with("clientbound") {
        parse_quote!(::mcrs_minecraft_protocol::PacketSide::Clientbound)
    } else if name_str.to_lowercase().starts_with("serverbound") {
        parse_quote!(::mcrs_minecraft_protocol::PacketSide::Serverbound)
    } else {
        return Err(Error::new(
            packet_attr.span,
            format!(
                "missing `side = PacketSide::...` value from `packet` attribute, name: `{}`",
                name_str.to_lowercase()
            ),
        ));
    };

    let Some(state) = packet_attr.state else {
        return Err(Error::new(
            packet_attr.span,
            "missing `state = ...` value from `packet` attr",
        ));
    };

    let string_id = name_str.to_snake_case();
    let string_id = format!(
        "{}/{}",
        &string_id[..string_id.find('_').unwrap_or(string_id.len())],
        &string_id[string_id.find('_').map(|i| i + 1).unwrap_or(0)..]
    );

    Ok(quote! {
        impl #impl_generics ::mcrs_minecraft_protocol::__private::Packet for #name #ty_generics
        #where_clause
        {
            const ID: i32 = #packet_id;
            const NAME: &'static str = #string_id;
            const SIDE: ::mcrs_minecraft_protocol::PacketSide = #side;
            const STATE: ::mcrs_minecraft_protocol::ConnectionState = ::mcrs_minecraft_protocol::ConnectionState::#state;
        }
    })
}

struct PacketAttr {
    span: Span,
    id: Option<Expr>,
    side: Option<Expr>,
    state: Option<Expr>,
}

impl Default for PacketAttr {
    fn default() -> Self {
        Self {
            span: Span::call_site(),
            id: Default::default(),
            side: Default::default(),
            state: Default::default(),
        }
    }
}

fn parse_packet_helper_attr(attrs: &[Attribute]) -> Result<Option<PacketAttr>> {
    for attr in attrs {
        if attr.path().is_ident("packet") {
            let mut res = PacketAttr {
                span: attr.span(),
                id: None,
                side: None,
                state: None,
            };

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("id") {
                    res.id = Some(meta.value()?.parse::<Expr>()?);
                    Ok(())
                } else if meta.path.is_ident("side") {
                    res.side = Some(meta.value()?.parse::<Expr>()?);
                    Ok(())
                } else if meta.path.is_ident("state") {
                    res.state = Some(meta.value()?.parse::<Expr>()?);
                    Ok(())
                } else {
                    Err(meta.error("unrecognized packet argument"))
                }
            })?;

            return Ok(Some(res));
        }
    }

    Ok(None)
}
