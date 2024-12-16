use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::{
    ext::IdentExt, parenthesized, parse::Parse, parse_macro_input, punctuated::Punctuated,
    spanned::Spanned, Token,
};

use crate::{
    general::{field_name, wrap_by_option, AttributeInfo, DefaultAssignment, Structure},
    propagate_err,
};

pub(super) fn make_derive(item: TokenStream) -> TokenStream {
    let mut config_struct = parse_macro_input!(item as Structure);
    let attribute_info =
        propagate_err!(AttributeInfo::parse_removal(&mut config_struct, "cfg_prop"));

    propagate_err!(config_struct.validate_attributes(&attribute_info));

    let mut property_struct = config_struct.create_property_struct(&attribute_info.struct_info);
    property_struct.update_field_types(&attribute_info);
    property_struct.extend_by_temporary_fields(&attribute_info);

    let mut tokens = proc_macro2::TokenStream::new();

    property_struct.build_struct(&mut tokens, &attribute_info);
    propagate_err!(property_struct.build_impl(&mut tokens, &config_struct.name, &attribute_info));

    tokens.into()
}

impl Structure {
    fn validate_attributes(
        &self,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<()> {
        let mut temporary_field_types = std::collections::HashMap::new();

        for (attached_field_name, field_info) in &attribute_info.fields_info {
            let Some(also_from_field) = &field_info.also_from_field else {
                continue;
            };

            let field = self
                .fields
                .iter()
                .find(|field| &field_name(field) == attached_field_name)
                .expect("Should be at least one field that have attibutes!");

            let r#type = temporary_field_types
                .entry(also_from_field.ident.to_string())
                .or_insert(&field.ty);

            if *r#type != &field.ty {
                return Err(syn::Error::new(
                    field.ty.span(),
                    format!(
                        "Expected the same types of fields with same attribute value 'also_from' - {}!", 
                        also_from_field.ident.to_string()
                    )
                ));
            }
        }

        Ok(())
    }

    fn create_property_struct(&self, struct_info: &StructInfo) -> Self {
        let mut new_type = self.clone();
        new_type.name = struct_info.name.clone();
        new_type
    }

    fn update_field_types(&mut self, attribute_info: &AttributeInfo<StructInfo, FieldInfo>) {
        for field in self.fields.iter_mut() {
            let Some(field_info) = attribute_info.fields_info.get(&field_name(field)) else {
                continue;
            };

            let Some(use_type) = field_info.use_type.clone() else {
                continue;
            };

            field.ty = syn::Type::Path(syn::TypePath {
                qself: None,
                path: use_type,
            });
        }
    }

    fn extend_by_temporary_fields(
        &mut self,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) {
        let mut temporary_fields = std::collections::HashMap::new();

        for (attached_field_name, field_info) in &attribute_info.fields_info {
            let Some(also_from_field) = &field_info.also_from_field else {
                continue;
            };

            let field = self
                .fields
                .iter()
                .find(|field| &field_name(field) == attached_field_name)
                .expect("Should be at least one field that have attibutes!");

            if !temporary_fields.contains_key(&also_from_field.ident) {
                temporary_fields.insert(also_from_field.ident.clone(), field.clone());
            }
        }

        for (field_ident, mut field) in temporary_fields {
            field.ident.replace(field_ident);
            field.attrs.clear();
            self.fields.push(field);
        }
    }

    fn build_struct(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) {
        let Structure {
            visibility,
            struct_token,
            name,
            braces,
            fields,
            ..
        } = self;

        let derive_info = attribute_info
            .struct_info
            .derive_info
            .as_ref()
            .map(|derive_info| derive_info.to_token_stream())
            .unwrap_or_default();
        quote! {
            #derive_info
            #visibility #struct_token #name
        }
        .to_tokens(tokens);

        braces.surround(tokens, |tokens| {
            let mut fields = fields.clone();
            fields = fields
                .into_iter()
                .map(|mut field| {
                    field.attrs.clear();
                    field.ty = wrap_by_option(field.ty);
                    field
                })
                .collect();
            fields.to_tokens(tokens)
        });
    }

    fn build_impl(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        target_type: &syn::Ident,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<()> {
        let fn_merge = self.build_fn_merge(attribute_info)?;
        let fn_unwrap_or_default = self.build_fn_unwrap_or_default(target_type, attribute_info)?;

        let impl_from_to_target = self.build_impl_from_to_target(target_type);

        let ident = &self.name;
        quote! {
            impl #ident {
                #fn_merge
                #fn_unwrap_or_default
            }

            #impl_from_to_target
        }
        .to_tokens(tokens);

        Ok(())
    }

    fn build_fn_merge(
        &self,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let field_idents: Punctuated<&syn::Ident, Token![,]> = self
            .fields
            .iter()
            .map(|field| field.ident.as_ref().expect("Must be a name field!"))
            .collect();

        let init_members: Punctuated<proc_macro2::TokenStream, Token![,]> = self
            .fields
            .iter()
            .map(|field| {
                let ident = field.ident.as_ref().expect("Must be a named field");
                let mut line = quote! { #ident: #ident };

                if attribute_info.is_mergeable_field(field) {
                    line = quote! { #line.map(|#ident| #ident.merge(other.#ident.clone())) };
                }

                quote! { #line.or(other.#ident) }
            })
            .collect();

        Ok(quote! {
            pub fn merge(self, other: Option<Self>) -> Self {
                let Some(other) = other else {
                    return self;
                };

                let Self { #field_idents } = self;
                Self {
                    #init_members
                }
            }
        })
    }

    fn build_fn_unwrap_or_default(
        &self,
        target_type: &syn::Ident,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let field_idents: Punctuated<&syn::Ident, Token![,]> = self
            .fields
            .iter()
            .map(|field| field.ident.as_ref().expect("Must be a name field!"))
            .collect();

        let init_members: Punctuated<proc_macro2::TokenStream, Token![,]> = self
            .fields
            .iter()
            .filter(|field| !attribute_info.is_temporary_field(field))
            .map(|field| {
                let ident = field.ident.as_ref().expect("Must be a named field");
                let mut line = quote! { #ident: #ident };

                if let Some(field_info) = attribute_info.fields_info.get(&field_name(&field)) {
                    if let Some(AlsoFromField {
                        ident: temporary_field_ident,
                        mergeable,
                    }) = &field_info.also_from_field
                    {
                        if *mergeable {
                            line = quote! { #line.map(|val| val.merge(#temporary_field_ident.clone())) };
                        }
                        line = quote! { #line.or(#temporary_field_ident.clone()) }
                    }

                    match &field_info.default {
                        DefaultAssignment::Expression(expr) => {
                            line = quote! { #line.unwrap_or_else(|| #expr) }
                        }
                        DefaultAssignment::FunctionCall(function_path) => {
                            line = quote! { #line.unwrap_or_else(#function_path) }
                        }
                        DefaultAssignment::DefaultCall => {
                            line = quote! { #line.unwrap_or_default() }
                        }
                    }

                    if field_info.use_type.is_some() {
                        line = quote! { #line.into() }
                    }
                } else {
                    line = quote! { #line.unwrap_or_default() }
                }

                line
            })
            .collect();

        Ok(quote! {
            pub fn unwrap_or_default(self) -> #target_type {
                let Self { #field_idents } = self;
                #target_type {
                    #init_members
                }
            }
        })
    }

    fn build_impl_from_to_target(&self, target_type: &syn::Ident) -> proc_macro2::TokenStream {
        let self_name = &self.name;
        quote! {
            impl From<#self_name> for #target_type {
                fn from(value: #self_name) -> #target_type {
                    value.unwrap_or_default()
                }
            }
        }
    }
}

impl AttributeInfo<StructInfo, FieldInfo> {
    fn is_temporary_field(&self, field: &syn::Field) -> bool {
        self.fields_info
            .values()
            .find(|field_info| {
                field_info
                    .also_from_field
                    .as_ref()
                    .is_some_and(|also_from_field| {
                        Some(&also_from_field.ident) == field.ident.as_ref()
                    })
            })
            .is_some()
    }

    fn is_mergeable_field(&self, field: &syn::Field) -> bool {
        self.fields_info
            .get(&field_name(field))
            .map(|field_info| field_info.mergeable)
            .unwrap_or(false)
    }
}

struct StructInfo {
    name: syn::Ident,
    derive_info: Option<DeriveInfo>,
}

impl Parse for StructInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let beginning_span = input.span();
        let mut name = None;
        let mut derive_info = None;

        loop {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "name" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    name = Some(content.parse()?);
                }
                "derive" => {
                    let content;
                    derive_info = Some(DeriveInfo {
                        ident,
                        paren: parenthesized!(content in input),
                        traits: content.parse_terminated(syn::Ident::parse_any, Token![,])?,
                    });
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
                "Expected \"name\" property for creating new struct, but it doesn't given",
            ));
        };

        Ok(Self { name, derive_info })
    }
}

struct DeriveInfo {
    ident: syn::Ident,
    paren: syn::token::Paren,
    traits: Punctuated<syn::Ident, Token![,]>,
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

struct FieldInfo {
    mergeable: bool,
    default: DefaultAssignment,
    also_from_field: Option<AlsoFromField>,
    use_type: Option<syn::Path>,
}

impl Parse for FieldInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut also_from_field = None;
        let mut mergeable = false;
        let mut default = DefaultAssignment::DefaultCall;
        let mut use_type: Option<syn::Path> = None;

        loop {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "also_from" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    also_from_field = Some(content.parse()?);
                }
                "mergeable" => mergeable = true,
                "default" => {
                    if input.peek(syn::token::Paren) {
                        let content;
                        let _paren = parenthesized!(content in input);
                        default = content.parse()?;
                    } else {
                        default = DefaultAssignment::DefaultCall;
                    }
                }
                "use_type" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    use_type = Some(content.parse()?);
                }
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            } else {
                break;
            }
        }

        Ok(Self {
            mergeable,
            default,
            also_from_field,
            use_type,
        })
    }
}

struct AlsoFromField {
    ident: syn::Ident,
    mergeable: bool,
}

impl Parse for AlsoFromField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let beginning_span = input.span();
        let mut name = None;
        let mut mergeable = false;

        loop {
            let ident: Ident = input.parse()?;

            match ident.to_string().as_str() {
                "name" => {
                    let _eq_token = input.parse::<Token![=]>()?;
                    name = Some(input.parse()?);
                }
                "mergeable" => mergeable = true,
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            } else {
                break;
            }
        }

        let Some(ident) = name else {
            return Err(syn::Error::new(
                beginning_span,
                "Expected at least 'name' for temporary field: #[cfg_prop(also_from(name = field_name))]",
            ));
        };

        Ok(Self { ident, mergeable })
    }
}
