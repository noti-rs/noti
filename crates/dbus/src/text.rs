use log::warn;
use std::collections::HashMap;
use unic_segment;

#[derive(Debug, PartialEq, Eq)]
pub struct Text {
    pub body: String,
    pub entities: Vec<Entity>,
}

impl Text {
    pub fn parse(input: String) -> Self {
        Parser::new(&input).parse()
    }
}

struct Parser<'a> {
    input: &'a str,
    body: String,
    entities: Vec<Entity>,
    stack: Vec<ParsedTag>,
    pos: usize,
    byte_pos: usize,
    cursor: unic_segment::GraphemeCursor,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            body: String::new(),
            entities: Vec::new(),
            stack: Vec::new(),
            pos: 0,
            byte_pos: 0,
            cursor: unic_segment::GraphemeCursor::new(0, input.len()),
        }
    }

    fn parse(mut self) -> Text {
        let mut byte_pos = self.cursor.cur_cursor();
        while let Some(grapheme) = self.cursor.next_grapheme(self.input) {
            let prev_pos = self.cursor.cur_cursor();
            match grapheme {
                "<" => {
                    if let Some(end_tag_position) = self.try_handle_tag(byte_pos) {
                        self.cursor.set_cursor(end_tag_position);

                        byte_pos = self.cursor.cur_cursor();
                        continue;
                    } else {
                        self.cursor.set_cursor(prev_pos);
                    }
                }
                "&" => {
                    if let Some(end_html_entity_pos) = self.try_handle_html_entity(byte_pos) {
                        let decoded_html_entity = html_escape::decode_html_entities(
                            &self.input[byte_pos..end_html_entity_pos],
                        );

                        self.pos += unic_segment::Graphemes::new(&decoded_html_entity).count();
                        self.byte_pos += decoded_html_entity.len();

                        self.body.push_str(&decoded_html_entity);

                        self.cursor.set_cursor(end_html_entity_pos);
                        byte_pos = self.cursor.cur_cursor();
                        continue;
                    } else {
                        self.cursor.set_cursor(prev_pos);
                    }
                }
                _ => (),
            }

            self.pos += 1;
            self.byte_pos += self.cursor.cur_cursor() - byte_pos;
            self.body.push_str(grapheme);

            byte_pos = self.cursor.cur_cursor();
        }

        self.close_unmatched_tags();
        self.entities.sort_by(|a, b| {
            a.offset
                .cmp(&b.offset)
                .then_with(|| a.length.cmp(&b.length))
        });

        Text {
            body: self.body.trim().to_string(),
            entities: self.entities,
        }
    }

    fn try_handle_html_entity(&mut self, start_byte_pos: usize) -> Option<usize> {
        // The pattern of html entities used here:
        // https://stackoverflow.com/questions/26127775/remove-html-entities-and-extract-text-content-using-regex
        //
        // And it converted into brute parsing code.
        fn is_alphanumeric(slice: &str) -> bool {
            slice.len() == 1 && slice.as_bytes()[0].is_ascii_alphanumeric()
        }

        fn is_number(slice: &str) -> bool {
            slice.len() == 1 && slice.as_bytes()[0].is_ascii_digit()
        }

        fn is_ascii_char(slice: &str, char: u8) -> bool {
            slice.len() == 1 && slice.as_bytes()[0] == char
        }

        self.cursor.set_cursor(start_byte_pos);

        let amp = self.cursor.next_grapheme(self.input)?;
        if amp != "&" {
            return None;
        }

        let mut begin = self.cursor.cur_cursor();
        let first_grapheme = self.cursor.next_grapheme(self.input)?;

        match first_grapheme {
            x if is_alphanumeric(x) => {
                self.cursor
                    .skip_until(self.input, |grapheme| grapheme == ";");

                if !self.input.as_bytes()[begin..self.cursor.cur_cursor()]
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric())
                {
                    return None;
                }
            }
            x if is_ascii_char(x, b'#') => {
                begin = self.cursor.cur_cursor();
                let second_grapheme = self.cursor.next_grapheme(self.input)?;

                match second_grapheme {
                    x if is_number(x) => {
                        self.cursor
                            .skip_until(self.input, |grapheme| grapheme == ";");

                        if !self.input.as_bytes()[begin..self.cursor.cur_cursor()]
                            .iter()
                            .all(|byte| byte.is_ascii_digit())
                        {
                            return None;
                        }
                    }
                    x if is_ascii_char(x, b'x') => {
                        begin = self.cursor.cur_cursor();
                        self.cursor
                            .skip_until(self.input, |grapheme| grapheme == ";");

                        if !self.input.as_bytes()[begin..self.cursor.cur_cursor()]
                            .iter()
                            .all(|byte| byte.is_ascii_hexdigit())
                        {
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }

        let semi_colon = self.cursor.next_grapheme(self.input)?;
        if semi_colon != ";" {
            return None;
        }

        Some(self.cursor.cur_cursor())
    }

    fn try_handle_tag(&mut self, start_byte_pos: usize) -> Option<usize> {
        let tag = Tag::try_parse(self.input, &mut self.cursor, start_byte_pos)?;

        let end = tag.byte_pos_end;
        match &tag.tag_type {
            TagType::Opening => {
                self.stack.push(ParsedTag {
                    tag,
                    begin_position: self.pos,
                    begin_position_byte: self.byte_pos,
                });
            }
            TagType::Closing => {
                self.handle_closing_tag(tag);
            }
            TagType::SelfClosing => {
                self.handle_self_closing_tag(tag);
            }
        }

        Some(end)
    }

    fn handle_closing_tag(&mut self, closing_tag: Tag) {
        if self
            .stack
            .last()
            .is_some_and(|parsed_tag| parsed_tag.tag.kind.to_id() == closing_tag.kind.to_id())
        {
            let ParsedTag {
                tag,
                begin_position,
                begin_position_byte,
            } = self.stack.pop().unwrap();

            let length = self.pos - begin_position;
            let graphemes = unic_segment::Graphemes::new(&self.body);
            if length > 0
                && !graphemes
                    .skip(begin_position)
                    .all(|grapheme| grapheme.trim().is_empty())
            {
                self.entities.push(Entity {
                    offset: begin_position,
                    offset_in_byte: begin_position_byte,
                    length,
                    length_in_byte: self.byte_pos - begin_position_byte,
                    kind: tag.kind,
                });
            }
        } else {
            let range = closing_tag.byte_pos_begin..closing_tag.byte_pos_end;
            warn!(
                "Unexpected closing tag {} at {:?} in text: {}",
                &self.input[range.clone()],
                range,
                self.input
            );
        }
    }

    fn handle_self_closing_tag(&mut self, tag: Tag) {
        self.entities.push(Entity {
            offset: self.pos,
            offset_in_byte: self.byte_pos,
            length: 0,
            length_in_byte: 0,
            kind: tag.kind,
        });
    }

    fn close_unmatched_tags(&mut self) {
        while let Some(ParsedTag {
            tag,
            begin_position,
            begin_position_byte,
        }) = self.stack.pop()
        {
            let length = self.pos - begin_position;

            let graphemes = unic_segment::Graphemes::new(&self.body);
            if length > 0
                && !graphemes
                    .skip(begin_position)
                    .all(|grapheme| grapheme.trim().is_empty())
            {
                self.entities.push(Entity {
                    offset: begin_position,
                    offset_in_byte: begin_position_byte,
                    length,
                    length_in_byte: self.byte_pos - begin_position_byte,
                    kind: tag.kind,
                });
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Entity {
    pub offset: usize,
    pub offset_in_byte: usize,
    pub length: usize,
    pub length_in_byte: usize,
    pub kind: EntityKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EntityKind {
    Bold,      // <b> ... </b>
    Italic,    // <i> ... </i>
    Underline, // <u> ... </u>
    Link {
        href: Option<String>,
    }, // <a href="hello.com"> ... </a>
    Image {
        src: Option<String>,
        alt: Option<String>,
    }, // <img src="./path/to/image.png" alt="..."/>
}

impl EntityKind {
    fn to_id(&self) -> u8 {
        match self {
            EntityKind::Bold => 0,
            EntityKind::Italic => 1,
            EntityKind::Underline => 2,
            EntityKind::Link { .. } => 3,
            EntityKind::Image { .. } => 4,
        }
    }
}

struct ParsedTag {
    tag: Tag,
    begin_position: usize,
    begin_position_byte: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct Tag {
    byte_pos_begin: usize,
    byte_pos_end: usize,
    kind: EntityKind,
    tag_type: TagType,
}

#[derive(Debug, PartialEq, Eq)]
enum TagType {
    Opening,
    Closing,
    SelfClosing,
}

impl Tag {
    /// Tries to parse the HTML tags: bold, italic, underline, link and image.
    ///
    /// For link supported only `href` attribute.
    /// For image supported only `src` and `alt` attributes.
    fn try_parse(
        input: &str,
        cursor: &mut unic_segment::GraphemeCursor,
        start_byte_pos: usize,
    ) -> Option<Self> {
        cursor.set_cursor(start_byte_pos);

        if cursor.next_grapheme(input)? != "<" {
            return None;
        }

        let mut tag_type = TagType::Opening;
        let mut first_grapheme = cursor.next_grapheme(input)?;

        if first_grapheme == "/" {
            tag_type = TagType::Closing;
            first_grapheme = cursor.next_grapheme(input)?;
        }

        let end_tag_byte_pos;
        let mut attributes = HashMap::new();

        let kind = match first_grapheme {
            "b" => {
                end_tag_byte_pos = Self::close_unattributed_tag(
                    input,
                    cursor,
                    cursor.cur_cursor(),
                    &mut tag_type,
                )?;
                EntityKind::Bold
            }
            "u" => {
                end_tag_byte_pos = Self::close_unattributed_tag(
                    input,
                    cursor,
                    cursor.cur_cursor(),
                    &mut tag_type,
                )?;
                EntityKind::Underline
            }
            "i" => {
                let end_tag_name = cursor.cur_cursor();
                let second_grapheme = cursor.next_grapheme(input);
                let third_grapheme = cursor.next_grapheme(input);
                if second_grapheme.is_some_and(|grapheme| grapheme == "m")
                    && third_grapheme.is_some_and(|grapheme| grapheme == "g")
                {
                    end_tag_byte_pos = Self::close_attributed_tag(
                        input,
                        cursor,
                        cursor.cur_cursor(),
                        &mut tag_type,
                        &mut attributes,
                    )?;
                    EntityKind::Image {
                        src: attributes.get("src").map(ToString::to_string),
                        alt: attributes.get("alt").map(ToString::to_string),
                    }
                } else {
                    end_tag_byte_pos =
                        Self::close_unattributed_tag(input, cursor, end_tag_name, &mut tag_type)?;
                    EntityKind::Italic
                }
            }
            "a" => {
                end_tag_byte_pos = Self::close_attributed_tag(
                    input,
                    cursor,
                    cursor.cur_cursor(),
                    &mut tag_type,
                    &mut attributes,
                )?;
                EntityKind::Link {
                    href: attributes.get("href").map(ToString::to_string),
                }
            }
            _ => return None,
        };

        Some(Self {
            byte_pos_begin: start_byte_pos,
            byte_pos_end: end_tag_byte_pos,
            kind,
            tag_type,
        })
    }

    fn close_unattributed_tag(
        input: &str,
        cursor: &mut unic_segment::GraphemeCursor,
        start_byte_pos: usize,
        tag_type: &mut TagType,
    ) -> Option<usize> {
        cursor.set_cursor(start_byte_pos);

        cursor.skip_whitespaces(input);
        let mut grapheme = cursor.next_grapheme(input)?;

        if grapheme == "/" {
            match tag_type {
                TagType::Opening => *tag_type = TagType::SelfClosing,
                TagType::SelfClosing | TagType::Closing => return None,
            }
            grapheme = cursor.next_grapheme(input)?;
        }

        if grapheme != ">" {
            return None;
        }

        Some(cursor.cur_cursor())
    }

    fn close_attributed_tag<'a>(
        input: &'a str,
        cursor: &mut unic_segment::GraphemeCursor,
        start_byte_pos: usize,
        tag_type: &mut TagType,
        attributes: &mut HashMap<&'a str, &'a str>,
    ) -> Option<usize> {
        cursor.set_cursor(start_byte_pos);

        cursor.skip_whitespaces(input);
        let mut begin = cursor.cur_cursor();

        let mut grapheme = cursor.next_grapheme(input)?;
        while grapheme != ">" && grapheme != "/" {
            cursor.skip_until(input, |grapheme| grapheme == "=");

            let attribute_name = &input[begin..cursor.cur_cursor()].trim();
            if !attribute_name
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_alphabetic())
            {
                return None;
            }

            let _eq_token = cursor.next_grapheme(input)?;

            cursor.skip_until(input, |grapheme| grapheme == "\"");
            let _begin_double_quote = cursor.next_grapheme(input)?;
            let attr_value_begin = cursor.cur_cursor();

            cursor.skip_until(input, |grapheme| grapheme == "\"");
            let attribute_value = &input[attr_value_begin..cursor.cur_cursor()];
            let _end_double_quote = cursor.next_grapheme(input);

            attributes.insert(attribute_name, attribute_value);

            cursor.skip_whitespaces(input);
            begin = cursor.cur_cursor();
            grapheme = cursor.next_grapheme(input)?;
        }

        if grapheme == "/" {
            match tag_type {
                TagType::Opening => *tag_type = TagType::SelfClosing,
                TagType::SelfClosing | TagType::Closing => return None,
            }
            grapheme = cursor.next_grapheme(input)?;
        }

        if grapheme != ">" {
            return None;
        }

        Some(cursor.cur_cursor())
    }
}

trait NextGrapheme {
    fn next_grapheme<'b>(&mut self, input: &'b str) -> Option<&'b str>;
}

impl NextGrapheme for unic_segment::GraphemeCursor {
    fn next_grapheme<'b>(&mut self, input: &'b str) -> Option<&'b str> {
        let start = self.cur_cursor();
        Some(&input[start..self.next_boundary(input, 0).unwrap()?])
    }
}

trait SkipWhitespaces {
    fn skip_whitespaces(&mut self, input: &str);
}

impl SkipWhitespaces for unic_segment::GraphemeCursor {
    fn skip_whitespaces(&mut self, input: &str) {
        let mut grapheme = self.next_grapheme(input);
        while grapheme.is_some_and(|grapheme| grapheme.trim().is_empty()) {
            grapheme = self.next_grapheme(input);
        }

        if grapheme.is_some_and(|grapheme| !grapheme.trim().is_empty()) {
            // INFO: backtrack to correct cursor boundary
            let _ = self.prev_boundary(input, 0);
        }
    }
}

trait SkipUntil {
    /// Skips the graphemes before it matches
    fn skip_until<'a, 'b, F: Fn(&'b str) -> bool>(&'a mut self, input: &'b str, checker: F);
}

impl SkipUntil for unic_segment::GraphemeCursor {
    fn skip_until<'a, 'b, F: Fn(&'b str) -> bool>(&'a mut self, input: &'b str, checker: F) {
        let mut grapheme = self.next_grapheme(input);
        while grapheme.is_some_and(|grapheme| !checker(grapheme)) {
            grapheme = self.next_grapheme(input);
        }

        if grapheme.is_some_and(checker) {
            // INFO: backtrack to correct cursor boundary
            let _ = self.prev_boundary(input, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_not_tag() {
        let input = String::from("Normal equation: 1 < 2");
        let text = Text::parse(input.clone());
        assert_eq!(
            text,
            Text {
                body: input,
                entities: vec![]
            }
        )
    }

    #[test]
    fn text_with_emoji() {
        let input = String::from("<b>coffee ☕️</b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("coffee ☕️"),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 8,
                    length_in_byte: 13,
                    kind: EntityKind::Bold
                }]
            }
        )
    }

    #[test]
    fn text() {
        let input = String::from("hello world!!!");
        let text = Text::parse(input.clone());
        assert_eq!(
            text,
            Text {
                body: input,
                entities: vec![],
            }
        );
    }

    #[test]
    fn text_with_tags() {
        let input = String::from("test<b>text</b> <i>parsing</i><u>!!!</u>");
        println!("input: {input}");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("testtext parsing!!!"),
                entities: vec![
                    Entity {
                        offset: 4,
                        offset_in_byte: 4,
                        length: 4,
                        length_in_byte: 4,
                        kind: EntityKind::Bold,
                    },
                    Entity {
                        offset: 9,
                        offset_in_byte: 9,
                        length: 7,
                        length_in_byte: 7,
                        kind: EntityKind::Italic,
                    },
                    Entity {
                        offset: 16,
                        offset_in_byte: 16,
                        length: 3,
                        length_in_byte: 3,
                        kind: EntityKind::Underline,
                    },
                ],
            }
        );
    }

    #[test]
    fn text_with_unclosed_tag() {
        let input = String::from("hello <b>world!!!");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello world!!!"),
                entities: vec![Entity {
                    offset: 6,
                    offset_in_byte: 6,
                    length: 8,
                    length_in_byte: 8,
                    kind: EntityKind::Bold,
                }],
            }
        );
    }

    #[test]
    fn text_with_multiple_unclosed_tags() {
        let input = String::from("<i>hello <b>world!!!");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello world!!!"),
                entities: vec![
                    Entity {
                        offset: 0,
                        offset_in_byte: 0,
                        length: 14,
                        length_in_byte: 14,
                        kind: EntityKind::Italic,
                    },
                    Entity {
                        offset: 6,
                        offset_in_byte: 6,
                        length: 8,
                        length_in_byte: 8,
                        kind: EntityKind::Bold,
                    },
                ],
            }
        );
    }

    #[test]
    fn text_with_unmatched_tags() {
        let input = String::from("<i>hello</b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello"),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 5,
                    length_in_byte: 5,
                    kind: EntityKind::Italic,
                },],
            }
        );
    }

    #[test]
    fn text_with_unmatched_tags2() {
        let input = String::from("hello<i> <b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello"),
                entities: vec![],
            }
        );
    }

    #[test]
    fn text_with_nested_tags() {
        let input = String::from("hello <b>wo<i>rld</i></b>!!!");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello world!!!"),
                entities: vec![
                    Entity {
                        offset: 6,
                        offset_in_byte: 6,
                        length: 5,
                        length_in_byte: 5,
                        kind: EntityKind::Bold,
                    },
                    Entity {
                        offset: 8,
                        offset_in_byte: 8,
                        length: 3,
                        length_in_byte: 3,
                        kind: EntityKind::Italic,
                    },
                ],
            }
        );
    }

    #[test]
    fn text_with_link() {
        let input = String::from("hello <a href=\"link.com\">click</a>!!!");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello click!!!"),
                entities: vec![Entity {
                    offset: 6,
                    offset_in_byte: 6,
                    length: 5,
                    length_in_byte: 5,
                    kind: EntityKind::Link {
                        href: Some("link.com".to_string())
                    },
                },],
            }
        )
    }

    #[test]
    fn text_with_link_without_href() {
        let input = String::from("<a>link</a>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("link"),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 4,
                    length_in_byte: 4,
                    kind: EntityKind::Link { href: None },
                },],
            }
        )
    }

    #[test]
    fn text_with_img() {
        let input = String::from("image:<img src=\"/path/to/image.png\"/>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("image:"),
                entities: vec![Entity {
                    offset: 6,
                    offset_in_byte: 6,
                    length: 0,
                    length_in_byte: 0,
                    kind: EntityKind::Image {
                        src: Some("/path/to/image.png".to_string()),
                        alt: None
                    },
                },],
            }
        )
    }

    #[test]
    fn text_with_img_2() {
        let input = String::from("<img src=\"/path/to/image.png\"/> some text");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("some text"),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 0,
                    length_in_byte: 0,
                    kind: EntityKind::Image {
                        src: Some("/path/to/image.png".to_string()),
                        alt: None
                    },
                },],
            }
        )
    }

    #[test]
    fn text_with_img_with_alt() {
        let input =
            String::from("image: <img src=\"/path/to/image.png\" alt=\"some cool image\"/>!!!");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("image: !!!"),
                entities: vec![Entity {
                    offset: 7,
                    offset_in_byte: 7,
                    length: 0,
                    length_in_byte: 0,
                    kind: EntityKind::Image {
                        src: Some("/path/to/image.png".to_string()),
                        alt: Some("some cool image".to_string()),
                    },
                },],
            }
        )
    }

    #[test]
    fn text_with_stupid_tags() {
        let input = String::from("<b><i> hi </b></i>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hi"),
                entities: vec![
                    Entity {
                        offset: 0,
                        offset_in_byte: 0,
                        length: 4,
                        length_in_byte: 4,
                        kind: EntityKind::Italic
                    },
                    Entity {
                        offset: 0,
                        offset_in_byte: 0,
                        length: 4,
                        length_in_byte: 4,
                        kind: EntityKind::Bold
                    },
                ],
            }
        )
    }

    #[test]
    fn text_with_empty_tags() {
        let input = String::from("test<b></b> <i> </i> <u>         </u>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("test"),
                entities: vec![],
            }
        )
    }

    #[test]
    fn text_with_empty_tags2() {
        let input = String::from("test<b>  <i> <u>      </u> </i> </b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("test"),
                entities: vec![],
            }
        )
    }

    #[test]
    fn text_with_spaces() {
        let input = String::from("test       asdasd");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("test       asdasd"),
                entities: vec![],
            }
        )
    }

    #[test]
    fn text_with_html_symbol() {
        let input = String::from("<b>hello&quot;</b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello\""),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 6,
                    length_in_byte: 6,
                    kind: EntityKind::Bold
                }]
            }
        )
    }

    #[test]
    fn text_with_chained_html_symbols() {
        let input = String::from("<b>hello&amp;quot;</b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello&quot;"),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 11,
                    length_in_byte: 11,
                    kind: EntityKind::Bold
                }]
            }
        )
    }

    #[test]
    fn text_with_escaped_slash() {
        let input = String::from("hello<&#47;b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("hello</b>"),
                entities: vec![]
            }
        )
    }

    #[test]
    fn text_with_lt_and_gt_html_escapes() {
        let input = String::from("<b>&lt;i&gt;penis&lt;/i&gt;</b>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("<i>penis</i>"),
                entities: vec![Entity {
                    offset: 0,
                    offset_in_byte: 0,
                    length: 12,
                    length_in_byte: 12,
                    kind: EntityKind::Bold
                }]
            }
        )
    }
}
