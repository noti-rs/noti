use anyhow::bail;
use config::{
    display::{Border, GBuilderBorder, GBuilderImageProperty, ImageProperty},
    spacing::{GBuilderSpacing, Spacing},
    text::{GBuilderTextProperty, TextProperty},
};
use log::warn;
use pest::iterators::{Pair, Pairs};
use render::widget::{
    Alignment, GBuilderAlignment, GBuilderFlexContainer, GBuilderWImage, GBuilderWText, Widget,
};
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

    let node_type = pair.into_inner().next().unwrap();
    convert_node_type(node_type)
}

fn convert_node_type(node_type: Pair<Rule>) -> anyhow::Result<Widget> {
    assert_eq!(
        node_type.as_rule(),
        Rule::NodeType,
        "In input should be a NodeType"
    );

    let mut node_type_pairs = node_type.into_inner();

    let widget_name = node_type_pairs.next().unwrap().as_str();
    let mut widget_gbuilder: GBuilder = widget_name.try_into()?;

    let properties = convert_properties(&mut node_type_pairs);

    let children = convert_children(&mut node_type_pairs)?;
    widget_gbuilder.set_properties(widget_name, properties);

    if !children.is_empty() {
        let assignment_result =
            widget_gbuilder.set_value("children", Value::Any(Box::new(children)));

        if let Err(ConversionError::UnknownField { field_name }) = assignment_result {
            warn!("The {widget_name} doesn't contain the '{field_name}' field! Skipped.");
        }
    }

    Ok(widget_gbuilder.try_build()?.try_downcast()?)
}

fn convert_properties(node_type_pairs: &mut Pairs<Rule>) -> Vec<Property> {
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
        match convert_property(property) {
            Ok(property) => converted_properties.push(property),
            Err(err) => warn!("Failed to parse property and skipped. Error: {err}"),
        }
        let _comma = properties_pairs.next();
    }

    converted_properties
}

fn convert_property(property: Pair<Rule>) -> anyhow::Result<Property> {
    assert_eq!(
        property.as_rule(),
        Rule::Property,
        "In input should be a Property"
    );

    let mut property_pairs = property.into_inner();
    let name = property_pairs.next().unwrap().as_str().to_string();
    let _eq_token = property_pairs.next();
    let value = convert_property_value(property_pairs.next().unwrap())?;

    Ok(Property { name, value })
}

fn convert_property_value(property_value: Pair<Rule>) -> anyhow::Result<Value> {
    assert_eq!(
        property_value.as_rule(),
        Rule::PropertyValue,
        "In input should be a PropertyValue"
    );

    let value = property_value.into_inner().next().unwrap();

    Ok(match value.as_rule() {
        Rule::TypeValue => convert_type_value(value)?,
        Rule::Literal => Value::String(value.as_str().to_string()),
        Rule::UInt => Value::UInt(value.as_str().parse().unwrap()),
        _ => unreachable!(),
    })
}

fn convert_children(node_type_pairs: &mut Pairs<Rule>) -> anyhow::Result<Vec<Widget>> {
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
        .map(convert_node_type)
        .collect::<anyhow::Result<Vec<Widget>>>()
}

fn convert_type_value(type_value: Pair<Rule>) -> anyhow::Result<Value> {
    assert_eq!(
        type_value.as_rule(),
        Rule::TypeValue,
        "In input should be a TypeValue"
    );

    let mut type_value_pairs = type_value.into_inner();

    let type_name = type_value_pairs.next().unwrap().as_str();
    let mut type_gbuilder: GBuilder = type_name.try_into()?;

    type_gbuilder.set_properties(type_name, convert_properties(&mut type_value_pairs));
    type_gbuilder.try_build().map(Value::Any).map(Ok)?
}

enum GBuilder {
    FlexContainer(GBuilderFlexContainer),
    WImage(GBuilderWImage),
    WText(GBuilderWText),

    TextProperty(GBuilderTextProperty),
    ImageProperty(GBuilderImageProperty),

    Spacing(GBuilderSpacing),
    Alignment(GBuilderAlignment),
    Border(GBuilderBorder),
}

impl GBuilder {
    fn get_associated_property(&self) -> Option<GBuilder> {
        match self {
            GBuilder::WImage(_) => Some(GBuilder::ImageProperty(GBuilderImageProperty::new())),
            GBuilder::WText(_) => Some(GBuilder::TextProperty(GBuilderTextProperty::new())),
            _ => None,
        }
    }

    fn set_properties(&mut self, self_name: &str, properties: Vec<Property>) {
        let mut associated_property = self.get_associated_property();

        for Property { name, value } in properties {
            let this = if self.contains_field(&name) {
                &mut *self
            } else if let Some(property) = associated_property.as_mut() {
                property
            } else {
                warn!("The '{name}' field for the {self_name} widget is unknown. Skipped.");
                continue;
            };

            if let Err(err) = this.set_value(&name, value) {
                warn!(
                    "Cannot set value for the '{name}' field due error and skipped. Error: {err}"
                );
            }
        }

        if let Some(property) = associated_property {
            let property = match property.try_build() {
                Ok(property) => Value::Any(property),
                Err(err) => {
                    println!("{err}");
                    warn!("Failed to analyze properties of the '{self_name}' type and skipped. Error: {err}");
                    return;
                }
            };

            if let Err(err) = self.set_value("property", property) {
                warn!("Failed to apply properties to the '{self_name}' wiget and skipped. Error: {err}");
            }
        }
    }

    fn contains_field(&self, field_name: &str) -> bool {
        macro_rules! implement_variants {
            ($($variant:ident),*) => {
                match self {
                    $(
                        Self::$variant(val) => val.contains_field(field_name),
                    )*
                }
            };
        }

        implement_variants!(
            FlexContainer,
            WImage,
            WText,
            TextProperty,
            ImageProperty,
            Spacing,
            Alignment,
            Border
        )
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
            TextProperty,
            ImageProperty,
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

            TextProperty into TextProperty,
            ImageProperty into ImageProperty,

            Spacing into Spacing,
            Alignment into Alignment,
            Border into Border
        ))
    }
}

impl TryFrom<&str> for GBuilder {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "FlexContainer" => GBuilder::FlexContainer(GBuilderFlexContainer::new()),
            "Image" => GBuilder::WImage(GBuilderWImage::new()),
            "Text" => GBuilder::WText(GBuilderWText::new()),
            "Spacing" => GBuilder::Spacing(GBuilderSpacing::new()),
            "Alignment" => GBuilder::Alignment(GBuilderAlignment::new()),
            "Border" => GBuilder::Border(GBuilderBorder::new()),
            _ => bail!("Unknown type: {value}!"),
        })
    }
}

#[derive(Debug)]
struct Property {
    name: String,
    value: Value,
}
