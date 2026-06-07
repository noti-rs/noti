use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{Token, parse::Parse, parse_macro_input, spanned::Spanned};

use crate::{general::{DeriveInfo, Structure}, propagate_err};

pub(super) fn make_widget(item: TokenStream, attributes: TokenStream) -> TokenStream {
    let macro_attributes = parse_macro_input!(attributes as WidgetAttributes);
    let mut structure = parse_macro_input!(item as Structure);

    let widget_style_name = propagate_err!(take_make_widget_style(&mut structure));

    let callbacks = propagate_err!(take_callbacks(&mut structure));
    let field_attributes = propagate_err!(take_field_attrs(&mut structure));

    let mut result = proc_macro2::TokenStream::new();

    widget_structure(&structure, &macro_attributes, &callbacks, &field_attributes)
        .to_tokens(&mut result);
    impl_widget_information(&structure, &macro_attributes).to_tokens(&mut result);
    impl_widget(&structure, &macro_attributes).to_tokens(&mut result);

    if let Some(make_style_widget_info) = widget_style_name {
        let mut structure = structure.clone();

        structure.attributes.clear();
        structure.fields = syn::punctuated::Punctuated::<syn::Field, Token![,]>::from_iter(
            structure.fields.iter().filter(|&field| 
                field_attributes.get(field.ident.as_ref().expect("Field must be named"))
                        .is_some_and(|attr| attr.is_style)).cloned()
            );

        let style_attributes = StyleAttributes {
            targets: vec![structure.name.clone()],
            widget_kind: macro_attributes.widget_kind,
        };

        structure.name = make_style_widget_info.name;

        if let Some(derive_info) = make_style_widget_info.derive {
            structure.attributes.push(
                syn::Attribute {
                    pound_token: Token![#](proc_macro2::Span::call_site()),
                    style: syn::AttrStyle::Outer,
                    bracket_token: syn::token::Bracket(derive_info.span()),
                    meta: syn::Meta::List(syn::MetaList {
                        delimiter: syn::MacroDelimiter::Paren(derive_info.paren),
                        path: syn::Path {
                            leading_colon: None,
                            segments: syn::punctuated::Punctuated::from_iter([syn::PathSegment {
                                ident: derive_info.ident,
                                arguments: syn::PathArguments::None,
                            }])
                        },
                        tokens: derive_info.traits.to_token_stream(),
                    })
                }
            );
        }

        widget_style_structure(&structure, &style_attributes).to_tokens(&mut result);
        impls_configure(&structure, &style_attributes).to_tokens(&mut result);
    }

    result.into()
}

pub(super) fn make_widget_style(item: TokenStream, attributes: TokenStream) -> TokenStream {
    let macro_attributes = parse_macro_input!(attributes as StyleAttributes);
    let structure = parse_macro_input!(item as Structure);

    let mut result = proc_macro2::TokenStream::new();

    widget_style_structure(&structure, &macro_attributes).to_tokens(&mut result);
    impls_configure(&structure, &macro_attributes).to_tokens(&mut result);

    result.into()
}

fn widget_structure(
    structure: &Structure,
    macro_attributes: &WidgetAttributes,
    callbacks: &[Callback],
    field_attributes: &HashMap<syn::Ident, FieldAttributes>,
) -> proc_macro2::TokenStream {
    let Structure {
        ref visibility,
        ref struct_token,
        ref attributes,
        ref name,
        ref braces,
        ref fields,
    } = structure;

    let attributes = attributes
        .iter()
        .fold(proc_macro2::TokenStream::new(), |mut acc, attr| {
            attr.to_tokens(&mut acc);
            acc
        });

    let mut body = proc_macro2::TokenStream::new();
    braces.surround(&mut body, |body| {
        for field in fields {
            let Some(field_attr) = field_attributes.get(field.ident.as_ref().expect("Field must be named")) else {
                quote! {
                    #field,
                }.to_tokens(body);

                continue;
            };

            let syn::Field {
                attrs,
                vis,
                ident,
                colon_token,
                ty,
                ..
            } = field;

            let attrs = attrs.iter().fold(proc_macro2::TokenStream::new(), |mut acc, attr| {
                attr.to_tokens(&mut acc);
                acc
            });

            let builder_arg = if field_attr.is_required {
                quote!{}
            } else {
                quote! {, default}
            };

            if field_attr.is_style {
                quote! {
                    #attrs
                    #[builder(with = |v: #ty| crate::types::style::StyleProperty::Explicit(v) #builder_arg)]
                    #vis #ident #colon_token crate::types::style::StyleProperty<#ty>,
                }.to_tokens(body);
            }

        }

        let minimal_fields = quote! {
            #[builder(skip)]
            id: crate::types::WidgetId,
            key: std::option::Option<crate::types::identifiers::WidgetKey>,
            #[builder(into, default)]
            class: crate::types::WidgetClass,
        };

        let standard_fields = quote! {
            #minimal_fields

            #[builder(with = |v: crate::types::spacing::Spacing| crate::types::style::StyleProperty::Explicit(v), default)]
            margin: crate::types::style::StyleProperty<crate::types::spacing::Spacing>,
        };

        let container_fields = quote! {
            #standard_fields

            #[builder(with = |v: crate::types::spacing::Spacing| crate::types::style::StyleProperty::Explicit(v), default)]
            padding: crate::types::style::StyleProperty<crate::types::spacing::Spacing>,

            #[builder(with = |v: crate::types::Color| crate::types::style::StyleProperty::Explicit(v), default)]
            background_color: crate::types::style::StyleProperty<crate::types::Color>,

            #[builder(with = |v: crate::types::border::Border| crate::types::style::StyleProperty::Explicit(v), default)]
            border: crate::types::style::StyleProperty<crate::types::border::Border>,

            #[builder(with = |v: crate::types::alignment::Alignment| crate::types::style::StyleProperty::Explicit(v), default)]
            alignment: crate::types::style::StyleProperty<crate::types::alignment::Alignment>,
        };

        let additional_fields = match macro_attributes.widget_kind {
            WidgetKind::Minimal => minimal_fields,
            WidgetKind::Standard => standard_fields,
            WidgetKind::Container => container_fields,
        };

        additional_fields.to_tokens(body);

        for callback in callbacks {
            //TODO: later implement output type here
            let Callback { ident, input_type, .. } = callback;
            let input_type = if let Some(ty) = input_type {
                ty.to_token_stream()
            } else {
                quote! { () }
            };

            quote! {
                #[builder(with = |f: impl for<'a> crate::events::FunctionCallback<'a, #input_type> + 'static| Box::new(f))]
                #ident: std::option::Option<crate::events::Callback<#input_type>>,
            }.to_tokens(body);
        }
    });

    quote! {
        #attributes
        #visibility #struct_token #name #body

    }
}

fn impl_widget_information(
    structure: &Structure,
    _macro_attributes: &WidgetAttributes,
) -> proc_macro2::TokenStream {
    let Structure { ref name, .. } = structure;

    quote! {
        impl crate::widget::WidgetInformation for #name {
            fn get_id(&self) -> WidgetId {
                self.id
            }

            fn set_id(&mut self, id: WidgetId) {
                self.id = id;
            }

            fn get_key(&self) -> Option<&WidgetKey> {
                self.key.as_ref()
            }

            fn get_class(&self) -> WidgetClass {
                self.class.clone()
            }
        }
    }
}

fn impl_widget(
    structure: &Structure,
    macro_attributes: &WidgetAttributes,
) -> proc_macro2::TokenStream {
    let Structure { ref name, .. } = structure;

    let fn_outer_spacing = quote! {
        fn outer_spacing(&self) -> Spacing {
            self.margin.unwrap_or_default()
        }
    };

    match macro_attributes.widget_kind {
        WidgetKind::Minimal => quote! {},
        WidgetKind::Standard => quote! {
            impl #name {
                #fn_outer_spacing
            }
        },
        WidgetKind::Container => quote! {
            impl #name {
                #fn_outer_spacing

                fn inner_spacing(&self) -> Spacing {
                    self.spacing.unwrap_or_default()
                        + Spacing::all_directional(
                            self.border
                                .as_ref()
                                .map(|border| border.size)
                                .unwrap_or_default(),
                        )
                }

                fn total_spacing(&self) -> Spacing {
                    self.inner_spacing() + self.outer_spacing()
                }
            }
        },
    }
}

struct WidgetAttributes {
    widget_kind: WidgetKind,
}

#[derive(Default)]
enum WidgetKind {
    Minimal,
    #[default]
    Standard,
    Container,
}

impl WidgetKind {
    fn from_ident(val: syn::Ident) -> syn::Result<Self> {
        Ok(match val.to_string().as_str() {
            "minimal" =>  WidgetKind::Minimal,
            "standard" =>  WidgetKind::Standard,
            "container" =>  WidgetKind::Container,
            _ => {
                return Err(syn::Error::new(
                    val.span(),
                    "Unknown kind, expected one of: minimal, standard or container",
                ));
            }
        })
    }
}


impl Parse for WidgetAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut widget_kind = WidgetKind::default();

        while !input.is_empty() {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "kind" => {
                    let _eq = input.parse::<Token![=]>()?;
                    widget_kind = WidgetKind::from_ident(input.parse::<syn::Ident>()?)?;
                }
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(WidgetAttributes { widget_kind })
    }
}

struct Callback {
    ident: syn::Ident,
    input_type: Option<syn::Type>,
    output_type: Option<syn::Type>,
}

impl Parse for Callback {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        let mut input_type = None;
        let mut output_type = None;

        if !input.is_empty() {
            let _colon = input.parse::<Token![:]>()?;

            input_type = Some(input.parse::<syn::Type>()?);
        }

        if !input.is_empty() {
            let _right_arrow = input.parse::<Token![->]>()?;

            output_type = Some(input.parse::<syn::Type>()?);
        }

        Ok(Callback {
            ident,
            input_type,
            output_type,
        })
    }
}

struct MakeStyleWidgetInfo {
    name: syn::Ident,
    derive: Option<DeriveInfo>,
}

impl Parse for MakeStyleWidgetInfo {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        let mut derive_info = None;

        if !input.is_empty() {

            let _comma = input.parse::<Token![,]>()?;

            let derive_ident = input.parse::<syn::Ident>()?;
            if derive_ident != "derive" {
                return Err(syn::Error::new(derive_ident.span(), "Expected derive ident and input."));
            }

            derive_info = Some(DeriveInfo::from_ident_and_input(derive_ident, &input)?);
        }

        Ok(MakeStyleWidgetInfo {
            name: ident,
            derive: derive_info,
        })
    }
}

fn take_make_widget_style(structure: &mut Structure) -> syn::Result<Option<MakeStyleWidgetInfo>> {
    let mut style_widget_info = None;

    for index in (0..structure.attributes.len()).rev() {
        match &structure.attributes[index].meta {
            syn::Meta::Path(path) => if path.to_token_stream().to_string() == "make_widget_style" {
                return Err(syn::Error::new(path.span(), "Expected single \"#[make_widget_style(Name)]\" attribute."));
            },
            syn::Meta::List(meta_list) => {
                if meta_list.path.to_token_stream().to_string() == "make_widget_style" {
                    style_widget_info = Some(syn::parse2::<MakeStyleWidgetInfo>(meta_list.tokens.clone())?);
                    structure.attributes.remove(index);
                    break;
                }
            },
            syn::Meta::NameValue(meta_name_value) => {
                if meta_name_value.path.to_token_stream().to_string() == "make_widget_style" {
                    return Err(syn::Error::new(meta_name_value.span(), "Expected single \"#[make_widget_style(Name)]\" attribute."));
                }
            }
        }
    }

    Ok(style_widget_info)
}

fn take_callbacks(structure: &mut Structure) -> syn::Result<Vec<Callback>> {
    let mut callbacks = vec![];

    for index in (0..structure.attributes.len()).rev() {
        let attribute_content = match &structure.attributes[index].meta {
            syn::Meta::List(meta_list) => {
                if meta_list.path.to_token_stream().to_string() == "callback" {
                    meta_list
                } else {
                    continue;
                }
            }
            syn::Meta::Path(path) => {
                if path.to_token_stream().to_string() == "callback" {
                    return Err(syn::Error::new(
                        path.span(),
                        "Expected syntax like callback(name: T -> R)",
                    ));
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        match attribute_content.delimiter {
            syn::MacroDelimiter::Paren(_) => (),
            _ => {
                return Err(syn::Error::new(
                    attribute_content.delimiter.span().span(),
                    "Expected parenthesis, but got different surrounding brackets!",
                ))
            }
        }

        let callback_content = attribute_content.tokens.clone();
        let callback = syn::parse2::<Callback>(callback_content)?;

        callbacks.push(callback);
        structure.attributes.remove(index);
    }

    Ok(callbacks)
}

struct FieldAttributes {
    is_style: bool,
    is_required: bool,
}

fn take_field_attrs(
    structure: &mut Structure,
) -> syn::Result<HashMap<syn::Ident, FieldAttributes>> {
    let mut field_attributes = HashMap::new();

    for field in &mut structure.fields {
        let Some(field_ident) = field.ident.as_ref().cloned() else {
            return Err(syn::Error::new(field.ident.span(), "Field must be named"));
        };

        let mut attributes = FieldAttributes { is_style: false, is_required: false, };

        for index in (0..field.attrs.len()).rev() {
            match &field.attrs[index].meta {
                syn::Meta::Path(path) => {
                    if path.to_token_stream().to_string() == "style" {
                        attributes.is_style = true;
                        field.attrs.remove(index);
                    }
                }
                syn::Meta::List(list) => {
                    if list.path.to_token_stream().to_string() == "style" {
                        attributes.is_style = true;

                        let ident = syn::parse2::<syn::Ident>(list.tokens.clone())?;
                        if ident == "required" {
                            attributes.is_required = true;
                        } else {
                            return Err(syn::Error::new(ident.span(), "Unknown attribute. Maybe you mean \"required\"?"))
                        }

                        field.attrs.remove(index);
                    }
                }
                _ => continue,
            }
        }

        // INFO: if attributes are not empty then insert
        if attributes.is_style {
            field_attributes.insert(field_ident, attributes);
        }
    }

    Ok(field_attributes)
}

struct StyleAttributes {
    targets: Vec<syn::Ident>,
    widget_kind: WidgetKind,
}

impl Parse for StyleAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut targets = None;
        let mut widget_kind = WidgetKind::default();

        while !input.is_empty() {
            let ident = input.parse::<syn::Ident>()?;

            match ident.to_string().as_str() {
                "targets" => {
                    let content;
                    syn::parenthesized!(content in input);

                    targets = Some(
                        syn::punctuated::Punctuated::<syn::Ident, Token![,]>::parse_terminated(&content)?
                            .into_iter()
                            .collect::<Vec<_>>()
                    );
                }
                "kind" => {
                    let _eq = input.parse::<Token![=]>()?;
                    widget_kind = WidgetKind::from_ident(input.parse::<syn::Ident>()?)?;

                }
                _ => {
                    return Err(syn::Error::new(ident.span(), "Unknown attribute. Available attributes: kind, targets."));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let Some(targets) = targets else {
            return Err(syn::Error::new(proc_macro2::Span::call_site(), "Missing \"targets\" attribute."))
        };

        Ok(StyleAttributes { targets, widget_kind })
    }
}

fn widget_style_structure(
    structure: &Structure,
    macro_attributes: &StyleAttributes,
) -> proc_macro2::TokenStream {
    let Structure {
        attributes,
        visibility,
        struct_token,
        name,
        braces,
        fields,
    } = structure;

    let attrs = attributes
        .iter()
        .fold(proc_macro2::TokenStream::new(), |mut acc, attr| {
            attr.to_tokens(&mut acc);
            acc
        });

    let mut body = proc_macro2::TokenStream::new();
    braces.surround(&mut body, |body| {
        for field in fields {
            let syn::Field {
                attrs,
                vis,
                ident,
                colon_token,
                ty,
                ..
            } = field;

            let attrs = attrs.iter().fold(proc_macro2::TokenStream::new(), |mut acc, attr| {
                attr.to_tokens(&mut acc);
                acc
            });

            quote! {
                #attrs 
                #[builder(with = |v: #ty| crate::types::style::StyleProperty::FromClass(v), default)]
                #vis #ident #colon_token crate::types::style::StyleProperty<#ty>,
            }.to_tokens(body);
        }


        let standard_fields = quote! {
            #[builder(with = |v: crate::types::spacing::Spacing| crate::types::style::StyleProperty::FromClass(v), default)]
            margin: crate::types::style::StyleProperty<crate::types::spacing::Spacing>,
        };

        let container_fields = quote! {
            #standard_fields

            #[builder(with = |v: crate::types::spacing::Spacing| crate::types::style::StyleProperty::FromClass(v), default)]
            padding: crate::types::style::StyleProperty<crate::types::spacing::Spacing>,

            #[builder(with = |v: crate::types::Color| crate::types::style::StyleProperty::FromClass(v), default)]
            background_color: crate::types::style::StyleProperty<crate::types::Color>,

            #[builder(with = |v: crate::types::border::Border| crate::types::style::StyleProperty::FromClass(v), default)]
            border: crate::types::style::StyleProperty<crate::types::border::Border>,

            #[builder(with = |v: crate::types::alignment::Alignment| crate::types::style::StyleProperty::FromClass(v), default)]
            alignment: crate::types::style::StyleProperty<crate::types::alignment::Alignment>,
        };

        let additional_fields = match macro_attributes.widget_kind {
            WidgetKind::Minimal => proc_macro2::TokenStream::new(),
            WidgetKind::Standard => standard_fields,
            WidgetKind::Container => container_fields,
        };

        additional_fields.to_tokens(body);
    });

    quote! {
        #attrs
        #visibility #struct_token #name #body
    }
}

fn impls_configure(structure: &Structure, macro_attributes: &StyleAttributes) -> proc_macro2::TokenStream {
    let Structure { name,  fields, .. } = structure;

    let body = fields.iter().fold(proc_macro2::TokenStream::new(), |mut acc, field| {
        let field_ident = field.ident.as_ref().expect("Field must be named");
        quote! {
            self.#field_ident.override_if_higher(config.#field_ident);
        }.to_tokens(&mut acc);
        acc
    });

    let mut impls = proc_macro2::TokenStream::new();

    for target in &macro_attributes.targets {
        quote! {
            impl crate::types::style::Configure<#name> for #target {
                fn configure(&mut self, config: #name) {
                    #body
                }
            }
        }.to_tokens(&mut impls);
    }

    impls
}
