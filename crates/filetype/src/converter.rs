use std::collections::HashMap;

use anyhow::bail;
use config::{
    display::{Border, GBuilderBorder},
    spacing::{GBuilderSpacing, Spacing},
};
use log::warn;
use pest::iterators::{Pair, Pairs};
use render::widget::{Alignment, GBuilderAlignment, GBuilderFlexContainer, GBuilderWImage, GBuilderWText, Widget};
use shared::{
    error::ConversionError,
    value::{TryDowncast, Value},
};

use crate::parser::Rule;

pub(super) fn convert_into_widgets(mut pairs: Pairs<Rule>) -> anyhow::Result<Widget> {
    let pair = pairs
        .next()
        .expect("There should be at least one Pair with Rule - Layout.");

    assert_eq!(
        pair.as_rule(),
        Rule::Layout,
        "In input should be a parsed Layout"
    );

    let mut inner_pairs = pair.into_inner();
    let mut alias_storage = HashMap::new();

    let maybe_aliases = inner_pairs.next().unwrap();
    let node_type = if maybe_aliases.as_rule() == Rule::AliasDefinitions {
        convert_aliases(maybe_aliases, &mut alias_storage)?;
        inner_pairs.next().unwrap()
    } else {
        maybe_aliases
    };

    convert_node_type(node_type, &alias_storage)
}

fn convert_aliases<'a>(
    alias_definitions: Pair<'a, Rule>,
    alias_storage: &mut HashMap<&'a str, GBuilder>,
) -> anyhow::Result<()> {
    assert_eq!(
        alias_definitions.as_rule(),
        Rule::AliasDefinitions,
        "In input should be an AliasDefinitions"
    );

    for alias_definition in alias_definitions.into_inner() {
        debug_assert_eq!(
            alias_definition.as_rule(),
            Rule::AliasDefinition,
            "In input should be an AliasDefinition"
        );

        let mut alias_definition_pairs = alias_definition.into_inner();

        let _alias_keyword = alias_definition_pairs.next().unwrap();
        let alias_identifier = alias_definition_pairs.next().unwrap().as_str();
        let _eq_token = alias_definition_pairs.next().unwrap();
        let type_value_definition = alias_definition_pairs.next().unwrap();

        alias_storage.insert(
            alias_identifier,
            convert_type_value(type_value_definition, alias_storage)?,
        );
    }

    Ok(())
}

fn convert_node_type<'a>(
    node_type: Pair<'a, Rule>,
    alias_storage: &'a HashMap<&'a str, GBuilder>,
) -> anyhow::Result<Widget> {
    assert_eq!(
        node_type.as_rule(),
        Rule::NodeType,
        "In input should be a NodeType"
    );

    let mut node_type_pairs = node_type.into_inner();

    let widget_name = node_type_pairs.next().unwrap().as_str();
    let mut widget_gbuilder: GBuilder = (widget_name, alias_storage).try_into()?;

    let properties = convert_properties(&mut node_type_pairs, alias_storage);

    let children = convert_children(&mut node_type_pairs, alias_storage)?;
    widget_gbuilder.set_properties(widget_name, properties);

    if !children.is_empty() {
        let assignment_result =
            widget_gbuilder.set_value("children", Value::Any(Box::new(children)));

        if let Err(ConversionError::UnknownField { field_name, .. }) = assignment_result {
            warn!("The {widget_name} doesn't contain the '{field_name}' field! Skipped.");
        }
    }

    match widget_gbuilder.try_build()?.try_downcast() {
        Ok(val) => Ok(val),
        Err(err) => bail!("{err}"),
    }
}

fn convert_properties<'a>(
    node_type_pairs: &mut Pairs<'a, Rule>,
    alias_storage: &'a HashMap<&'a str, GBuilder>,
) -> Vec<Property> {
    let _open_paren = node_type_pairs.next();

    let properties_or_close_paren = node_type_pairs.next().unwrap();
    let properties;
    if let Rule::Properties = properties_or_close_paren.as_rule() {
        properties = properties_or_close_paren;
        let _close_paren = node_type_pairs.next();
    } else {
        return vec![];
    }

    assert_eq!(
        properties.as_rule(),
        Rule::Properties,
        "In input shoul be the Properties",
    );

    let mut converted_properties = vec![];
    let mut properties_pairs = properties.into_inner();
    while let Some(property) = properties_pairs.next() {
        match convert_property(property, alias_storage) {
            Ok(property) => converted_properties.push(property),
            Err(err) => warn!("Failed to parse property and skipped. Error: {err}"),
        }
        let _comma = properties_pairs.next();
    }

    converted_properties
}

fn convert_property<'a>(
    property: Pair<'a, Rule>,
    alias_storage: &'a HashMap<&'a str, GBuilder>,
) -> anyhow::Result<Property> {
    assert_eq!(
        property.as_rule(),
        Rule::Property,
        "In input should be a Property"
    );

    let mut property_pairs = property.into_inner();
    let name = property_pairs.next().unwrap().as_str().to_string();
    let _eq_token = property_pairs.next();
    let value = convert_property_value(property_pairs.next().unwrap(), alias_storage)?;

    Ok(Property { name, value })
}

fn convert_property_value<'a>(
    property_value: Pair<'a, Rule>,
    alias_storage: &'a HashMap<&'a str, GBuilder>,
) -> anyhow::Result<Value> {
    assert_eq!(
        property_value.as_rule(),
        Rule::PropertyValue,
        "In input should be a PropertyValue"
    );

    let value = property_value.into_inner().next().unwrap();

    Ok(match value.as_rule() {
        Rule::TypeValue => convert_type_value(value, alias_storage)
            .and_then(GBuilder::try_build)
            .map(Value::Any)?,
        Rule::Literal => Value::String(value.as_str().to_string()),
        Rule::UInt => Value::UInt(value.as_str().parse().unwrap()),
        _ => unreachable!(),
    })
}

fn convert_children<'a>(
    node_type_pairs: &mut Pairs<'a, Rule>,
    alias_storage: &'a HashMap<&'a str, GBuilder>,
) -> anyhow::Result<Vec<Widget>> {
    let open_brace = node_type_pairs.next();

    let mut children = None;
    if open_brace.is_some() {
        let close_brace_or_children = node_type_pairs.next().unwrap();

        if let Rule::Children = close_brace_or_children.as_rule() {
            children = Some(close_brace_or_children);
            let _close_brace = node_type_pairs.next();
        }
    }

    let Some(children) = children else {
        return Ok(vec![]);
    };

    assert_eq!(
        children.as_rule(),
        Rule::Children,
        "In input should be the Children"
    );

    children
        .into_inner()
        .map(|child| convert_node_type(child, alias_storage))
        .collect::<anyhow::Result<Vec<Widget>>>()
}

fn convert_type_value<'a>(
    type_value: Pair<'a, Rule>,
    alias_storage: &'a HashMap<&'a str, GBuilder>,
) -> anyhow::Result<GBuilder> {
    assert_eq!(
        type_value.as_rule(),
        Rule::TypeValue,
        "In input should be a TypeValue"
    );

    let mut type_value_pairs = type_value.into_inner();

    let type_name = type_value_pairs.next().unwrap().as_str();
    let mut type_gbuilder: GBuilder = (type_name, alias_storage).try_into()?;

    let maybe_value = type_value_pairs.clone().nth(1).unwrap();
    if maybe_value.as_rule() == Rule::Properties {
        type_gbuilder.set_properties(
            type_name,
            convert_properties(&mut type_value_pairs, alias_storage),
        );
    } else {
        type_gbuilder.constructor(
            type_name,
            convert_property_value(maybe_value, alias_storage)?,
        );
    }

    Ok(type_gbuilder)
}

#[derive(Clone)]
enum GBuilder {
    FlexContainer(GBuilderFlexContainer),
    WImage(GBuilderWImage),
    WText(GBuilderWText),

    Spacing(GBuilderSpacing),
    Alignment(GBuilderAlignment),
    Border(GBuilderBorder),
}

impl GBuilder {
    fn set_properties(&mut self, self_name: &str, properties: Vec<Property>) {
        for Property { name, value } in properties {
            if let Err(err) = self.set_value(&name, value) {
                warn!(
                    "Cannot set value for the '{name}' field in {self_name} due error and skipped. Error: {err}"
                );
            }
        }
    }

    fn constructor(&mut self, self_name: &str, value: Value) {
        macro_rules! implement_variants {
            ($($variant:ident),*) => {
                match self {
                    $(
                        Self::$variant(val) => {
                            val.constructor(value).err()
                        }
                    )*
                }
            };
        }

        if let Some(err) = implement_variants!(
            FlexContainer,
            WImage,
            WText,
            Spacing,
            Alignment,
            Border
        ) {
            warn!("Failed to call constructor of {self_name}, trying to defaulting. Error: {err}");
        }
    }

    fn set_value(&mut self, field_name: &str, value: Value) -> Result<&mut Self, ConversionError> {
        macro_rules! implement_variants {
            ($($variant:ident),*) => {
                match self {
                    $(
                        Self::$variant(val) => {
                            val.set_value(field_name, value)?;
                        }
                    )*
                }
            };
        }

        implement_variants!(
            FlexContainer,
            WImage,
            WText,
            Spacing,
            Alignment,
            Border
        );
        Ok(self)
    }

    fn try_build(self) -> anyhow::Result<Box<dyn std::any::Any>> {
        macro_rules! implement_variants {
            ($($variant:ident into $dest_type:path),*) => {
                match self {
                    $(
                        Self::$variant(val) => Box::new(Into::<$dest_type>::into(
                            val.try_build().map_err(|str| anyhow::format_err!("{str}"))?
                        )),
                    )*
                }
            };
        }

        Ok(implement_variants!(
            WImage into Widget,
            WText into Widget,
            FlexContainer into Widget,

            Spacing into Spacing,
            Alignment into Alignment,
            Border into Border
        ))
    }
}

impl TryFrom<(&str, &HashMap<&str, GBuilder>)> for GBuilder {
    type Error = anyhow::Error;

    fn try_from(
        (identifier, alias_storage): (&str, &HashMap<&str, GBuilder>),
    ) -> Result<Self, Self::Error> {
        Ok(match identifier {
            "FlexContainer" => GBuilder::FlexContainer(GBuilderFlexContainer::new()),
            "Image" => GBuilder::WImage(GBuilderWImage::new()),
            "Text" => GBuilder::WText(GBuilderWText::new()),
            "Spacing" => GBuilder::Spacing(GBuilderSpacing::new()),
            "Alignment" => GBuilder::Alignment(GBuilderAlignment::new()),
            "Border" => GBuilder::Border(GBuilderBorder::new()),
            other => {
                if let Some(aliased_gbuilder) = alias_storage.get(other).cloned() {
                    aliased_gbuilder
                } else {
                    bail!("Unknown type: {other}!")
                }
            }
        })
    }
}

#[derive(Debug)]
struct Property {
    name: String,
    value: Value,
}
