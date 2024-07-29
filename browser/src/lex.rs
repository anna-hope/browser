use unicode_segmentation::UnicodeSegmentation;

// AFAIK no entity in the spec is longer than 26 chars.
const MAX_ENTITY_LEN: usize = 26;

pub(crate) struct Layout {
    display_list: Vec<Token>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Token {
    Text(String),
    Tag(String),
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

        if grapheme == "<" && render {
            in_tag = true;
            if !current_buf.is_empty() {
                out.push(Token::Text(current_buf.clone()))
            }

            current_buf.clear();
        } else if grapheme == ">" && render {
            in_tag = false;
            out.push(Token::Tag(current_buf.clone()));
            current_buf.clear();
        } else if !in_tag {
            current_buf.push_str(grapheme);
        }
        current_index += 1;
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
        let expected = vec![Token::Text("<div>".to_string())];
        assert_eq!(parsed, expected);
    }

    #[test]
    fn skip_unknown_entities() {
        let example = "&potato;div&chips;";
        let parsed = lex(example, true);
        let expected = vec![Token::Text("&potato;div&chips;".to_string())];
        assert_eq!(parsed, expected);
    }
}
