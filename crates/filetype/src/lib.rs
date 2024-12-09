use std::path::Path;

use render::widget::Widget;

mod converter;
mod parser;

pub fn parse_layout(path: &Path) -> anyhow::Result<Widget> {
    let data = std::fs::read_to_string(path)?;
    let pairs = parser::parse(&data)?;
    converter::convert_into_widgets(pairs)
}

#[test]
fn minimal_type() {
    let pairs = parser::parse(
        r#"
        /*WImage(
            max_width = 3,
            max_height = 4,
        )*/

        // WText(kind = title)
        
        FlexContainer(
            max_width = 400,
            max_height = 120,

            direction = horizontal,
            alignment = Alignment(
                horizontal = start,
                vertical = space_between,
            )
        ) {
            Image(
                max_size = 86,
            )
            Text(
                kind = title,
                wrap = false,
                line_spacing = 10,
            )
        }
    "#,
    )
    .unwrap();
    converter::convert_into_widgets(pairs).unwrap();
}
