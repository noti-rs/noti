use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, Parser},
    parse_macro_input,
    spanned::Spanned,
    Token,
};

use crate::{general::Structure, propagate_err};

pub(super) fn make_widget(item: TokenStream, attributes: TokenStream) -> TokenStream {
    let macro_attributes = parse_macro_input!(attributes as WidgetAttributes);
    let mut structure = parse_macro_input!(item as Structure);

    let callbacks = propagate_err!(take_callbacks(&mut structure));
    let field_attributes = propagate_err!(take_field_attrs(&mut structure));

    let mut result = proc_macro2::TokenStream::new();

    widget_structure(&structure, &macro_attributes, &callbacks, &field_attributes)
        .to_tokens(&mut result);
    impl_widget_information(&structure, &macro_attributes).to_tokens(&mut result);
    impl_widget(&structure, &macro_attributes).to_tokens(&mut result);
    impl_widget_diff(&structure, &macro_attributes, &field_attributes).to_tokens(&mut result);

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
            let field_attr = match field_attributes.get(field.ident.as_ref().expect("Field must be named")) {
                Some(field_attr) if field_attr.is_style => field_attr,
                _ => {
                    quote! {
                        #field,
                    }.to_tokens(body);

                    continue;
                }
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

            #[builder(default)]
            input_behavior: crate::types::InputBehavior,
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
            let Callback { ident, input_type, output_type } = callback;
            let input_type = if let Some(ty) = input_type {
                ty.to_token_stream()
            } else {
                quote! { () }
            };

            let output_type = if let Some(ty) = output_type {
                ty.to_token_stream()
            } else {
                quote! { () }
            };

            quote! {
                #[builder(with = |f: impl for<'a> crate::events::FunctionCallback<'a, #input_type, #output_type> + 'static| Box::new(f))]
                #ident: std::option::Option<crate::events::Callback<#input_type, #output_type>>,
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

            fn input_behavior(&self) -> &crate::types::InputBehavior {
                &self.input_behavior
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
                    self.padding.unwrap_or_default()
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

fn impl_widget_diff(
    structure: &Structure,
    widget_attributes: &WidgetAttributes,
    field_attributes: &HashMap<syn::Ident, FieldAttributes>,
) -> proc_macro2::TokenStream {
    let Structure { ref name, .. } = structure;

    let mut checks = proc_macro2::TokenStream::new();
    field_attributes
        .iter()
        .filter_map(|(ident, attributes)| attributes.dirty.as_ref().map(|expr| (ident, expr)))
        .for_each(|(ident, punctuated)| {
            let dirty_flags_with_full_path = punctuated
                .iter()
                .map(|ident| quote! { crate::types::dirty_flags::DirtyFlags::#ident })
                .collect::<syn::punctuated::Punctuated<proc_macro2::TokenStream, Token![|]>>();

            quote! {
                if self.#ident != other_widget.#ident {
                    dirty_flags |= #dirty_flags_with_full_path;
                }
            }
            .to_tokens(&mut checks);
        });

    let minimal_checks = quote! {};

    let standard_checks = quote! {
        #minimal_checks

        if self.margin != other_widget.margin {
            dirty_flags |= crate::types::dirty_flags::DirtyFlags::NEEDS_MEASURE;
        }
    };

    let container_checks = quote! {
        #standard_checks

        if self.padding != other_widget.padding || self.border != other_widget.border {
            dirty_flags |= crate::types::dirty_flags::DirtyFlags::NEEDS_MEASURE;
        }

        // TODO: maybe for future flags
        // - background_color
        // - alignment
    };

    let base_checks = match widget_attributes.widget_kind {
        WidgetKind::Minimal => minimal_checks,
        WidgetKind::Standard => standard_checks,
        WidgetKind::Container => container_checks,
    };

    let body = quote! {
        let other_widget = other.downcast_ref::<Self>()
            .expect("The other widget have different type from Self! Something went wrong during rebuild phase.");
        let mut dirty_flags = crate::types::dirty_flags::DirtyFlags::empty();

        #base_checks

        #checks

        dirty_flags
    };

    quote! {
        impl crate::stage::rebuild::WidgetDiff for #name {
            fn diff(&self, other: &dyn std::any::Any) -> crate::types::dirty_flags::DirtyFlags {
                #body
            }
        }
    }
}

struct WidgetAttributes {
    widget_kind: WidgetKind,
}

#[derive(Default, Clone)]
enum WidgetKind {
    Minimal,
    #[default]
    Standard,
    Container,
}

impl WidgetKind {
    fn from_ident(val: syn::Ident) -> syn::Result<Self> {
        Ok(match val.to_string().as_str() {
            "minimal" => WidgetKind::Minimal,
            "standard" => WidgetKind::Standard,
            "container" => WidgetKind::Container,
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
    dirty: Option<syn::punctuated::Punctuated<syn::Ident, Token![|]>>,
}

fn take_field_attrs(
    structure: &mut Structure,
) -> syn::Result<HashMap<syn::Ident, FieldAttributes>> {
    let mut field_attributes = HashMap::new();

    for field in &mut structure.fields {
        let Some(field_ident) = field.ident.as_ref().cloned() else {
            return Err(syn::Error::new(field.ident.span(), "Field must be named"));
        };

        let mut attributes = FieldAttributes {
            is_style: false,
            is_required: false,
            dirty: None,
        };

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
                            return Err(syn::Error::new(
                                ident.span(),
                                "Unknown attribute. Maybe you mean \"required\"?",
                            ));
                        }

                        field.attrs.remove(index);
                    } else if list.path.to_token_stream().to_string() == "dirty" {
                        attributes.dirty = Some(
                            syn::punctuated::Punctuated::parse_separated_nonempty
                                .parse2(list.tokens.clone())?,
                        );
                        field.attrs.remove(index);
                    }
                }
                _ => continue,
            }
        }

        // INFO: if attributes are not empty then insert
        if attributes.is_style || attributes.dirty.is_some() {
            field_attributes.insert(field_ident, attributes);
        }
    }

    Ok(field_attributes)
}
