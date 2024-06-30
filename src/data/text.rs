use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, iter::Peekable, str::Chars};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    body: String,
    entities: Vec<Entity>,
}

impl Text {
    pub fn parse(input: String) -> Self {
        let parser = Parser::new(input);
        parser.parse()
    }
}

struct Parser {
    input: String,
    body: String,
    entities: Vec<Entity>,
    stack: Vec<(Tag, usize)>,
    pos: usize,
    chars: Peekable<Chars<'static>>,
}

impl Parser {
    fn new(input: String) -> Self {
        let input_static: &'static str = Box::leak(input.into_boxed_str());

        Self {
            input: input_static.to_string(),
            body: String::new(),
            entities: Vec::new(),
            stack: Vec::new(),
            pos: 0,
            chars: input_static.chars().peekable(),
        }
    }

    fn parse(mut self) -> Text {
        while let Some(c) = self.chars.next() {
            if c == '<' {
                self.handle_tag();
            } else {
                self.body.push(c);
                self.pos += 1;
            }
        }

        self.close_unmatched_tags();
        self.entities.sort_by(|a, b| {
            a.offset
                .cmp(&b.offset)
                .then_with(|| a.length.cmp(&b.length))
        });

        Text {
            body: self.body,
            entities: self.entities,
        }
    }

    fn handle_tag(&mut self) {
        let mut tag = String::new();
        while let Some(&next_char) = self.chars.peek() {
            if next_char == '>' {
                self.chars.next();
                break;
            } else {
                tag.push(self.chars.next().unwrap());
            }
        }

        if let Some(parsed_tag) = Tag::parse(&tag) {
            if parsed_tag.is_closing {
                self.handle_closing_tag();
            } else if parsed_tag.is_self_closing {
                self.handle_self_closing_tag(parsed_tag);
            } else {
                self.stack.push((parsed_tag, self.pos));
            }
        }
    }

    fn handle_closing_tag(&mut self) {
        if let Some((start_tag, start_pos)) = self.stack.pop() {
            let length = self.pos - start_pos;
            self.entities.push(Entity {
                offset: start_pos,
                length,
                kind: start_tag.kind,
            });
        }
    }

    fn handle_self_closing_tag(&mut self, tag: Tag) {
        self.entities.push(Entity {
            offset: self.pos,
            length: 0,
            kind: tag.kind,
        });
    }

    fn close_unmatched_tags(&mut self) {
        while let Some((start_tag, start_pos)) = self.stack.pop() {
            let length = self.pos - start_pos;
            self.entities.push(Entity {
                offset: start_pos,
                length,
                kind: start_tag.kind,
            });
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    offset: usize,
    length: usize,
    kind: EntityKind,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
enum EntityKind {
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

struct Tag {
    kind: EntityKind,
    is_closing: bool,
    is_self_closing: bool,
    attributes: Option<HashMap<String, String>>,
}

impl Tag {
    fn parse(tag: &str) -> Option<Tag> {
        let (tag, is_closing) = if tag.starts_with('/') {
            (&tag[1..], true)
        } else {
            (tag, false)
        };

        let is_self_closing = tag.ends_with('/');
        let tag = if is_self_closing {
            &tag[..tag.len() - 1]
        } else {
            tag
        };

        let mut parts = tag.split_whitespace();
        let tag_name = parts.next()?;

        let attributes = parts.collect::<Vec<&str>>().join(" ");
        let attributes = Self::parse_attributes(&attributes);

        let kind = match tag_name {
            "b" => EntityKind::Bold,
            "i" => EntityKind::Italic,
            "u" => EntityKind::Underline,
            "a" => EntityKind::Link {
                href: attributes.get("href").cloned(),
            },
            "img" => EntityKind::Image {
                src: attributes.get("src").cloned(),
                alt: attributes.get("alt").cloned(),
            },
            _ => return None,
        };

        Some(Tag {
            kind,
            is_closing,
            is_self_closing,
            attributes: Some(attributes),
        })
    }

    fn parse_attributes(attributes: &str) -> HashMap<String, String> {
        let re = Regex::new(r#"(\w+)\s*=\s*"([^"]*)""#).unwrap(); // `key = "val"`
        re.captures_iter(attributes)
            .map(|cap| (cap[1].to_string(), cap[2].to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let input = String::from("test <b>text</b> <i>parsing</i> <u>!!!</u>");
        println!("input: {input}");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("test text parsing !!!"),
                entities: vec![
                    Entity {
                        offset: 5,
                        length: 4,
                        kind: EntityKind::Bold,
                    },
                    Entity {
                        offset: 10,
                        length: 7,
                        kind: EntityKind::Italic,
                    },
                    Entity {
                        offset: 18,
                        length: 3,
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
                    length: 8,
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
                        length: 14,
                        kind: EntityKind::Italic,
                    },
                    Entity {
                        offset: 6,
                        length: 8,
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
                    length: 5,
                    kind: EntityKind::Italic,
                },],
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
                        length: 5,
                        kind: EntityKind::Bold,
                    },
                    Entity {
                        offset: 8,
                        length: 3,
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
                    length: 5,
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
                    length: 4,
                    kind: EntityKind::Link { href: None },
                },],
            }
        )
    }

    #[test]
    fn text_with_img() {
        let input = String::from("image:<img src=\"/path/to/image.png\"></img>");
        let text = Text::parse(input);
        assert_eq!(
            text,
            Text {
                body: String::from("image:"),
                entities: vec![Entity {
                    offset: 6,
                    length: 0,
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
                body: String::from(" some text"),
                entities: vec![Entity {
                    offset: 0,
                    length: 0,
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
                    length: 0,
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
                body: String::from(" hi "),
                entities: vec![
                    Entity {
                        offset: 0,
                        length: 4,
                        kind: EntityKind::Italic
                    },
                    Entity {
                        offset: 0,
                        length: 4,
                        kind: EntityKind::Bold
                    },
                ],
            }
        )
    }
}
