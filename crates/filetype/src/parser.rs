use pest::{iterators::Pairs, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "layout.pest"]
pub(super) struct LayoutParser;

pub(super) fn parse(input: &str) -> anyhow::Result<Pairs<Rule>> {
    Ok(LayoutParser::parse(Rule::Layout, input)?)
}

#[test]
fn minimal_example() {
    LayoutParser::parse(
        Rule::Layout,
        r#"
            FlexContainer(
                max_width = larst,
                max_height = 4,
                property = Property(
                    config_val = true,
                ),
            ) {
                Image()
                Text(kind = title)
            }
        "#,
    )
    .unwrap();
}

#[test]
#[should_panic]
fn missing_closing_parenthesis() {
    LayoutParser::parse(
        Rule::Layout,
        r#"
            FlexContainer( {}
        "#,
    )
    .unwrap();
}

#[test]
#[should_panic]
fn missing_comma_in_properties() {
    LayoutParser::parse(
        Rule::Layout,
        r#"
            FlexContainer(
                min_width = 3
                max_width = 4
            )
        "#,
    )
    .unwrap();
}

#[test]
#[should_panic]
fn redundant_comma_in_children() {
    LayoutParser::parse(
        Rule::Layout,
        r#"
            FlexContainer(
                min_width = 3,
                max_width = 4
            ) {
                Text(),
                Image()
            }
        "#,
    )
    .unwrap();
}

#[test]
#[should_panic]
fn test_redundant_semicolon_in_children() {
    LayoutParser::parse(
        Rule::Layout,
        r#"
            FlexContainer(
                min_width = 3,
                max_width = 4
            ) {
                Text();
                Image();
            }
        "#,
    )
    .unwrap();
}
