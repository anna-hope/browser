use slotmap::{DefaultKey, SlotMap};
use unicode_segmentation::UnicodeSegmentation;

// AFAIK no entity in the spec is longer than 26 chars.
const MAX_ENTITY_LEN: usize = 26;

type NodeKey = DefaultKey;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Element {
    pub(crate) tag: String,
    children: Vec<NodeKey>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum NodeData {
    Text(String),
    Element(Element),
}

#[derive(Debug, PartialEq, Clone)]
struct Node {
    data: NodeData,
    key: NodeKey,
    parent_key: Option<NodeKey>,
}

impl Node {
    fn new(key: NodeKey, data: NodeData, parent_key: Option<NodeKey>) -> Self {
        Self {
            key,
            data,
            parent_key,
        }
    }
}

type NodeMap = SlotMap<NodeKey, Node>;

#[derive(Debug, Copy, Clone)]
pub(crate) struct NodeRef<'tree> {
    node: &'tree Node,
    node_map: &'tree NodeMap,
}

impl<'tree> NodeRef<'tree> {
    fn new(node: &'tree Node, node_map: &'tree NodeMap) -> Self {
        Self { node, node_map }
    }

    pub(crate) fn data(&self) -> &NodeData {
        &self.node.data
    }

    pub(crate) fn parent(&self) -> Option<Self> {
        let parent = self
            .node_map
            .get(self.node.parent_key?)
            .expect("The parent key exists, but doesn't map to any existing Node in the NodeMap");
        Some(Self {
            node: parent,
            node_map: self.node_map,
        })
    }

    pub(crate) fn children(&self) -> Option<Vec<Self>> {
        match self.data() {
            NodeData::Element(element) => {
                let child_nodes = element
                    .children
                    .iter()
                    .map(|key| {
                        let node = self.node_map.get(*key).expect(
                            "The child key exists, but doesn't map to any Node in the NodeMap",
                        );
                        Self {
                            node,
                            node_map: &self.node_map,
                        }
                    })
                    .collect::<Vec<_>>();
                Some(child_nodes)
            }
            NodeData::Text(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HtmlTree {
    root: Node,
    node_map: NodeMap,
}

impl HtmlTree {
    fn new(root: Node, node_map: NodeMap) -> Self {
        Self { root, node_map }
    }

    pub(crate) fn root(&self) -> NodeRef {
        NodeRef::new(&self.root, &self.node_map)
    }
}

pub(crate) struct HtmlParser {
    body: String,
    unfinished: Vec<Node>,
    node_map: NodeMap,
    parse_tags: bool,
}

impl HtmlParser {
    fn new(body: String, parse_tags: bool) -> Self {
        Self {
            body,
            unfinished: vec![],
            node_map: SlotMap::new(),
            parse_tags,
        }
    }

    fn parse(&mut self) {
        let mut in_tag = false;
        let mut current_entity = String::new();
        let mut skip_entity = false;

        let mut current_buf = String::new();
        // TODO: Think of a way of getting all the graphemes without allocating another Vec
        let graphemes =
            UnicodeSegmentation::graphemes(self.body.as_str(), true).collect::<Vec<_>>();

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

            if grapheme == "<" && self.parse_tags {
                in_tag = true;
                if !current_buf.is_empty() {
                    self.add_text(current_buf.clone())
                }

                current_buf.clear();
            } else if grapheme == ">" && self.parse_tags {
                in_tag = false;
                self.add_tag(current_buf.clone());
                current_buf.clear();
            } else {
                current_buf.push_str(grapheme);
            }

            current_index += 1;
        }

        if !in_tag && !current_buf.is_empty() {
            self.add_text(current_buf)
        }
    }

    fn add_text(&mut self, text: String) {
        todo!()
    }

    fn add_tag(&mut self, tag: String) {
        todo!()
    }
}

pub(crate) fn parse(body: String, parse_tags: bool) -> Option<HtmlTree> {
    let mut parser = HtmlParser::new(body, parse_tags);
    parser.parse();
    let root_node = parser.unfinished.pop()?;
    Some(HtmlTree::new(root_node, parser.node_map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entities() {
        let example = "&lt;div&gt;";
        let parsed = parse(example.to_string(), true).expect("Must have root node");
        let text = "<div>".to_string();
        let expected = NodeData::Text(text);
        assert_eq!(parsed.root.data, expected);
    }

    #[test]
    fn skip_unknown_entities() {
        let example = "&potato;div&chips;";
        let parsed = parse(example.to_string(), true).expect("Must have root node");
        let text = example.to_string();
        let expected = NodeData::Text(text);
        assert_eq!(parsed.root.data, expected);
    }
}
