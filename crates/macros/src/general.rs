use std::collections::HashMap;

use quote::ToTokens;
use syn::{ext::IdentExt, parse::Parse, spanned::Spanned, Ident, Token};

#[derive(Clone)]
pub struct Structure {
    pub attributes: Vec<syn::Attribute>,
    pub visibility: syn::Visibility,
    pub struct_token: Token![struct],
    pub name: syn::Ident,
    pub braces: syn::token::Brace,
    pub fields: syn::punctuated::Punctuated<syn::Field, Token![,]>,
}

impl Parse for Structure {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attributes: input.call(syn::Attribute::parse_outer)?,
            visibility: input.parse()?,
            struct_token: input.parse()?,
            name: input.parse()?,
            braces: syn::braced!(content in input),
            fields: content.parse_terminated(syn::Field::parse_named, Token![,])?,
        })
    }
}

pub struct AttributeInfo<S, F>
where
    S: Parse,
    F: Parse,
{
    pub struct_info: S,
    pub fields_info: HashMap<String, F>,
}

impl<S, F> AttributeInfo<S, F>
where
    S: Parse,
    F: Parse,
{
    pub(crate) fn parse_removal(
        structure: &mut Structure,
        attribute_name: &str,
    ) -> syn::Result<Self> {
        fn remove_suitable_attributes(
            attributes: &mut Vec<syn::Attribute>,
            attribute_name: &str,
        ) -> Option<syn::Attribute> {
            let index = attributes
                .iter()
                .enumerate()
                .find_map(|(i, attribute)| matches(attribute, attribute_name).then_some(i));

            index.map(|index| attributes.remove(index))
        }

        fn attribute_tokens(
            attribute: syn::Attribute,
            attribute_name: &str,
        ) -> syn::Result<proc_macro2::TokenStream> {
            let span = attribute.span();
            if let syn::Meta::List(meta_list) = attribute.meta {
                if let syn::MacroDelimiter::Paren(_) = &meta_list.delimiter {
                    Ok(meta_list.tokens)
                } else {
                    Err(syn::Error::new(
                        span,
                        "Expected parenthesis, not brackets or braces!",
                    ))
                }
            } else {
                Err(syn::Error::new(
                    span,
                    format!("Expected attribute like #[{attribute_name}()]"),
                ))
            }
        }

        let Some(outer_attribute) =
            remove_suitable_attributes(&mut structure.attributes, attribute_name)
        else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Expected #[{attribute_name}(name(StructName))] as outer attribute but it isn't provided"),
            ));
        };
        let struct_info = syn::parse2(attribute_tokens(outer_attribute, attribute_name)?)?;

        let mut fields_info = HashMap::new();
        for field in structure.fields.iter_mut() {
            let field_name = field_name(field);
            let Some(field_attribute) =
                remove_suitable_attributes(&mut field.attrs, attribute_name)
            else {
                continue;
            };

            fields_info.insert(
                field_name,
                syn::parse2(attribute_tokens(field_attribute, attribute_name)?)?,
            );
        }

        Ok(Self {
            struct_info,
            fields_info,
        })
    }
}

fn matches(attribute: &syn::Attribute, attribute_name: &str) -> bool {
    if let syn::Meta::List(meta_list) = &attribute.meta {
        if meta_list.path.is_ident(attribute_name) {
            return true;
        }
    }

    false
}
pub(crate) fn field_name(field: &syn::Field) -> String {
    field.expect_ident().to_string()
}

pub(crate) enum DefaultAssignment {
    DefaultCall,
    Expression(syn::Expr),
    FunctionCall(syn::Path),
}

impl Parse for DefaultAssignment {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(if input.peek(Ident::peek_any) && input.peek2(Token![=]) {
            let ident = input.parse::<syn::Ident>()?;
            if ident != "path" {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("Expected 'path' for function path, but given {ident}"),
                ));
            }

            let _ = input.parse::<Token![=]>()?;
            DefaultAssignment::FunctionCall(input.parse()?)
        } else {
            DefaultAssignment::Expression(input.parse()?)
        })
    }
}

pub struct DeriveInfo {
    ident: syn::Ident,
    paren: syn::token::Paren,
    traits: syn::punctuated::Punctuated<syn::Ident, Token![,]>,
}

impl DeriveInfo {
    pub fn from_ident_and_input(
        ident: syn::Ident,
        input: &syn::parse::ParseStream,
    ) -> syn::Result<Self> {
        let content;
        Ok(Self {
            ident,
            paren: syn::parenthesized!(content in input),
            traits: content.parse_terminated(syn::Ident::parse_any, Token![,])?,
        })
    }
}

impl ToTokens for DeriveInfo {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        proc_macro2::Punct::new('#', proc_macro2::Spacing::Joint).to_tokens(tokens);
        syn::token::Bracket::default().surround(tokens, |tokens| {
            self.ident.to_tokens(tokens);
            self.paren
                .surround(tokens, |tokens| self.traits.to_tokens(tokens));
        });
    }
}

pub(crate) trait ExpectIdent {
    fn expect_ident(&self) -> &syn::Ident;
}

impl ExpectIdent for syn::Field {
    fn expect_ident(&self) -> &syn::Ident {
        self.ident.as_ref().expect("Fields should be named!")
    }
}

pub(crate) fn wrap_by_option(ty: syn::Type) -> syn::Type {
    use proc_macro2::Span;
    use syn::PathSegment;

    syn::Type::Path(syn::TypePath {
        qself: None,
        path: syn::Path {
            leading_colon: None,
            segments: <syn::punctuated::Punctuated<PathSegment, Token![::]>>::from_iter(vec![
                PathSegment {
                    ident: Ident::new("std", Span::call_site()),
                    arguments: syn::PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("option", Span::call_site()),
                    arguments: syn::PathArguments::None,
                },
                PathSegment {
                    ident: Ident::new("Option", Span::call_site()),
                    arguments: syn::PathArguments::AngleBracketed(
                        syn::AngleBracketedGenericArguments {
                            colon2_token: None,
                            lt_token: Token![<](Span::call_site()),
                            args: syn::punctuated::Punctuated::from_iter(vec![
                                syn::GenericArgument::Type(ty),
                            ]),
                            gt_token: Token![>](Span::call_site()),
                        },
                    ),
                },
            ]),
        },
    })
}
