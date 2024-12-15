use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::{
    ext::IdentExt, parenthesized, parse::Parse, parse_macro_input, punctuated::Punctuated,
    spanned::Spanned, Token,
};

use crate::{
    general::{field_name, AttributeInfo, DefaultAssignment, Structure},
    propagate_err,
};

pub(super) fn make_derive(item: TokenStream) -> TokenStream {
    let mut cfg_struct = parse_macro_input!(item as Structure);
    let attribute_info = propagate_err!(AttributeInfo::parse_removal(&mut cfg_struct, "cfg_prop"));

    let mut cfg_property_struct =
        propagate_err!(cfg_struct.create_property_struct(&attribute_info));
    propagate_err!(cfg_property_struct.unwrap_option_types());
    cfg_property_struct.update_field_types(&attribute_info);

    let mut tokens = proc_macro2::TokenStream::new();
    cfg_property_struct.build_struct(&mut tokens, &attribute_info);

    propagate_err!(cfg_struct.build_impl(&mut tokens, &cfg_property_struct.name, &attribute_info));
    cfg_struct.build_impl_traits(&mut tokens, &cfg_property_struct.name);

    tokens.into()
}

impl Structure {
    fn create_property_struct(
        &self,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<Self> {
        let mut new_type = self.clone();
        new_type.name = attribute_info.struct_info.name.clone();
        Ok(new_type)
    }

    fn unwrap_option_types(&mut self) -> syn::Result<()> {
        fn get_inner_type(type_path: &syn::TypePath) -> syn::Result<syn::Type> {
            let last = type_path
                .path
                .segments
                .last()
                .expect("Must be last element of type path");

            if last.ident != "Option" {
                return Err(syn::Error::new(
                    type_path.span(),
                    "Expected Option<T> type, but given another",
                ));
            }

            let err = Err(syn::Error::new(
                type_path.span(),
                "Expected single <T> argument in Option<T>",
            ));

            let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
                return err;
            };

            let syn::GenericArgument::Type(ty) = arguments.args[0].clone() else {
                return err;
            };

            Ok(ty)
        }

        fn get_type_path(ty: &syn::Type) -> syn::Result<&syn::TypePath> {
            match ty {
                syn::Type::Path(type_path) => Ok(type_path),
                syn::Type::Group(syn::TypeGroup { elem, .. }) => get_type_path(elem),
                _ => Err(syn::Error::new(
                    ty.span(),
                    "Invalid type, expected Option<T> type",
                )),
            }
        }

        for field in self.fields.iter_mut() {
            field.ty = get_inner_type(get_type_path(&field.ty)?)?;
        }

        Ok(())
    }

    fn update_field_types(&mut self, attribute_info: &AttributeInfo<StructInfo, FieldInfo>) {
        for field in self.fields.iter_mut() {
            let Some(field_attr_info) = attribute_info.fields_info.get(&field_name(field)) else {
                continue;
            };

            let Some(use_type) = field_attr_info.use_type.clone() else {
                continue;
            };

            field.ty = syn::Type::Path(syn::TypePath {
                qself: None,
                path: syn::Path::from(use_type),
            });
        }
    }

    fn build_struct(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        attribute_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) {
        self.attributes
            .iter()
            .for_each(|attr| attr.to_tokens(tokens));
        if let Some(derive_info) = &attribute_info.struct_info.derive_info {
            derive_info.to_tokens(tokens);
        }
        attribute_info.struct_info.attributes.to_tokens(tokens);

        self.visibility.to_tokens(tokens);
        self.struct_token.to_tokens(tokens);
        self.name.to_tokens(tokens);
        self.braces.surround(tokens, |tokens| {
            self.fields
                .iter()
                .filter(|field| !attribute_info.is_temporary_field(field))
                .map(|field| {
                    let attribute_tokens = attribute_info
                        .fields_info
                        .get(&field_name(field))
                        .map(|field_info| field_info.attributes.clone())
                        .unwrap_or_default();
                    quote! {
                        #attribute_tokens #field,
                    }
                })
                .for_each(|field_tokens| field_tokens.to_tokens(tokens));
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

        let ident = &self.name;
        quote! {
            impl #ident {
                #fn_merge
                #fn_unwrap_or_default
            }
        }
        .to_tokens(tokens);

        Ok(())
    }

    fn build_fn_merge(
        &self,
        _attr_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let fields = self
            .fields
            .iter()
            .map(|field| {
                let ident = field.ident.as_ref().expect("Must be a named field");
                let mut line = quote! { #ident: self.#ident.clone() };

                if _attr_info.is_mergeable_field(field) {
                    line = quote! { #line.map(|#ident| #ident.merge(other.#ident.clone())) };
                }

                quote! { #line.or(other.#ident.clone()), }
            })
            .reduce(|mut lhs, rhs| {
                lhs.extend(rhs);
                lhs
            })
            .unwrap_or_default();

        Ok(quote! {
            pub fn merge(self, other: Option<Self>) -> Self {
                if let None = other {
                    return self;
                }
                let other = unsafe { other.unwrap_unchecked() };

                Self {
                    #fields
                }
            }
        })
    }

    fn build_fn_unwrap_or_default(
        &self,
        target_type: &syn::Ident,
        attr_info: &AttributeInfo<StructInfo, FieldInfo>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let fields = self
            .fields
            .iter()
            .filter(|field| !attr_info.is_temporary_field(field))
            .map(|field| {
                let ident = field.ident.as_ref().expect("Must be a named field");
                let mut line = quote! { #ident: self.#ident.clone() };

                if let Some(field_char) = attr_info.fields_info.get(&ident.to_string()) {
                    if let Some(InheritsField(target)) = &field_char.inherits {
                        quote! {
                            .map(|val| val.merge(self.#target.clone()))
                            .or(self.#target.clone())
                        }
                        .to_tokens(&mut line);
                    }

                    match &field_char.default {
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

                    if field_char.use_type.is_some() {
                        quote! { .into() }.to_tokens(&mut line);
                    }
                } else {
                    quote! { .unwrap_or_default() }.to_tokens(&mut line);
                }

                quote! { , }.to_tokens(&mut line);

                line
            })
            .reduce(|mut lhs, rhs| {
                lhs.extend(rhs);
                lhs
            })
            .unwrap_or_default();

        Ok(quote! {
            pub fn unwrap_or_default(&self) -> #target_type {
                #target_type {
                    #fields
                }
            }
        })
    }

    fn build_impl_traits(
        &self,
        tokens: &mut proc_macro2::TokenStream,
        target_type: &syn::Ident,
    ) {
        let self_name = &self.name;
        quote! {
            impl From<#self_name> for #target_type {
                fn from(value: #self_name) -> #target_type {
                    value.unwrap_or_default()
                }
            }
        }
        .to_tokens(tokens);
    }
}

impl AttributeInfo<StructInfo, FieldInfo> {
    fn is_temporary_field(&self, field: &syn::Field) -> bool {
        self.fields_info
            .get(&field_name(field))
            .map(|field_info| field_info.temporary)
            .unwrap_or(false)
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
    attributes: proc_macro2::TokenStream,
}

impl Parse for StructInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let beginning_span = input.span();
        let mut name = None;
        let mut derive_info = None;
        let mut attributes = proc_macro2::TokenStream::new();

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
                "attributes" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    attributes = content.parse()?;
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

        Ok(Self {
            name,
            derive_info,
            attributes,
        })
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
    temporary: bool,
    mergeable: bool,
    default: DefaultAssignment,
    inherits: Option<InheritsField>,
    use_type: Option<syn::Ident>,
    attributes: proc_macro2::TokenStream,
}

impl Parse for FieldInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut temporary = false;
        let mut mergeable = false;
        let mut default = DefaultAssignment::DefaultCall;
        let mut inherits: Option<InheritsField> = None;
        let mut use_type: Option<syn::Ident> = None;
        let mut attributes = proc_macro2::TokenStream::new();

        loop {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "temporary" => temporary = true,
                "mergeable" => mergeable = true,
                "default" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    default = content.parse()?;
                }
                "inherits" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    inherits = Some(content.parse()?);
                }
                "use_type" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    use_type = Some(content.parse()?);
                }
                "attributes" => {
                    let content;
                    let _paren = parenthesized!(content in input);
                    attributes = content.parse()?;
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
            temporary,
            mergeable,
            default,
            inherits,
            use_type,
            attributes,
        })
    }
}

struct InheritsField(syn::Ident);

impl Parse for InheritsField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let argument: Ident = input.parse()?;

        if argument.to_string().as_str() != "field" {
            return Err(syn::parse::Error::new(
                argument.span(),
                "Invalid argument. Currently possible only \"field\".",
            ));
        }

        let _eq_token = input.parse::<Token![=]>()?;

        Ok(Self(input.parse()?))
    }
}
