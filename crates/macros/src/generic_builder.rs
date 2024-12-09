use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    braced, parenthesized, parse::Parse, parse_macro_input, punctuated::Punctuated,
    spanned::Spanned, GenericArgument, Ident, Token,
};

use crate::{
    general::{field_name, DefaultAssignment},
    propagate_err,
};

pub(super) fn make_derive(item: TokenStream) -> TokenStream {
    let mut structure = parse_macro_input!(item as Structure);
    let attr_info = propagate_err!(AttrInfo::parse_removal(&mut structure));

    let generic_builder = structure.create_generic_builder(&attr_info.struct_attr_info);

    let mut tokens = proc_macro2::TokenStream::new();
    generic_builder.build_gbuilder_struct(&mut tokens);

    propagate_err!(generic_builder.build_gbuilder_impl(&mut tokens, &attr_info, &structure));

    tokens.into()
}

#[derive(Clone)]
struct Structure {
    attributes: Vec<syn::Attribute>,
    visibility: syn::Visibility,
    struct_token: Token![struct],
    name: syn::Ident,
    braces: syn::token::Brace,
    fields: Punctuated<syn::Field, Token![,]>,
}

impl Parse for Structure {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            attributes: input.call(syn::Attribute::parse_outer)?,
            visibility: input.parse()?,
            struct_token: input.parse()?,
            name: input.parse()?,
            braces: braced!(content in input),
            fields: content.parse_terminated(syn::Field::parse_named, Token![,])?,
        })
    }
}

impl Structure {
    fn create_generic_builder(&self, struct_attr_info: &StructAttrInfo) -> Self {
        let mut generic_builder = self.clone();
        generic_builder.name = struct_attr_info.name.clone();
        generic_builder
    }

    fn build_gbuilder_struct(&self, tokens: &mut proc_macro2::TokenStream) {
        let Structure {
            ref visibility,
            ref struct_token,
            ref name,
            ref braces,
            ref fields,
            ..
        } = self;

        quote! {
            #visibility #struct_token #name
        }
        .to_tokens(tokens);

        braces.surround(tokens, |tokens| {
            let mut fields = fields.clone();
            fields.iter_mut().for_each(|field| {
                field.attrs.clear();
                field.ty = wrap_by_option(field.ty.clone())
            });
            fields.to_tokens(tokens)
        });
    }

    fn build_gbuilder_impl(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        attr_info: &AttrInfo,
        target_struct: &Structure,
    ) -> syn::Result<()> {
        let unhidden_fields = self
            .fields
            .iter()
            .filter(|field| !attr_info.is_hidden_field(field))
            .collect::<Punctuated<&syn::Field, Token![,]>>();

        let fn_new = self.build_fn_new();
        let fn_contains_field = self.build_fn_contains_field(&unhidden_fields)?;
        let fn_set_value = self.build_fn_set_value(&unhidden_fields);
        let fn_try_build = self.build_fn_try_build(target_struct, attr_info);

        let gbuilder_ident = &self.name;
        quote! {
            impl #gbuilder_ident {
                #fn_new
                #fn_contains_field
                #fn_set_value
                #fn_try_build
            }

            impl Default for #gbuilder_ident {
                fn default() -> Self {
                    Self::new()
                }
            }
        }
        .to_tokens(tokens);

        Ok(())
    }

    fn build_fn_new(&self) -> proc_macro2::TokenStream {
        let init_members: Punctuated<proc_macro2::TokenStream, Token![,]> = self
            .fields
            .iter()
            .map(|field| field.ident.clone().expect("Fields should be named"))
            .map(|field_ident| quote! { #field_ident: None })
            .collect();

        let visibility = &self.visibility;
        quote! {
            #visibility fn new() -> Self {
                Self {
                    #init_members
                }
            }
        }
    }

    fn build_fn_contains_field(
        &self,
        unhidden_fields: &Punctuated<&syn::Field, Token![,]>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let quoted_field_names: Punctuated<String, Token![,]> = unhidden_fields
            .iter()
            .map(|field| {
                field
                    .ident
                    .clone()
                    .expect("Fields should be named")
                    .to_string()
            })
            .collect();
        let field_count = quoted_field_names.len();

        let visibility = &self.visibility;
        Ok(quote! {
            const FIELD_NAMES: [&'static str; #field_count] = [
                #quoted_field_names
            ];

            #visibility fn contains_field(&self, field_name: &str) -> bool {
                Self::FIELD_NAMES.contains(&field_name)
            }
        })
    }

    fn build_fn_set_value(
        &self,
        unhidden_fields: &Punctuated<&syn::Field, Token![,]>,
    ) -> proc_macro2::TokenStream {
        let set_members: Punctuated<proc_macro2::TokenStream, Token![,]> = unhidden_fields
            .clone()
            .into_iter()
            .map(|field| {
                let field_ident = field.ident.clone().expect("Fields should be named");
                let field_name = field_ident.to_string();
                quote! { #field_name => self.#field_ident = Some(value.try_into()?) }
            })
            .collect();

        let err_expression = quote! {
            Err(shared::error::ConversionError::UnknownField { field_name: field_name.to_string() })
        };
        let function_body = if set_members.is_empty() {
            err_expression
        } else {
            quote! {
                match field_name {
                    #set_members,
                    _ => return #err_expression
                }
                Ok(self)
            }
        };

        let visibility = &self.visibility;
        quote! {
            #visibility fn set_value(
                &mut self,
                field_name: &str,
                value: shared::value::Value
            ) -> std::result::Result<&mut Self, shared::error::ConversionError> {
                #function_body
            }
        }
    }

    fn build_fn_try_build(
        &self,
        target_struct: &Structure,
        attr_info: &AttrInfo,
    ) -> proc_macro2::TokenStream {
        let target_type = &target_struct.name;

        let init_members: Punctuated<proc_macro2::TokenStream, Token![,]> = self
            .fields
            .clone()
            .into_iter()
            .map(|field| {
                let field_ident = field.ident.expect("Fields should be named");
                let field_name = field_ident.to_string();
                let mut line = quote! { #field_ident: self.#field_ident };

                if let Some(default_assignment) = attr_info
                    .field_attr_info
                    .get(&field_name)
                    .and_then(|field_attr| field_attr.default.as_ref())
                {
                    match default_assignment {
                        DefaultAssignment::Expression(expr) => {
                            quote! { .unwrap_or(#expr) }.to_tokens(&mut line)
                        }
                        DefaultAssignment::FunctionCall(function_path) => {
                            quote! { .unwrap_or(#function_path()) }.to_tokens(&mut line)
                        }
                        DefaultAssignment::DefaultCall => {
                            quote! { .unwrap_or_default() }.to_tokens(&mut line);
                        }
                    }
                } else {
                    quote! { .ok_or("The field '".to_string() + #field_name + "' should be set")? }
                        .to_tokens(&mut line);
                }

                line
            })
            .collect();

        let visibility = &self.visibility;
        quote! {
            #visibility fn try_build(self) -> std::result::Result<#target_type, String> {
                Ok(#target_type {
                    #init_members
                })
            }
        }
    }
}

struct AttrInfo {
    struct_attr_info: StructAttrInfo,
    field_attr_info: HashMap<String, FieldAttrInfo>,
}

impl AttrInfo {
    fn parse_removal(cfg_struct: &mut Structure) -> syn::Result<Self> {
        fn removed_suitable_attr(attributes: &mut Vec<syn::Attribute>) -> Option<syn::Attribute> {
            let index = attributes
                .iter()
                .enumerate()
                .find_map(|(i, attr)| AttrInfo::is_gbuilder(attr).then_some(i));

            index.map(|index| attributes.remove(index))
        }

        fn attr_tokens(attr: syn::Attribute) -> syn::Result<proc_macro2::TokenStream> {
            if let syn::Meta::List(meta_list) = attr.meta {
                Ok(meta_list.tokens)
            } else {
                Err(syn::Error::new(
                    attr.span(),
                    "Expected attribute like #[gbuilder()]",
                ))
            }
        }

        let Some(outer_attribute) = removed_suitable_attr(&mut cfg_struct.attributes) else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Expected #[gbuilder(name(StructName))] as outer attribute but it isn't provided",
            ));
        };
        let struct_attr_info = syn::parse2(attr_tokens(outer_attribute)?)?;

        let mut field_attr_info = HashMap::new();

        for field in cfg_struct.fields.iter_mut() {
            let field_name = field_name(field);
            let Some(field_attribute) = removed_suitable_attr(&mut field.attrs) else {
                continue;
            };

            field_attr_info.insert(field_name, syn::parse2(attr_tokens(field_attribute)?)?);
        }

        Ok(Self {
            struct_attr_info,
            field_attr_info,
        })
    }

    fn is_hidden_field(&self, field: &syn::Field) -> bool {
        self.field_attr_info
            .get(&field_name(field))
            .map(|field_info| field_info.hidden)
            .unwrap_or(false)
    }

    fn is_gbuilder(attr: &syn::Attribute) -> bool {
        if let syn::Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("gbuilder") {
                return true;
            }
        }

        false
    }
}

struct StructAttrInfo {
    name: syn::Ident,
}

impl Parse for StructAttrInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let beginning_span = input.span();
        let mut name;

        loop {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "name" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    name = Some(content.parse()?);
                }
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            } else {
                break;
            }
        }

        let Some(name) = name else {
            return Err(syn::Error::new(
                beginning_span,
                "Expected \"name\" of generic builder for creating new struct, but it doesn't given",
            ));
        };

        Ok(Self { name })
    }
}

struct FieldAttrInfo {
    hidden: bool,
    default: Option<DefaultAssignment>,
}

impl Parse for FieldAttrInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut hidden = false;
        let mut default = None;

        loop {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "hidden" => hidden = true,
                "default" => {
                    if input.peek(syn::token::Paren) {
                        let content;
                        let _paren = parenthesized!(content in input);
                        default = Some(content.parse()?);
                    } else {
                        default = Some(DefaultAssignment::DefaultCall)
                    }
                }
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            } else {
                break;
            }
        }

        Ok(Self { default, hidden })
    }
}

fn wrap_by_option(ty: syn::Type) -> syn::Type {
    use proc_macro2::Span;
    use syn::PathSegment;

    syn::Type::Path(syn::TypePath {
        qself: None,
        path: syn::Path {
            leading_colon: None,
            segments: <Punctuated<PathSegment, Token![::]>>::from_iter(vec![
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
                            args: Punctuated::from_iter(vec![GenericArgument::Type(ty)]),
                            gt_token: Token![>](Span::call_site()),
                        },
                    ),
                },
            ]),
        },
    })
}
