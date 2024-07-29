use unicode_segmentation::UnicodeSegmentation;

// AFAIK no entity in the spec is longer than 26 chars.
const MAX_ENTITY_LEN: usize = 26;

// pub(crate) struct Layout {
//     display_list: Vec<Token>,
// }

#[derive(Debug, PartialEq)]
pub(crate) enum Token {
    Text {
        text: String,
        start: usize,
        end: usize,
    },
    Tag(String),
}

impl Token {
    pub(crate) fn new_text_full_len(text: String) -> Self {
        let start = 0;
        let end = text.len();
        Self::Text { text, start, end }
    }
}

pub(crate) fn lex(body: &str, render: bool) -> Vec<Token> {
    let mut in_tag = false;
    let mut current_entity = String::new();
    let mut skip_entity = false;

    let mut current_buf = String::new();

    let mut out = vec![];
    // TODO: Think of a way of getting all the graphemes without allocating another Vec
    let graphemes = UnicodeSegmentation::graphemes(body, true).collect::<Vec<_>>();

    let mut current_index = 0;
    while current_index < graphemes.len() {
        let grapheme = graphemes[current_index];

        if grapheme == "&" {
            if skip_entity {
                // Reset.
                skip_entity = false;
            } else {
                // This is an entity, so we'll consume the chars until we reach its end.

                // TODO: Use https://html.spec.whatwg.org/entities.json to get all entities
                // in the spec?

                current_entity.push_str(grapheme);
                current_index += 1;

                while let Some(next_grapheme) = graphemes.get(current_index) {
                    current_entity.push_str(next_grapheme);
                    current_index += 1;
                    if *next_grapheme == ";" || current_entity.len() == MAX_ENTITY_LEN {
                        break;
                    }
                }

                let parsed_entity = match current_entity.as_str() {
                    "&lt;" => Some('<'),
                    "&gt;" => Some('>'),
                    _ => None,
                };

                if let Some(entity) = parsed_entity {
                    current_buf.push(entity);
                } else {
                    // Skip entities we don't know by "rewinding" the index
                    // to start at the current entity (or whatever else starts with &).
                    // (I don't love this.)
                    skip_entity = true;
                    current_index -= current_entity.len();
                }
                current_entity.clear();
                continue;
            }
        }

        current_index += 1;

        if grapheme == "<" && render {
            in_tag = true;
            if !current_buf.is_empty() {
                out.push(Token::Text {
                    text: current_buf.clone(),
                    start: current_index - current_buf.len(),
                    end: current_index,
                })
            }

            current_buf.clear();
        } else if grapheme == ">" && render {
            in_tag = false;
            out.push(Token::Tag(current_buf.clone()));
            current_buf.clear();
        } else if !in_tag {
            current_buf.push_str(grapheme);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entities() {
        let example = "&lt;div&gt;";
        let parsed = lex(example, true);
        let text = "<div>".to_string();
        let expected = vec![Token::Text {
            start: 0,
            end: text.len(),
            text,
        }];
        assert_eq!(parsed, expected);
    }

    #[test]
    fn skip_unknown_entities() {
        let example = "&potato;div&chips;";
        let parsed = lex(example, true);
        let text = example.to_string();
        let expected = vec![Token::Text {
            start: 0,
            end: text.len(),
            text,
        }];
        assert_eq!(parsed, expected);
    }
}
