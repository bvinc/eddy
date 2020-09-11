use crate::language::capture::Capture;
use eddy_ts::{language, Language, Node, Query, QueryCursor};
use ropey::Rope;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Debug)]
pub struct RustLayer {
    highlights_query: Query,
    captures_by_id: Vec<Option<Capture>>,
    node_to_capture: HashMap<usize, Capture>,
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

        Self {
            highlights_query,
            captures_by_id,
            node_to_capture: HashMap::new(),
        }
    }
    pub fn lang() -> Language {
        language::rust()
    }
    pub fn highlights_query() -> Query {
        Query::new(Self::lang(), language::RUST_HIGHLIGHTS).unwrap()
    }
    pub fn capture(&self, idx: usize) -> Option<Capture> {
        self.captures_by_id.get(idx).and_then(|c| *c)
    }
    pub fn capture_from_node(&self, id: usize) -> Option<Capture> {
        self.node_to_capture.get(&id).map(|c| *c)
    }

    pub fn update_highlights(&mut self, rope: &Rope, root_node: Node) {
        self.node_to_capture.clear();
        let query = RustLayer::highlights_query();

        fn rope_bytes_to_str<'a>(rope: &'a Rope, range: std::ops::Range<usize>) -> Cow<'a, str> {
            let start_char = rope.byte_to_char(range.start);
            let end_char = rope.byte_to_char(range.end);
            rope.slice(start_char..end_char).into()
        }

        let mut cursor = QueryCursor::new();
        // let matches = cursor
        //     .matches(&query, tree.root_node(), move |n: Node| {
        //         rope_bytes_to_str(&rope, n.byte_range())
        //             .to_owned()
        //             .to_string()
        //     })
        //     .peekable();
        // for m in matches {
        //     println!("pattern_index {}", m.pattern_index);
        //     for c in m.captures {
        //         println!("index: {} node: {:?}", c.index, c.node);
        //     }
        // }
        let captures = cursor.captures(&query, root_node, move |n: Node| {
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
