use super::{print_tree, Layer};
use crate::language::capture::Capture;
use eddy_ts::{language, Language, Node, Parser, Query, QueryCursor, Tree};
use ropey::Rope;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

pub struct RustLayer {
    highlights_query: Query,
    captures_by_id: Vec<Option<Capture>>,
    node_to_capture: HashMap<usize, Capture>,
    parser: Parser,
    tree: Option<Tree>,
}

impl RustLayer {
    pub fn new() -> Self {
        let highlights_query = Query::new(Self::lang(), language::RUST_HIGHLIGHTS).unwrap();
        let mut capture_map = HashMap::new();
        let captures_by_id = highlights_query
            .capture_names()
            .iter()
            .map(|cn| Capture::from_name(cn))
            .collect();
        for (id, cn) in highlights_query.capture_names().iter().enumerate() {
            capture_map.insert(id, Capture::from_name(cn));
        }

        let mut parser = Parser::new();
        parser.set_language(Self::lang());

        Self {
            highlights_query,
            captures_by_id,
            node_to_capture: HashMap::new(),
            parser,
            tree: None,
        }
    }
    pub fn lang() -> Language {
        language::rust()
    }
    pub fn highlights_query() -> Query {
        Query::new(Self::lang(), language::RUST_HIGHLIGHTS).unwrap()
    }
}

impl Layer for RustLayer {
    fn capture(&self, idx: usize) -> Option<Capture> {
        self.captures_by_id.get(idx).and_then(|c| *c)
    }
    fn capture_from_node(&self, id: usize) -> Option<Capture> {
        self.node_to_capture.get(&id).map(|c| *c)
    }
    fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    fn update_highlights(&mut self, rope: &Rope) {
        self.tree = self.parser.parse_with(
            &mut |byte_idx, pos| {
                if byte_idx > rope.len_bytes() {
                    return [].as_ref();
                }
                let (s, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte_idx);
                let ret = &s.as_bytes()[byte_idx - chunk_byte_idx..];
                // println!("asked for {} {}, returned {:?}", byte_idx, pos, ret);
                ret
            },
            None, //self.tree.as_ref(),
        );
        if let Some(tree) = &self.tree {
            // print_tree(tree.root_node(), 0);

            self.node_to_capture.clear();
            let query = RustLayer::highlights_query();

            fn rope_bytes_to_str<'a>(
                rope: &'a Rope,
                range: std::ops::Range<usize>,
            ) -> Cow<'a, str> {
                let start_char = rope.byte_to_char(range.start);
                let end_char = rope.byte_to_char(range.end);
                rope.slice(start_char..end_char).into()
            }

            let mut cursor = QueryCursor::new();
            let captures = cursor.captures(&query, tree.root_node(), move |n: Node| {
                rope_bytes_to_str(&rope, n.byte_range())
                    .to_owned()
                    .to_string()
            });
            for cap in captures {
                for c in cap.0.captures {
                    if let Some(capture) = self.capture(c.index as usize) {
                        self.node_to_capture.insert(c.node.id(), capture);
                    }
                }
            }
        }
    }
}

impl fmt::Debug for RustLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RustLayer")
            .field("highlights_query", &self.highlights_query)
            .field("captures_by_id", &self.captures_by_id)
            .field("node_to_capture", &self.node_to_capture)
            .field("tree", &self.tree)
            .finish()
    }
}
