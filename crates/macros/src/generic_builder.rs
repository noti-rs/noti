use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parenthesized, parse::Parse, parse_macro_input, punctuated::Punctuated, spanned::Spanned, Token,
};

use crate::{
    general::{
        field_name, wrap_by_option, AttributeInfo, DefaultAssignment, DeriveInfo, ExpectIdent,
        Structure,
    },
    propagate_err,
};

pub(super) fn make_derive(item: TokenStream) -> TokenStream {
    let mut structure = parse_macro_input!(item as Structure);
    let attribute_info = propagate_err!(AttributeInfo::parse_removal(&mut structure, "gbuilder"));
    propagate_err!(structure.verify_attributes(&attribute_info));

    let generic_builder = structure.create_generic_builder(&attribute_info.struct_info);

    let mut tokens = proc_macro2::TokenStream::new();
    generic_builder.build_gbuilder_struct(&mut tokens, &attribute_info);

    propagate_err!(generic_builder.build_gbuilder_impl(&mut tokens, &attribute_info, &structure));

    tokens.into()
}

impl Structure {
    fn verify_attributes(
        &self,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<()> {
        let seen_field_names: std::collections::HashSet<_> = self
            .fields
            .iter()
            .map(ExpectIdent::expect_ident)
            .map(ToString::to_string)
            .collect();
        let mut alias_types = std::collections::HashMap::new();

        for (attached_field_name, field_info) in &attribute_info.fields_info {
            let Some(aliases) = &field_info.aliases else {
                continue;
            };

            let field = self
                .fields
                .iter()
                .find(|field| &field_name(field) == attached_field_name)
                .expect("Should be at least one field that have attibutes!");

            for alias in aliases.iter() {
                if seen_field_names.contains(&alias.to_string()) {
                    return Err(syn::Error::new(
                        alias.span(),
                        "The alias name clashes to field name!",
                    ));
                }

                let r#type = alias_types
                    .entry(alias)
                    .or_insert(field.ty.to_token_stream().to_string());

                if *r#type != field.ty.to_token_stream().to_string() {
                    return Err(syn::Error::new(
                        field.ty.span(),
                        format!(
                            "Expected the same types of fields with same attribute value 'aliases' - {}!", 
                            alias
                        )
                    ));
                }
            }
        }

        Ok(())
    }

    fn create_generic_builder(&self, struct_info: &StructInfo) -> Self {
        let mut generic_builder = self.clone();
        generic_builder.name = struct_info.name.clone();
        generic_builder
    }

    fn build_gbuilder_struct(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) {
        let Structure {
            ref visibility,
            ref struct_token,
            ref name,
            ref braces,
            ref fields,
            ..
        } = self;

        let derive_info = attribute_info
            .struct_info
            .derive_info
            .as_ref()
            .map(ToTokens::to_token_stream)
            .unwrap_or_default();
        quote! {
            #derive_info
            #visibility #struct_token #name
        }
        .to_tokens(tokens);

        braces.surround(tokens, |tokens| {
            let mut fields = fields.clone();
            fields.iter_mut().for_each(|field| {
                field.attrs.clear();
                field.ty = if let Some(use_gbuilder) = attribute_info
                    .fields_info
                    .get(&field_name(field))
                    .and_then(|field_info| field_info.use_gbuilder.as_ref())
                {
                    syn::Type::Path(syn::TypePath {
                        qself: None,
                        path: use_gbuilder.clone(),
                    })
                } else {
                    wrap_by_option(field.ty.clone())
                }
            });
            fields.to_tokens(tokens)
        });
    }

    fn build_gbuilder_impl(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
        target_struct: &Structure,
    ) -> syn::Result<()> {
        let unhidden_fields = self
            .fields
            .iter()
            .filter(|field| !attribute_info.is_hidden_field(field))
            .collect::<Punctuated<&syn::Field, Token![,]>>();

        let fn_new = self.build_fn_new(attribute_info);
        let fn_set_value = self.build_fn_set_value(&unhidden_fields, attribute_info);
        let fn_try_build = self.build_fn_try_build(target_struct, attribute_info);

        let gbuilder_ident = &self.name;
        quote! {
            impl #gbuilder_ident {
                #fn_new
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

    fn build_fn_new(
        &self,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> proc_macro2::TokenStream {
        let init_members: Punctuated<proc_macro2::TokenStream, Token![,]> = self
            .fields
            .iter()
            .map(|field| field.ident.clone().expect("Fields should be named"))
            .map(|field_ident| {
                if let Some(use_gbuilder) = attribute_info
                    .fields_info
                    .get(&field_ident.to_string())
                    .and_then(|field_info| field_info.use_gbuilder.as_ref())
                {
                    quote! { #field_ident: #use_gbuilder::new() }
                } else {
                    quote! { #field_ident: None }
                }
            })
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

    fn build_fn_set_value(
        &self,
        unhidden_fields: &Punctuated<&syn::Field, Token![,]>,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> proc_macro2::TokenStream {
        let (simple_fields, associated_gbuilders): (Vec<&syn::Field>, Vec<&syn::Field>) =
            unhidden_fields.clone().into_iter().partition(|field| {
                attribute_info
                    .fields_info
                    .get(&field_name(field))
                    .is_none_or(|field_info| field_info.use_gbuilder.is_none())
            });

        let set_members: Vec<proc_macro2::TokenStream> = simple_fields
            .into_iter()
            .map(|field| {
                let field_ident = field.ident.clone().expect("Fields should be named");
                let field_name = field_ident.to_string();
                quote! {
                    #field_name => {
                        self.#field_ident = Some(shared::value::TryFromValue::try_from(value)?);
                        Ok(self)
                    }
                }
            })
            .collect();

        let assignment_of_aliases: Vec<_> = self.hashmap_of_aliases(attribute_info)
            .iter()
            .map(|(alias_ident, field)| {
                let alias_name = alias_ident.to_string();
                let assignments: Vec<_> = field.iter().map(|field|{
                let field_ident = field.expect_ident();
                quote!{
                    self.#field_ident = Some(shared::value::TryFromValue::try_from_cloned(&value)?);
                }
            }).collect();
                quote! {
                    #alias_name => {
                        #(#assignments)*

                        Ok(self)
                    }
                }
            }).collect();

        let associated_gbuilder_assignments: Vec<proc_macro2::TokenStream> = associated_gbuilders
            .into_iter()
            .map(|field| {
                let field_ident = field.expect_ident();

                quote! {
                    match self.#field_ident.set_value(field_name, value) {
                        Ok(_) => return Ok(self),
                        Err(shared::error::ConversionError::UnknownField{ value: returned_value,.. }) => value = returned_value,
                        Err(err) => return Err(err),
                    }
                }
            })
            .collect();

        let visibility = &self.visibility;
        quote! {
            #visibility fn set_value(
                &mut self,
                field_name: &str,
                mut value: shared::value::Value
            ) -> std::result::Result<&mut Self, shared::error::ConversionError> {
                match field_name {
                    #(#set_members)*
                    #(#assignment_of_aliases)*
                    _ => {
                        #(#associated_gbuilder_assignments)*

                        Err(shared::error::ConversionError::UnknownField { field_name: field_name.to_string(), value })
                    }
                }
            }
        }
    }

    fn build_fn_try_build(
        &self,
        target_struct: &Structure,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> proc_macro2::TokenStream {
        let target_type = &target_struct.name;

        let init_members: Punctuated<proc_macro2::TokenStream, Token![,]> = self
            .fields
            .clone()
            .into_iter()
            .map(|field| {
                let field_ident = field.expect_ident();
                let field_name = field_ident.to_string();
                let mut line = quote! { #field_ident: self.#field_ident };

                let is_associated_gbuilder = attribute_info
                    .fields_info
                    .get(&field_name)
                    .and_then(|field_info| field_info.use_gbuilder.as_ref())
                    .is_some();
                if is_associated_gbuilder {
                    line = quote! { #line.try_build() }
                };

                if let Some(default_assignment) = attribute_info
                    .fields_info
                    .get(&field_name)
                    .and_then(|field_attribute| field_attribute.default.as_ref())
                {
                    match default_assignment {
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
                } else if is_associated_gbuilder {
                    line = quote! { #line? }
                } else {
                    let err_msg = format!("The field '{field_name}' should be set");
                    line = quote! { #line.ok_or(#err_msg)? }
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

    fn hashmap_of_aliases<'a>(
        &'a self,
        attribute_info: &'a AttributeInfo<StructInfo, FieldInfo>,
    ) -> std::collections::HashMap<&'a syn::Ident, Vec<&'a syn::Field>> {
        let mut alias_fields = std::collections::HashMap::new();

        for (attached_field_name, field_info) in &attribute_info.fields_info {
            let Some(aliases) = &field_info.aliases else {
                continue;
            };

            let field = self
                .fields
                .iter()
                .find(|field| &field_name(field) == attached_field_name)
                .expect("Should be at least one field that have attibutes!");

            aliases
                .iter()
                .for_each(|alias| alias_fields.entry(alias).or_insert(vec![]).push(field));
        }

        alias_fields
    }
}

impl AttributeInfo<StructInfo, FieldInfo> {
    fn is_hidden_field(&self, field: &syn::Field) -> bool {
        self.fields_info
            .get(&field_name(field))
            .map(|field_info| field_info.hidden)
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
                    derive_info = Some(DeriveInfo::from_ident_and_input(ident, &input)?);
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

        Ok(Self { name, derive_info })
    }
}

struct FieldInfo {
    hidden: bool,
    default: Option<DefaultAssignment>,
    use_gbuilder: Option<syn::Path>,
    aliases: Option<Punctuated<syn::Ident, Token![,]>>,
}

impl Parse for FieldInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut hidden = false;
        let mut default = None;
        let mut use_gbuilder = None;
        let mut aliases = None;

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
                "use_gbuilder" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    use_gbuilder = Some(content.parse()?);
                }
                "aliases" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    aliases = Some(content.parse_terminated(syn::Ident::parse, Token![,])?);
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
            default,
            hidden,
            use_gbuilder,
            aliases,
        })
    }
}
