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
    root_key: NodeKey,
    node_map: NodeMap,
}

impl HtmlTree {
    fn new(root_key: NodeKey, node_map: NodeMap) -> Self {
        Self { root_key, node_map }
    }

    pub(crate) fn root(&self) -> NodeRef {
        let root = self
            .node_map
            .get(self.root_key)
            .expect("The root key doesn't map to any node in the NodeMap");
        NodeRef::new(root, &self.node_map)
    }
}

pub(crate) struct HtmlParser {
    unfinished: Vec<NodeKey>,
    node_map: NodeMap,
    parse_tags: bool,
}

impl HtmlParser {
    fn new(parse_tags: bool) -> Self {
        Self {
            unfinished: vec![],
            node_map: SlotMap::new(),
            parse_tags,
        }
    }

    fn parse(mut self, body: String) -> Option<HtmlTree> {
        let mut in_tag = false;
        let mut current_entity = String::new();
        let mut skip_entity = false;

        let mut current_buf = String::new();
        // TODO: Think of a way of getting all the graphemes without allocating another Vec
        let graphemes = UnicodeSegmentation::graphemes(body.as_str(), true).collect::<Vec<_>>();

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

        Some(HtmlTree::new(self.finish()?, self.node_map))
    }

    fn add_text(&mut self, text: String) {
        let parent_key = self
            .unfinished
            .last()
            .expect("No parent node to add this text to");
        let data = NodeData::Text(text);
        let node_key = self
            .node_map
            .insert_with_key(|key| Node::new(key, data, Some(*parent_key)));
        let parent = self
            .node_map
            .get_mut(*parent_key)
            .expect("This parent key doesn't map to any Node in the NodeMap");
        match parent.data {
            NodeData::Element(ref mut element) => element.children.push(node_key),
            _ => panic!(
                "Parent data must be NodeData::Element, got {:?}",
                parent.data
            ),
        }
    }

    fn add_tag(&mut self, tag: String) {
        if tag.starts_with('/') {
            // "The last tag is an edge case, because there's no unfinished node to add it to."
            if self.unfinished.len() == 1 {
                return;
            }

            let node_key = self.unfinished.pop().expect("No node keys in unfinished");
            let parent_key = self.unfinished.last().expect("No node keys in unfinished");
            let parent = self
                .node_map
                .get_mut(*parent_key)
                .expect("The parent key doesn't map to any Node in the NodeMap");

            match parent.data {
                NodeData::Element(ref mut element) => element.children.push(node_key),
                _ => panic!("Parent must be NodeData::Element, got {:?}", parent.data),
            }
        } else {
            let parent = self.unfinished.last();
            let data = NodeData::Element(Element {
                tag,
                children: vec![],
            });
            let node_key = self
                .node_map
                .insert_with_key(|key| Node::new(key, data, parent.copied()));
            self.unfinished.push(node_key);
        }
    }

    fn finish(&mut self) -> Option<NodeKey> {
        while self.unfinished.len() > 1 {
            // Ok to unwrap here because we definitely have > 1 keys in unfinished.
            #[allow(clippy::unwrap_used)]
            let node_key = self.unfinished.pop().unwrap();
            let parent_key = *self.unfinished.last().unwrap();
            let parent = self
                .node_map
                .get_mut(parent_key)
                .expect("This parent key doesn't map to any Node in the NodeMap");
            match parent.data {
                NodeData::Element(ref mut element) => element.children.push(node_key),
                _ => panic!("Parent must be NodeData::Element, got {:?}", parent.data),
            }
        }
        self.unfinished.pop()
    }
}

pub(crate) fn parse(body: String, parse_tags: bool) -> Option<HtmlTree> {
    HtmlParser::new(parse_tags).parse(body)
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
        assert_eq!(parsed.root().data(), &expected);
    }

    #[test]
    fn skip_unknown_entities() {
        let example = "&potato;div&chips;";
        let parsed = parse(example.to_string(), true).expect("Must have root node");
        let text = example.to_string();
        let expected = NodeData::Text(text);
        assert_eq!(parsed.root().data(), &expected);
    }
}
